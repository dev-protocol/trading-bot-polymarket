//! Place/cancel pair orders. Same logic as v2_python pair_orders: ORDER_SIZE, liquidity, place_up/down, cancel.

use crate::clob_client::OrderExecutor;
use crate::config;
use tracing::info;

pub fn get_order_size() -> f64 {
    config::order_size()
}

pub fn get_rebalance_order_size() -> f64 {
    config::rebalance_order_size().unwrap_or_else(|| config::order_size())
}

pub fn get_min_liquidity_size() -> f64 {
    config::min_liquidity_size()
}

pub fn get_max_liquidity_size() -> f64 {
    config::max_liquidity_size()
}

/// True if liquidity at the chosen price for each leg is in [MIN, MAX].
pub fn liquidity_ok_for_pair(best_bid_size: f64, best_ask_size: f64) -> bool {
    let min = get_min_liquidity_size();
    let max = get_max_liquidity_size();
    (min..=max).contains(&best_bid_size) && (min..=max).contains(&best_ask_size)
}

/// Place BUY UP at price. size None = use ORDER_SIZE. Returns (ok, status, order_id).
pub fn place_up_order(
    executor: Option<&dyn OrderExecutor>,
    price: f64,
    yes_asset_id: &str,
    size: Option<f64>,
) -> (bool, String, String) {
    let sz = size.unwrap_or_else(get_order_size);
    if sz <= 0.0 {
        return (false, String::new(), String::new());
    }
    place_order_impl(executor, yes_asset_id, price, sz, "UP")
}

/// Place BUY DOWN at price.
pub fn place_down_order(
    executor: Option<&dyn OrderExecutor>,
    price: f64,
    no_asset_id: &str,
    size: Option<f64>,
) -> (bool, String, String) {
    let sz = size.unwrap_or_else(get_order_size);
    if sz <= 0.0 {
        return (false, String::new(), String::new());
    }
    place_order_impl(executor, no_asset_id, price, sz, "DOWN")
}

fn place_order_impl(
    executor: Option<&dyn OrderExecutor>,
    token_id: &str,
    price: f64,
    size: f64,
    side: &str,
) -> (bool, String, String) {
    match executor {
        Some(ex) => ex.place_order(token_id, price, size, side),
        None => {
            info!(
                "DRY RUN would place {} at {:.3} size={}",
                side,
                price,
                size
            );
            (
                true,
                "stub".to_string(),
                format!("dry-run-{}", side.to_lowercase()),
            )
        }
    }
}

pub fn cancel_order(executor: Option<&dyn OrderExecutor>, order_id: &str) -> bool {
    if order_id.is_empty() {
        return false;
    }
    match executor {
        Some(ex) => ex.cancel(order_id),
        None => {
            info!("DRY RUN would cancel order_id={}..", &order_id[..order_id.len().min(24)]);
            true
        }
    }
}
