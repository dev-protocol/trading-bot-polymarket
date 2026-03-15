//! Rust fork of v2_python — full logic: market + user WSS, detector, pair orders (stub or real),
//! portfolio, pause, rebalance, auto-switch market, db/markets.json.

mod allowance;
mod btc_slug;
mod clob_client;
mod config;
mod db;
mod detector;
mod gamma_api;
mod pair_orders;
mod portfolio_state;
mod wss_listener;

use anyhow::Result;
use rustls::crypto::aws_lc_rs;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::info;

const SWITCH_BUFFER_SEC: u64 = 5;
const FILLED_ORDER_TOLERANCE: f64 = 0.01;

/// Shared state for the current market (same as Python globals).
struct BotState {
    current_orderbook: detector::OrderBook5,
    detector_state: detector::DetectorState,
    pair_orders_placed: u32,
    pause_place_pair_order: bool,
    pause_count: u32,
    last_best_ask: Option<f64>,
    yes_asset_id: String,
    no_asset_id: String,
    condition_id: String,
}

impl BotState {
    fn reset(&mut self, yes: &str, no: &str, cid: &str) {
        self.yes_asset_id = yes.to_string();
        self.no_asset_id = no.to_string();
        self.condition_id = cid.to_string();
        self.current_orderbook = detector::OrderBook5::default();
        self.detector_state.reset();
        self.pair_orders_placed = 0;
        self.pause_place_pair_order = false;
        self.pause_count = 0;
        self.last_best_ask = None;
    }
}

fn log_info(msg: &str) {
    info!("{}", msg);
    if config::log_to_file() {
        let _ = std::fs::create_dir_all("db");
        let line = format!("[INFO] {}\n", msg);
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("db/output.log")
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
    }
}

fn log_section(title: &str) {
    let block = format!("\n{}\n  {}\n{}\n", "=".repeat(60), title, "=".repeat(60));
    info!("{}", block);
    if config::log_to_file() {
        let _ = std::fs::create_dir_all("db");
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("db/output.log")
            .and_then(|mut f| std::io::Write::write_all(&mut f, block.as_bytes()));
    }
}

fn can_place(executor: Option<&dyn clob_client::OrderExecutor>, yes: &str, no: &str) -> bool {
    (executor.is_some() || !config::dry_run()) && !yes.is_empty() && !no.is_empty()
}

