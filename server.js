const express = require('express');
const fs = require('fs');
const path = require('path');

const app = express();
const PORT = process.env.PORT || 10001;
const ACCOUNTS_PATH = process.env.ACCOUNTS_PATH || path.join(__dirname, 'accounts.json');
const POLL_INTERVAL_MS = 60 * 1000;

let cachedUsage = {};

function loadAccounts() {
  try {
    const raw = fs.readFileSync(ACCOUNTS_PATH, 'utf-8');
    return JSON.parse(raw);
  } catch (err) {
    console.error(`Failed to load accounts from ${ACCOUNTS_PATH}:`, err.message);
    return [];
  }
}

async function fetchUsage(account) {
  const url = `https://claude.ai/api/organizations/${account.orgId}/usage`;
  const res = await fetch(url, {
    method: 'GET',
    headers: {
      'Cookie': `sessionKey=${account.sessionCookie}`,
      'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
      'Accept': 'application/json'
    }
  });

  if (!res.ok) {
    throw new Error(`HTTP ${res.status}: ${res.statusText}`);
  }

  return await res.json();
}

function normalizeWindow(w) {
  if (!w || typeof w !== 'object') return null;
  if (typeof w.utilization !== 'number' || !Number.isFinite(w.utilization)) return null;
  const utilization = Math.max(0, Math.min(100, w.utilization));
  const resets_at = typeof w.resets_at === 'string' ? w.resets_at : null;
  return { utilization, resets_at };
}

function normalizeUsage(raw) {
  if (!raw || typeof raw !== 'object') return null;
  const five_hour = normalizeWindow(raw.five_hour);
  const seven_day = normalizeWindow(raw.seven_day);
  if (!five_hour && !seven_day) return null;
  return { five_hour, seven_day };
}

async function pollAllAccounts() {
  const accounts = loadAccounts();
  const results = {};

  await Promise.all(accounts.map(async (account) => {
    try {
      const raw = await fetchUsage(account);
      const usage = normalizeUsage(raw);
      results[account.name] = {
        name: account.name,
        usage,
        error: null,
        lastUpdated: new Date().toISOString()
      };
    } catch (err) {
      results[account.name] = {
        name: account.name,
        usage: cachedUsage[account.name]?.usage || null,
        error: err.message,
        lastUpdated: cachedUsage[account.name]?.lastUpdated || null
      };
    }
  }));

  cachedUsage = results;
}

app.use(express.json());
app.use(express.static(path.join(__dirname, 'public')));

function maskCookie(cookie) {
  if (!cookie || cookie.length <= 8) return '********';
  return '•'.repeat(cookie.length - 8) + cookie.slice(-8);
}

app.get('/api/accounts', (_req, res) => {
  const accounts = loadAccounts();
  res.json(accounts.map(a => ({
    name: a.name,
    orgId: a.orgId,
    sessionCookie: maskCookie(a.sessionCookie)
  })));
});

function saveAccountsToDisk(accounts) {
  fs.writeFileSync(ACCOUNTS_PATH, JSON.stringify(accounts, null, 2) + '\n', 'utf-8');
}

app.post('/api/accounts', async (req, res) => {
  const account = req.body;
  if (!account.name || !account.orgId || !account.sessionCookie) {
    return res.status(400).json({ error: 'name, orgId, and sessionCookie are required' });
  }
  try {
    const accounts = loadAccounts();
    accounts.push({ name: account.name, orgId: account.orgId, sessionCookie: account.sessionCookie });
    saveAccountsToDisk(accounts);
    await pollAllAccounts();
    res.json({ ok: true, index: accounts.length - 1 });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

app.put('/api/accounts/:index', async (req, res) => {
  const idx = parseInt(req.params.index, 10);
  const updates = req.body;
  try {
    const accounts = loadAccounts();
    if (idx < 0 || idx >= accounts.length) {
      return res.status(404).json({ error: 'Account not found' });
    }
    if (updates.name) accounts[idx].name = updates.name;
    if (updates.orgId) accounts[idx].orgId = updates.orgId;
    if (updates.sessionCookie && !updates.sessionCookie.startsWith('•')) {
      accounts[idx].sessionCookie = updates.sessionCookie;
    }
    saveAccountsToDisk(accounts);
    await pollAllAccounts();
    res.json({ ok: true });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

app.delete('/api/accounts/:index', async (req, res) => {
  const idx = parseInt(req.params.index, 10);
  try {
    const accounts = loadAccounts();
    if (idx < 0 || idx >= accounts.length) {
      return res.status(404).json({ error: 'Account not found' });
    }
    accounts.splice(idx, 1);
    saveAccountsToDisk(accounts);
    await pollAllAccounts();
    res.json({ ok: true });
  } catch (err) {
    res.status(500).json({ error: err.message });
  }
});

app.get('/api/usage', (_req, res) => {
  res.json({
    accounts: Object.values(cachedUsage),
    pollIntervalMs: POLL_INTERVAL_MS
  });
});

app.get('/api/refresh', async (_req, res) => {
  await pollAllAccounts();
  res.json({
    accounts: Object.values(cachedUsage),
    pollIntervalMs: POLL_INTERVAL_MS
  });
});

async function start() {
  const accounts = loadAccounts();
  if (accounts.length === 0) {
    console.log('No accounts configured. Add accounts via the dashboard UI or copy accounts.example.json to accounts.json.');
  } else {
    console.log(`Loaded ${accounts.length} account(s): ${accounts.map(a => a.name).join(', ')}`);
  }

  await pollAllAccounts();

  setInterval(pollAllAccounts, POLL_INTERVAL_MS);

  app.listen(PORT, '127.0.0.1', () => {
    console.log(`Dashboard running at http://localhost:${PORT}`);
  });
}

start();
