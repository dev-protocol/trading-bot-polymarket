# v2_rust

**Rust fork of v2_python.** Same idea (Polymarket BTC 15m market WSS + detector + pair orders), reimplemented in Rust.

**Standalone:** no runtime dependency on the Python repo. Use your own `.env` in this directory and run with `cargo run` — no need for v2_python to be present.

## Install Rust

### Ubuntu / Debian

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

### Windows

1. Download and run the installer: https://win.rustup.rs/x86_64 (or run `winget install Rustlang.Rustup`).
2. Restart the terminal, then run:
   ```powershell
   rustc --version
   cargo --version
   ```

## Does it work the same as v2_python?

**Logic: yes (forked).** The bot flow is ported, and live CLOB order placement is now wired through the official Rust SDK. Remaining gaps are auto-redeem and detector parity with Python's fuller signal stack.

| Feature | v2_python | v2_rust |
|--------|-----------|---------|
| Gamma API + 15m slug | ✅ | ✅ |
| Market WSS + 5-deep book | ✅ | ✅ |
| Rise/fall detector | ✅ | ✅ |
| **Real CLOB orders** | ✅ (py_clob_client) | ✅ (`polymarket-client-sdk`, pure Rust) |
| User WSS (auth + order updates) | ✅ | ✅ (auth from auth/auth.json) |
| Portfolio state (cash, positions, open orders) | ✅ | ✅ |
| Pause after N pairs + PAUSE_WAIT_SEC | ✅ | ✅ |
| Rebalance (REBALANCE_SIZE, REBALANCE_ORDER_SIZE) | ✅ | ✅ |
| Auto-switch to next 15m market | ✅ | ✅ |
| Liquidity check (MIN/MAX_LIQUIDITY_SIZE) | ✅ | ✅ |
| Cancel unfilled leg when other leg fills | ✅ | ✅ |
| db/markets.json + unredeemed | ✅ | ✅ |
| Auto-redeem (onchain) | ✅ | log only |
| ORDER_SIZE / get_order_size() | ✅ | ✅ |
| LOG_TO_FILE (db/output.log) | ✅ | ✅ |

## How to run this bot

1. **Install Rust** (see above) so `cargo` is available.

2. **Create a `.env`** in this directory (`v2_rust/`) with:

- `PRIVATE_KEY` — wallet private key (0x...) for live CLOB orders
- `CLOB_HOST` — optional, default `https://clob.polymarket.com`
- `DRY_RUN` — set to `1`/`true`/`yes` for dry run (no real orders)
- `SIGNATURE_TYPE` — `0` EOA, `1` proxy, `2` Gnosis Safe
- `FUNDER_ADDRESS` — optional override for Polymarket funder/proxy wallet

3. **Run the bot:**

   **Ubuntu / Linux / macOS:**
   ```bash
   cd v2_rust
   cargo run
   ```

   **Windows (PowerShell or CMD):**
   ```powershell
   cd v2_rust
   cargo run
   ```

   For dry run (no real orders), set `DRY_RUN=1` in `.env`. To place real orders, set `DRY_RUN=0` and set `PRIVATE_KEY` to your wallet private key (0x...).

## What it does

1. Loads config from `.env`.
2. Fetches the current 15m BTC market from Polymarket Gamma API (slug `btc-updown-15m-{unix}`).
3. Connects to the market WebSocket, subscribes to yes/no asset IDs.
4. On each `book` update for the UP (yes) asset: builds 5-deep book, runs rise/fall detector, and places/cancels real CLOB orders when `DRY_RUN=0`.

## Layout

- `config` — env (PRIVATE_KEY, CLOB_HOST, DRY_RUN)
- `btc_slug` — 15m slug and window remaining
- `gamma_api` — fetch market by slug
- `wss_listener` — market WSS connect and subscribe
- `detector` — 5-deep book and simple rise/fall
- `clob_client` — native Rust CLOB client/auth via `polymarket-client-sdk`
- `pair_orders` — real place/cancel flow

## TODO

- Improve detector parity with Python OBI/trend logic.
- Auto-redeem onchain.

## Reference

- [Polymarket WSS](https://docs.polymarket.com/market-data/websocket/overview)
- [Polymarket CLOB](https://docs.polymarket.com/developers/CLOB/)