/// Rebalance until |imbalance| < REBALANCE_SIZE. Same logic as Python _maybe_rebalance.
async fn maybe_rebalance(
    state: &RwLock<BotState>,
    portfolio: Option<&Arc<portfolio_state::PortfolioState>>,
    executor: Option<&Arc<dyn clob_client::OrderExecutor>>,
) -> bool {
    let rebalance_sz = config::rebalance_size();
    if rebalance_sz <= 0.0 {
        return true;
    }
    let portfolio = match portfolio {
        Some(p) => p,
        None => return true,
    };
    let ex = match executor {
        Some(e) => e,
        None => return true,
    };
    let (cid, yes_id, no_id) = {
        let s = state.read().unwrap();
        if s.condition_id.is_empty() {
            return true;
        }
        (s.condition_id.clone(), s.yes_asset_id.clone(), s.no_asset_id.clone())
    };
    let rebalance_order_sz = pair_orders::get_rebalance_order_size();
    let max_rounds = 20u32;
    for _ in 0..max_rounds {
        let (qty_up, qty_down, _, _) = portfolio.get_position(&cid);
        let imbalance = qty_up - qty_down;
        if imbalance.abs() < rebalance_sz {
            return true;
        }
        let (best_bid, best_ask, _, best_ask_size) = {
            let s = state.read().unwrap();
            detector::best_bid_ask(&s.current_orderbook)
        };
        if best_bid == 0.0 && best_ask == 0.0 {
            return false;
        }
        let amount = imbalance.abs().min(rebalance_order_sz);
        if imbalance > 0.0 {
            let price_down = (1.0 - best_bid - 0.01).max(0.01);
            let (ok, status, oid) = pair_orders::place_down_order(Some(ex.as_ref()), price_down, &no_id, Some(amount));
            if !ok {
                return false;
            }
            if status == "matched" {
                portfolio.apply_immediate_fill(&oid, &cid, "DOWN", "BUY", amount, price_down);
                log_info(&format!("REBALANCE filled {:.1} — checking again", amount));
            }
        } else {
            let (ok, status, oid) = pair_orders::place_up_order(Some(ex.as_ref()), best_ask, &yes_id, Some(amount));
            if !ok {
                return false;
            }
            if status == "matched" {
                portfolio.apply_immediate_fill(&oid, &cid, "UP", "BUY", amount, best_ask);
                log_info(&format!("REBALANCE filled {:.1} — checking again", amount));
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    log_info("REBALANCE max rounds reached — keeping pause");
    false
}

/// After PAUSE_WAIT_SEC: rebalance then resume. Same as Python _run_pause_then_resume.
async fn run_pause_then_resume(
    state: Arc<RwLock<BotState>>,
    portfolio: Option<Arc<portfolio_state::PortfolioState>>,
    executor: Option<Arc<dyn clob_client::OrderExecutor>>,
) {
    let wait = config::pause_wait_sec();
    tokio::time::sleep(Duration::from_secs_f64(wait)).await;
    let rebalance_done = maybe_rebalance(
        &state,
        portfolio.as_ref(),
        executor.as_ref(),
    ).await;
    if !rebalance_done {
        log_info("REBALANCE not complete — no pair orders until next pause cycle");
        return;
    }
    let limit = config::limit_pause_count();
    let mut s = state.write().unwrap();
    if limit > 0 && s.pause_count >= limit {
        log_info(&format!("STOP Reached pause limit ({}/{}) — no more pair orders", s.pause_count, limit));
    } else {
        s.pause_place_pair_order = false;
        s.pair_orders_placed = 0;
        log_info(&format!("Pause ended — resuming pair orders (limit={})", config::pair_order_limit()));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let _ = aws_lc_rs::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    log_section("Startup");
    info!("v2_rust — Polymarket BTC 15m bot (full logic fork of v2_python)");
    if config::private_key().is_some() {
        info!("PRIVATE_KEY loaded");
    } else {
        info!("PRIVATE_KEY not set — DRY RUN only");
    }
    info!("CLOB_HOST={}", config::clob_host());
    info!("DRY_RUN={}", config::dry_run());
    let place_real = !config::dry_run() && config::private_key().is_some();
    log_info(if place_real { "Placing REAL orders" } else { "DRY RUN - no real orders (set DRY_RUN=0 to enable)" });

    if place_real {
        if let Some(pk) = config::private_key() {
            let rpc_url = config::poly_rpc_url();
            match allowance::wallet_address_and_balance(&pk, &rpc_url).await {
                Ok((addr, balance)) => {
                    log_info(&format!("Wallet {} POL balance={} wei", addr, balance));
                    if balance.is_zero() {
                        log_info("Skipping allowance approval: wallet has 0 POL for gas");
                    } else if let Err(err) = allowance::approve_allowance(&pk, &rpc_url, true).await {
                        log_info(&format!("Allowance check/approve failed: {}", err));
                    }
                }
                Err(err) => log_info(&format!("Wallet balance check failed: {}", err)),
            }
        }
    }

    let portfolio: Option<Arc<portfolio_state::PortfolioState>> =
        if config::starting_cash() > 0.0 {
            let p = Arc::new(portfolio_state::PortfolioState::new(config::starting_cash()));
            log_info(&format!("Portfolio cash=${:.2}", p.cash_balance.read().unwrap()));
            Some(p)
        } else {
            log_info("Portfolio init skipped (STARTING_CASH not set)");
            None
        };

    let executor: Arc<dyn clob_client::OrderExecutor> = clob_client::default_executor();

    let auth_dir = std::path::Path::new("auth");
    let user_auth = clob_client::load_user_auth(auth_dir);

    info!("15m current {}", btc_slug::get_slug_15m(false));
    info!("15m next    {}", btc_slug::get_slug_15m(true));
    log_info(&format!(
        "PAIR_ORDER_LIMIT={}  PAUSE_WAIT_SEC={}  LIMIT_PAUSE_COUNT={}  REBALANCE_SIZE={}  REBALANCE_ORDER_SIZE={:?}",
        config::pair_order_limit(),
        config::pause_wait_sec(),
        config::limit_pause_count(),
        config::rebalance_size(),
        config::rebalance_order_size(),
    ));

    // Redeem unredeemed at startup (log only; real redeem needs onchain)
    let unredeemed = db::unredeemed_markets();
    if !unredeemed.is_empty() {
        log_info(&format!("STARTUP would redeem {} unredeemed market(s) (onchain not wired)", unredeemed.len()));
    }

    log_section("WSS market + user (auto-switch on new 15m window)");

    let http = reqwest::Client::new();
    let mut use_next_slug = false;
    let mut prev_slug: Option<String> = None;
    let mut prev_cid: Option<String> = None;

    loop {
        let slug_15m = btc_slug::get_slug_15m(use_next_slug);
        let market = match gamma_api::fetch_market_for_slug(&http, &slug_15m).await? {
            Some(m) => m,
            None => {
                log_info(&format!("No market for slug {} - retrying in 30s", slug_15m));
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        let condition_id = market.condition_id.clone();
        db::add_market_to_db(&slug_15m, &condition_id, None);

        if let (Some(ps), Some(pc)) = (prev_slug.as_ref(), prev_cid.as_ref()) {
            if config::auto_redeem_delay_sec() > 0.0 {
                log_info(&format!("AUTO_REDEEM would schedule in {:.0}s for previous market (onchain not wired)", config::auto_redeem_delay_sec()));
            }
        }
        prev_slug = None;
        prev_cid = None;

        let window_end = market.window_end_sec.unwrap_or_else(|| btc_slug::get_window_end_ts_15m());
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let run_for_secs = (window_end.saturating_sub(now).saturating_sub(SWITCH_BUFFER_SEC)) as u64;
        let run_for = Duration::from_secs(run_for_secs);

        let state: Arc<RwLock<BotState>> = Arc::new(RwLock::new(BotState {
            current_orderbook: detector::OrderBook5::default(),
            detector_state: detector::DetectorState::default(),
            pair_orders_placed: 0,
            pause_place_pair_order: false,
            pause_count: 0,
            last_best_ask: None,
            yes_asset_id: market.yes_asset_id.clone(),
            no_asset_id: market.no_asset_id.clone(),
            condition_id: condition_id.clone(),
        }));

        state.write().unwrap().reset(&market.yes_asset_id, &market.no_asset_id, &condition_id);

        let asset_ids = vec![market.yes_asset_id.clone(), market.no_asset_id.clone()];
        let market_sub = wss_listener::build_market_subscribe(&asset_ids);
        let remaining = btc_slug::get_time_remaining_sec_15m();

        log_section(&format!("Market  {}", slug_15m));
        log_info(&format!("WSS market  asset_ids: {}.. {}..", &asset_ids[0][..asset_ids[0].len().min(20)], &asset_ids[1][..asset_ids[1].len().min(20)]));
        log_info(&format!("WSS user    condition_id: {}..", &condition_id[..condition_id.len().min(18)]));
        log_info(&format!("Window ends in  {:.0}s", remaining));

        let state_m = state.clone();
        let yes_id = market.yes_asset_id.clone();
        let no_id = market.no_asset_id.clone();
        let portfolio_m = portfolio.clone();
        let executor_m = executor.clone();
        let pair_limit = config::pair_order_limit();
        let limit_pause = config::limit_pause_count();
        let pause_wait = config::pause_wait_sec();

        let on_book = move |event_type: &str, ev: &serde_json::Value| {
            if event_type != "book" {
                return;
            }
            let asset_id = ev.get("asset_id").and_then(|v| v.as_str()).unwrap_or("");
            let mut s = state_m.write().unwrap();
            if asset_id != s.yes_asset_id {
                return;
            }
            let bids = ev.get("bids").and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[]);
            let asks = ev.get("asks").and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[]);
            let ob = detector::book_to_5_deep(bids, asks);
            s.current_orderbook = ob.clone();
            let (best_bid, best_ask, best_bid_size, best_ask_size) = detector::best_bid_ask(&ob);
            s.last_best_ask = Some(best_ask);
            let direction = detector::detect(&mut s.detector_state, best_bid, best_ask);
            let ignore_signal = matches!(direction, Some("rise") if best_ask < 0.5)
                || matches!(direction, Some("fall") if best_ask > 0.5);
            let can = can_place(Some(executor_m.as_ref()), &s.yes_asset_id, &s.no_asset_id);
            let liquidity_ok = pair_orders::liquidity_ok_for_pair(best_bid_size, best_ask_size);
            let order_sz = pair_orders::get_order_size();
            if direction.is_some() && !ignore_signal && can && liquidity_ok && !s.pause_place_pair_order {
                let dir = direction.unwrap();
                let ex = Some(executor_m.as_ref());
                if dir == "rise" {
                    s.pair_orders_placed += 1;
                    let size_up = order_sz.min(best_ask_size);
                    let (ok, status, order_id) = if place_real {
                        pair_orders::place_up_order(ex, best_ask, &s.yes_asset_id, Some(size_up))
                    } else {
                        log_info(&format!("DRY RUN would place UP at {:.3} size={}", best_ask, size_up));
                        (true, "live".to_string(), "dry-run-up".to_string())
                    };
                    if status == "live" || status == "delayed" {
                        s.pair_orders_placed = s.pair_orders_placed.saturating_sub(1);
                    }
                    if !ok {
                        log_info("ORDER UP failed (check pair_orders / API logs)");
                        s.pair_orders_placed = s.pair_orders_placed.saturating_sub(1);
                    } else {
                        log_info(&format!("ORDER UP placed at {:.3} size={} status={}", best_ask, size_up, status));
                        if let Some(ref p) = portfolio_m {
                            if status == "matched" {
                                p.apply_immediate_fill(&order_id, &s.condition_id, "UP", "BUY", size_up, best_ask);
                            } else {
                                p.register_order(&order_id, &s.condition_id, "UP", "BUY", size_up, best_ask);
                            }
                        }
                        if status == "matched" {
                            let size_down = order_sz.min(best_bid_size);
                            let down_price = (1.0 - best_ask - 0.01).max(0.01);
                            let (follow_ok, follow_status, down_id) = if place_real {
                                pair_orders::place_down_order(ex, down_price, &s.no_asset_id, Some(size_down))
                            } else {
                                log_info(&format!("DRY RUN would place DOWN at {:.3} size={}", down_price, size_down));
                                (true, "matched".to_string(), "dry-run-down".to_string())
                            };
                            if follow_ok && !down_id.is_empty() {
                                log_info(&format!("ORDER DOWN follow placed size={} status={} order_down_id={}..", size_down, follow_status, &down_id[..down_id.len().min(24)]));
                                if let Some(ref p) = portfolio_m {
                                    if follow_status == "matched" {
                                        p.apply_immediate_fill(&down_id, &s.condition_id, "DOWN", "BUY", size_down, down_price);
                                    }
                                }
                            }
                            if s.pair_orders_placed >= pair_limit {
                                s.pause_count += 1;
                                s.pause_place_pair_order = true;
                                let limit_str = if limit_pause == 0 { "∞".to_string() } else { limit_pause.to_string() };
                                log_info(&format!("PAUSE {}/{} {} pair orders — waiting {}s", s.pause_count, limit_str, s.pair_orders_placed, pause_wait));
                                let state_c = state_m.clone();
                                let port_c = portfolio_m.clone();
                                let ex_c = executor_m.clone();
                                tokio::spawn(async move {
                                    run_pause_then_resume(state_c, port_c, Some(ex_c)).await;
                                });
                            }
                        } else if (status == "live" || status == "delayed") && !order_id.is_empty() {
                            if place_real {
                                pair_orders::cancel_order(ex, &order_id);
                            }
                            if let Some(ref p) = portfolio_m {
                                p.unregister_order(&order_id);
                            }
                            log_info(&format!("ORDER UP cancelled order_id={}..", &order_id[..order_id.len().min(24)]));
                        } else if (status == "live" || status == "delayed") && !order_id.is_empty() && !place_real {
                            log_info("DRY RUN would cancel UP");
                            if let Some(ref p) = portfolio_m {
                                p.unregister_order(&order_id);
                            }
                        }
                    }
                } else {
                    // fall
                    s.pair_orders_placed += 1;
                    let best_ask_down = 1.0 - best_bid;
                    let size_down = order_sz.min(best_bid_size);
                    let (ok, status, order_id) = if place_real {
                        pair_orders::place_down_order(ex, best_ask_down, &s.no_asset_id, Some(size_down))
                    } else {
                        log_info(&format!("DRY RUN would place DOWN at {:.3} size={}", best_ask_down, size_down));
                        (true, "live".to_string(), "dry-run-down".to_string())
                    };
                    if status == "live" || status == "delayed" {
                        s.pair_orders_placed = s.pair_orders_placed.saturating_sub(1);
                    }
                    if !ok {
                        log_info("ORDER DOWN failed (check pair_orders / API logs)");
                        s.pair_orders_placed = s.pair_orders_placed.saturating_sub(1);
                    } else {
                        log_info(&format!("ORDER DOWN placed at {:.3} size={} status={}", best_ask_down, size_down, status));
                        if let Some(ref p) = portfolio_m {
                            if status == "matched" {
                                p.apply_immediate_fill(&order_id, &s.condition_id, "DOWN", "BUY", size_down, best_ask_down);
                            } else {
                                p.register_order(&order_id, &s.condition_id, "DOWN", "BUY", size_down, best_ask_down);
                            }
                        }
                        if status == "matched" {
                            let size_up = order_sz.min(best_ask_size);
                            let up_price = (best_bid - 0.01).max(0.01);
                            let (follow_ok, follow_status, up_id) = if place_real {
                                pair_orders::place_up_order(ex, up_price, &s.yes_asset_id, Some(size_up))
                            } else {
                                log_info(&format!("DRY RUN would place UP at {:.3} size={}", up_price, size_up));
                                (true, "matched".to_string(), "dry-run-up".to_string())
                            };
                            if follow_ok && !up_id.is_empty() {
                                log_info(&format!("ORDER UP follow placed size={} status={} order_up_id={}..", size_up, follow_status, &up_id[..up_id.len().min(24)]));
                                if let Some(ref p) = portfolio_m {
                                    if follow_status == "matched" {
                                        p.apply_immediate_fill(&up_id, &s.condition_id, "UP", "BUY", size_up, up_price);
                                    }
                                }
                            }
                            if s.pair_orders_placed >= pair_limit {
                                s.pause_count += 1;
                                s.pause_place_pair_order = true;
                                let limit_str = if limit_pause == 0 { "∞".to_string() } else { limit_pause.to_string() };
                                log_info(&format!("PAUSE {}/{} {} pair orders — waiting {}s", s.pause_count, limit_str, s.pair_orders_placed, pause_wait));
                                let state_c = state_m.clone();
                                let port_c = portfolio_m.clone();
                                let ex_c = executor_m.clone();
                                tokio::spawn(async move {
                                    run_pause_then_resume(state_c, port_c, Some(ex_c)).await;
                                });
                            }
                        } else if (status == "live" || status == "delayed") && !order_id.is_empty() {
                            if place_real {
                                pair_orders::cancel_order(ex, &order_id);
                            }
                            if let Some(ref p) = portfolio_m {
                                p.unregister_order(&order_id);
                            }
                            log_info(&format!("ORDER DOWN cancelled order_id={}..", &order_id[..order_id.len().min(24)]));
                        } else if (status == "live" || status == "delayed") && !order_id.is_empty() && !place_real {
                            log_info("DRY RUN would cancel DOWN");
                            if let Some(ref p) = portfolio_m {
                                p.unregister_order(&order_id);
                            }
                        }
                    }
                }
            }
            if !config::log_to_file() {
                return;
            }
            if let Some(dir) = direction {
                info!("BOOK bid={:.3} ask={:.3} | {}", best_bid, best_ask, dir);
            }
        };

        let state_u = state.clone();
        let portfolio_u = portfolio.clone();
        let on_user = move |event_type: &str, ev: &serde_json::Value| {
            if event_type != "order" {
                return;
            }
            let portfolio_u = match &portfolio_u {
                Some(p) => p,
                None => return,
            };
            let order_id = ev.get("id").or_else(|| ev.get("order_id")).and_then(|v| v.as_str()).unwrap_or("").trim();
            if order_id.is_empty() {
                return;
            }
            let cid = ev.get("market").and_then(|v| v.as_str()).unwrap_or("").trim();
            let outcome = (ev.get("outcome").and_then(|v| v.as_str()).unwrap_or("")).trim().to_uppercase().replace("YES", "UP").replace("NO", "DOWN");
            let side = (ev.get("side").and_then(|v| v.as_str()).unwrap_or("BUY")).trim().to_uppercase();
            let price: f64 = ev.get("price").and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64())).unwrap_or(0.0);
            let msg_type = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if msg_type == "PLACEMENT" {
                let size: f64 = ev.get("original_size").or_else(|| ev.get("size")).and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64())).unwrap_or(0.0);
                portfolio_u.register_order(order_id, cid, &outcome, &side, size, price);
                return;
            }
            if msg_type == "UPDATE" {
                let (cid_use, price_use, outcome_use, order_size) = {
                    let orders = portfolio_u.open_orders.read().unwrap();
                    let stored = match orders.get(order_id) {
                        Some(s) => s.clone(),
                        None => return,
                    };
                    let cid_use = if cid.is_empty() { stored.condition_id.clone() } else { cid.to_string() };
                    let price_use = if price <= 0.0 { stored.price } else { price };
                    let outcome_use = if outcome.is_empty() { stored.outcome.clone() } else { outcome };
                    (cid_use, price_use, outcome_use, stored.size)
                };
                let size_matched: f64 = ev.get("size_matched").and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64())).unwrap_or(0.0);
                portfolio_u.on_order_update(order_id, size_matched, &cid_use, &outcome_use, &side, price_use);
                if size_matched >= order_size - FILLED_ORDER_TOLERANCE {
                    portfolio_u.unregister_order(order_id);
                }
                return;
            }
            if msg_type == "CANCELLATION" {
                portfolio_u.unregister_order(order_id);
            }
        };

        let run_for = run_for.max(Duration::from_secs(1));
        let market_sub = market_sub.clone();
        let user_sub = user_auth.as_ref().map(|a| wss_listener::build_user_subscribe(a, &[condition_id.clone()]));

        let market_handle = tokio::spawn(async move {
            wss_listener::run_market_wss_until(
                config::DEFAULT_MARKET_WSS,
                &market_sub,
                run_for,
                on_book,
            )
            .await
        });

        let user_handle = if let Some(ref sub) = user_sub {
            let sub = sub.clone();
            Some(tokio::spawn(async move {
                wss_listener::run_user_wss_until(
                    config::DEFAULT_USER_WSS,
                    &sub,
                    run_for,
                    on_user,
                )
                .await
            }))
        } else {
            log_info("User WSS skipped (auth/auth.json not found; run Python bot once to derive creds)");
            None
        };

        let _ = market_handle.await;
        if let Some(h) = user_handle {
            let _ = h.await;
        }

        log_info("Switching to new 15m market...");
        prev_slug = Some(slug_15m);
        prev_cid = Some(condition_id);
        use_next_slug = true;
    }
}
