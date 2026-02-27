# Claude Multi-Account Usage Dashboard

A self-hosted dashboard for monitoring Claude.ai usage across multiple accounts. Shows session (5h) and weekly (7d) utilization with reset countdowns.

## Quick Start

```bash
npm install
npm start
```

Open http://localhost:10001 and add your accounts via the UI.

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

4. Copy the Org ID and Session Cookie into the dashboard.

### Option 2: DevTools manually

**Organization ID:**
1. Go to https://claude.ai
2. DevTools (F12) → Application → Cookies → `https://claude.ai`
3. Find the `lastActiveOrg` cookie — that value is your Org ID

**Session Cookie:**
1. Same Cookies panel
2. Find the `sessionKey` cookie (starts with `sk-ant-sid...`) — that value is your Session Cookie
   - Note: this cookie is `HttpOnly` so `document.cookie` can't read it — you must grab it from the Cookies panel

### Option 3: Network tab

1. DevTools (F12) → Network tab
2. Send any message in Claude
3. Click any request to `claude.ai/api/organizations/...`
4. The URL contains your Org ID: `/api/organizations/{THIS_IS_YOUR_ORG_ID}/...`
5. In Request Headers, find `Cookie:` → the `sessionKey=...` value is your Session Cookie

## Configuration

Accounts can be managed two ways:

- **Web UI**: Click "Manage Accounts" on the dashboard
- **JSON file**: Edit `accounts.json` directly:

```json
[
  {
    "name": "Personal",
    "orgId": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
    "sessionCookie": "sk-ant-..."
  },
  {
    "name": "Work",
    "orgId": "yyyyyyyy-yyyy-yyyy-yyyy-yyyyyyyyyyyy",
    "sessionCookie": "sk-ant-..."
  }
]
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `10001` | Dashboard port |
| `ACCOUNTS_PATH` | `./accounts.json` | Path to accounts config |

## Notes

- Session cookies expire periodically — you'll need to update them when they do
- The dashboard polls each account every 60 seconds
- Listens on `127.0.0.1` only (not exposed to network)
