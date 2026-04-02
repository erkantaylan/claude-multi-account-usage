# Claude Multi-Account Usage Monitor

Monitor Claude.ai usage across multiple accounts. Includes a web dashboard and a system tray app.

## Structure

```
cmau/
├── accounts.example.json    # Example config
├── web/                     # Web dashboard (Node.js/Express)
│   ├── server.js
│   ├── package.json
│   └── public/index.html
└── tray/                    # System tray app (Tauri/Rust)
    ├── src/index.html
    └── src-tauri/
        ├── Cargo.toml
        └── src/main.rs
```

## Web Dashboard

```bash
cd web
npm install
npm start
```

Open http://localhost:10001. Add accounts via the UI.

## System Tray App

Sits in the Ubuntu AppIndicator area (top-right). Click the icon to show a compact usage popup.

### Build & Run

```bash
cd tray
cargo run --manifest-path src-tauri/Cargo.toml
```

### Features

- System tray icon with color-coded usage level (green/yellow/red)
- Compact popup showing all accounts with 5h session + 7d weekly utilization
- Progress bars with reset countdowns
- Auto-refresh every 5 minutes
- Right-click menu: Refresh Now, Quit

### Configuration

Set `ACCOUNTS_PATH` to point to your accounts.json:

```bash
ACCOUNTS_PATH=/path/to/accounts.json cargo run --manifest-path src-tauri/Cargo.toml
```

If not set, looks for `accounts.json` next to the binary.

## Getting Your Credentials

Each account needs two things: **Organization ID** and **Session Cookie**.

### Option 1: Browser Console (easiest)

1. Log in to https://claude.ai
2. Open DevTools (F12) → Console
3. Paste this:

```js
(() => {
  const orgId = document.cookie.split('; ').find(r => r.startsWith('lastActiveOrg='))?.split('=')[1];
  const sessionKey = document.cookie.split('; ').find(r => r.startsWith('sessionKey='))?.split('=')[1];
  console.log('\n%c Claude Credentials ', 'background:#2c84db;color:white;font-weight:bold;padding:4px 8px;border-radius:4px');
  console.log(`Org ID:         ${orgId || '(not found)'}`);
  console.log(`Session Cookie: ${sessionKey || '(not found — grab it manually from Application > Cookies, look for sessionKey starting with sk-ant-sid)'}`);
  if (orgId && sessionKey) {
    const data = JSON.stringify({ orgId, sessionCookie: sessionKey }, null, 2);
    console.log(`\nReady to paste:\n${data}`);
    navigator.clipboard?.writeText(data).then(() => console.log('(copied to clipboard)'));
  }
})();
```

4. Copy the Org ID and Session Cookie into the config.

### Option 2: DevTools manually

**Organization ID:**
1. Go to https://claude.ai
2. DevTools (F12) → Application → Cookies → `https://claude.ai`
3. Find the `lastActiveOrg` cookie — that value is your Org ID

**Session Cookie:**
1. Same Cookies panel
2. Find the `sessionKey` cookie (starts with `sk-ant-sid...`)
   - Note: this cookie is `HttpOnly` so `document.cookie` can't read it — grab it from the Cookies panel

## accounts.json Format

```json
[
  {
    "name": "Personal",
    "orgId": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
    "sessionCookie": "sk-ant-..."
  }
]
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `10001` | Web dashboard port |
| `ACCOUNTS_PATH` | `./accounts.json` | Path to accounts config |

## Notes

- Session cookies expire periodically — update them when they do
- Web dashboard polls every 5 minutes, tray app polls every 5 minutes
- Web dashboard listens on `127.0.0.1` only
