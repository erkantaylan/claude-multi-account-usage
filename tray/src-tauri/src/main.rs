// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager, State,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Account {
    name: String,
    #[serde(rename = "orgId")]
    org_id: String,
    #[serde(rename = "sessionCookie")]
    session_cookie: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageWindow {
    utilization: f64,
    resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageData {
    five_hour: Option<UsageWindow>,
    seven_day: Option<UsageWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountUsage {
    name: String,
    usage: Option<UsageData>,
    error: Option<String>,
    #[serde(rename = "lastUpdated")]
    last_updated: Option<String>,
}

struct AppState {
    usage_data: Mutex<Vec<AccountUsage>>,
    accounts_path: String,
}

fn get_accounts_path() -> String {
    std::env::var("ACCOUNTS_PATH").unwrap_or_else(|_| {
        let mut path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("."))
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        // In dev, walk up from src-tauri/target/debug to find accounts.json
        // In prod, look next to the binary
        for candidate in [
            path.join("accounts.json"),
            path.join("../../../accounts.json"),
            path.join("../../../../accounts.json"),
            PathBuf::from("accounts.json"),
        ] {
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
        path.push("accounts.json");
        path.to_string_lossy().to_string()
    })
}

fn load_accounts(path: &str) -> Vec<Account> {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            eprintln!("Failed to load accounts from {}: {}", path, e);
            Vec::new()
        }
    }
}

async fn fetch_usage(account: &Account) -> Result<UsageData, String> {
    let url = format!(
        "https://claude.ai/api/organizations/{}/usage",
        account.org_id
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Cookie", format!("sessionKey={}", account.session_cookie))
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let raw: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))?;

    let five_hour = raw.get("five_hour").and_then(|w| {
        let util = w.get("utilization")?.as_f64()?;
        let resets = w.get("resets_at").and_then(|r| r.as_str().map(String::from));
        Some(UsageWindow {
            utilization: util.clamp(0.0, 100.0),
            resets_at: resets,
        })
    });

    let seven_day = raw.get("seven_day").and_then(|w| {
        let util = w.get("utilization")?.as_f64()?;
        let resets = w.get("resets_at").and_then(|r| r.as_str().map(String::from));
        Some(UsageWindow {
            utilization: util.clamp(0.0, 100.0),
            resets_at: resets,
        })
    });

    if five_hour.is_none() && seven_day.is_none() {
        return Err("No usage data in response".to_string());
    }

    Ok(UsageData {
        five_hour,
        seven_day,
    })
}

async fn poll_all_accounts(state: &AppState) -> Vec<AccountUsage> {
    let accounts = load_accounts(&state.accounts_path);
    let mut results = Vec::new();

    for account in &accounts {
        let now = chrono::Utc::now().to_rfc3339();
        match fetch_usage(account).await {
            Ok(usage) => {
                results.push(AccountUsage {
                    name: account.name.clone(),
                    usage: Some(usage),
                    error: None,
                    last_updated: Some(now),
                });
            }
            Err(err) => {
                // Try to keep previous usage data on error
                let prev = state.usage_data.lock().unwrap();
                let prev_usage = prev
                    .iter()
                    .find(|u| u.name == account.name)
                    .and_then(|u| u.usage.clone());
                let prev_updated = prev
                    .iter()
                    .find(|u| u.name == account.name)
                    .and_then(|u| u.last_updated.clone());
                drop(prev);

                results.push(AccountUsage {
                    name: account.name.clone(),
                    usage: prev_usage,
                    error: Some(err),
                    last_updated: prev_updated,
                });
            }
        }
    }

    // Update cached state
    *state.usage_data.lock().unwrap() = results.clone();
    results
}

fn max_utilization(data: &[AccountUsage]) -> f64 {
    data.iter()
        .filter_map(|a| {
            a.usage.as_ref().map(|u| {
                let five = u.five_hour.as_ref().map(|w| w.utilization).unwrap_or(0.0);
                let seven = u.seven_day.as_ref().map(|w| w.utilization).unwrap_or(0.0);
                five.max(seven)
            })
        })
        .fold(0.0_f64, f64::max)
}

fn make_tray_icon(level: &str) -> Vec<u8> {
    let (r, g, b) = match level {
        "red" => (206u8, 32, 41),
        "yellow" => (234, 179, 8),
        _ => (76, 175, 80),
    };

    let size: u32 = 32;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    let center = size as f64 / 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 - center + 0.5;
            let dy = y as f64 - center + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= 12.0 {
                pixels.extend_from_slice(&[r, g, b, 255]);
            } else {
                pixels.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    pixels
}

fn update_tray_icon(app: &AppHandle, usage_data: &[AccountUsage]) {
    let max_util = max_utilization(usage_data);
    let level = if max_util >= 90.0 {
        "red"
    } else if max_util >= 70.0 {
        "yellow"
    } else {
        "green"
    };

    let rgba = make_tray_icon(level);
    {
        let image = Image::new_owned(rgba, 32, 32);
        if let Some(tray) = app.tray_by_id("main-tray") {
            let _ = tray.set_icon(Some(image));
            let _ = tray.set_tooltip(Some(&format!("Claude Usage - Max: {:.0}%", max_util)));
        }
    }
}

#[tauri::command]
async fn get_usage(state: State<'_, AppState>) -> Result<Vec<AccountUsage>, String> {
    let data = state.usage_data.lock().unwrap().clone();
    Ok(data)
}

#[tauri::command]
async fn refresh_usage(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Vec<AccountUsage>, String> {
    let results = poll_all_accounts(&state).await;
    update_tray_icon(&app, &results);
    Ok(results)
}

fn main() {
    let accounts_path = get_accounts_path();
    eprintln!("Accounts path: {}", accounts_path);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            usage_data: Mutex::new(Vec::new()),
            accounts_path,
        })
        .invoke_handler(tauri::generate_handler![get_usage, refresh_usage])
        .setup(|app| {
            // Build tray menu
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let refresh = MenuItemBuilder::with_id("refresh", "Refresh Now").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&refresh, &quit]).build()?;

            // Build tray icon
            let rgba = make_tray_icon("green");
            let icon = Image::new_owned(rgba, 32, 32);

            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("Claude Usage Monitor")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "refresh" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<AppState>();
                            let results = poll_all_accounts(&state).await;
                            update_tray_icon(&app, &results);
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.eval("if(window.onUsageUpdate) window.onUsageUpdate()");
                            }
                        });
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Initial poll and auto-refresh timer
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                let results = poll_all_accounts(&state).await;
                update_tray_icon(&app_handle, &results);
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.eval("if(window.onUsageUpdate) window.onUsageUpdate()");
                }

                // Auto-refresh every 5 minutes
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
                interval.tick().await; // skip first tick (already polled)
                loop {
                    interval.tick().await;
                    let results = poll_all_accounts(&state).await;
                    update_tray_icon(&app_handle, &results);
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.eval("if(window.onUsageUpdate) window.onUsageUpdate()");
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
