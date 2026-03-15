# v2_typescript

TypeScript fork of the Polymarket BTC 15m bot (same flow as v2_rust). Market WSS + detector + pair orders, using `@polymarket/clob-client` and Node.js.

## Install Node.js and npm

### Ubuntu / Debian

**Option A — NodeSource (recommended, current LTS):**
```bash
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
sudo apt-get install -y nodejs
node --version
npm --version
```

**Option B — system package:**
```bash
sudo apt update
sudo apt install -y nodejs npm
node --version
npm --version
```

### Windows

1. **Installer:** Download the LTS installer from https://nodejs.org/ and run it. Ensure "Add to PATH" is checked.
2. **Or with winget:**
   ```powershell
   winget install OpenJS.NodeJS.LTS
   ```
3. Restart the terminal, then run:
   ```powershell
   node --version
   npm --version
   ```

## How to run this bot

1. **Install Node.js and npm** (see above) so `node` and `npm` are available.

2. **Create a `.env`** in this directory (`v2_typescript/`) — you can copy from `.env.example`:
   - `PRIVATE_KEY` — wallet private key (0x...) for live CLOB orders
   - `DRY_RUN=1` for dry run (no real orders); `DRY_RUN=0` to place real orders
   - Optional: `CLOB_HOST`, `POLY_RPC_URL`, `ORDER_SIZE`, etc. (see `.env.example`)

3. **Install dependencies and run:**

   **Ubuntu / Linux / macOS:**
   ```bash
   cd v2_typescript
   npm install
   npm start
   ```

   **Windows (PowerShell or CMD):**
   ```powershell
   cd v2_typescript
   npm install
   npm start
   ```

   The bot will connect to the current BTC 15m market, subscribe to the order book, and place or simulate pair orders according to the detector. User WSS (live order/fill updates) requires `auth/auth.json`; it is created automatically the first time you run with `PRIVATE_KEY` set and the CLOB client derives API credentials.
