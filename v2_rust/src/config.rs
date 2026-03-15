//! Config from .env. Same env vars as v2_python for parity.

use std::env;

pub const DEFAULT_CLOB_HOST: &str = "https://clob.polymarket.com";
pub const DEFAULT_MARKET_WSS: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
pub const DEFAULT_USER_WSS: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/user";
pub const GAMMA_BASE: &str = "https://gamma-api.polymarket.com";
pub const CHAIN_ID: u64 = 137;

pub fn private_key() -> Option<String> {
    env::var("PRIVATE_KEY").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn poly_rpc_url() -> String {
    env::var("POLY_RPC_URL")
        .or_else(|_| env::var("POLYGON_RPC_URL"))
        .unwrap_or_else(|_| "https://polygon-rpc.com".to_string())
}

pub fn clob_host() -> String {
    env::var("CLOB_HOST").unwrap_or_else(|_| DEFAULT_CLOB_HOST.to_string())
}

pub fn dry_run() -> bool {
    let v = env::var("DRY_RUN").unwrap_or_else(|_| "0".to_string());
    matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes")
}

pub fn signature_type() -> u32 {
    env::var("SIGNATURE_TYPE").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

pub fn funder_address() -> Option<String> {
    env::var("FUNDER_ADDRESS")
        .or_else(|_| env::var("POLY_FUNDER"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn order_size() -> f64 {
    env::var("ORDER_SIZE").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(5.0)
}

pub fn min_liquidity_size() -> f64 {
    env::var("MIN_LIQUIDITY_SIZE").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(30.0)
}

pub fn max_liquidity_size() -> f64 {
    env::var("MAX_LIQUIDITY_SIZE").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(10000.0)
}

pub fn pause_wait_sec() -> f64 {
    env::var("PAUSE_WAIT_SEC").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(5.0)
}

pub fn pair_order_limit() -> u32 {
    env::var("PAIR_ORDER_LIMIT").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(4)
}

pub fn limit_pause_count() -> u32 {
    env::var("LIMIT_PAUSE_COUNT").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

pub fn auto_redeem_delay_sec() -> f64 {
    env::var("AUTO_REDEEM_DELAY_SEC").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(120.0)
}

pub fn rebalance_size() -> f64 {
    env::var("REBALANCE_SIZE").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0)
}

pub fn rebalance_order_size() -> Option<f64> {
    env::var("REBALANCE_ORDER_SIZE").ok().and_then(|s| {
        let s = s.trim();
        if s.is_empty() {
            None
        } else {
            s.parse().ok()
        }
    })
}

pub fn log_to_file() -> bool {
    let v = env::var("LOG_TO_FILE").unwrap_or_else(|_| "0".to_string());
    matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes")
}

/// STARTING_CASH for portfolio init; 0 = no portfolio tracking.
pub fn starting_cash() -> f64 {
    env::var("STARTING_CASH").ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0)
}
