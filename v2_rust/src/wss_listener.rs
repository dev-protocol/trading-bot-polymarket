//! Market and user WSS. Same logic as v2_python wss_listener.

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::info;

const PING_INTERVAL_SECS: u64 = 10;

pub fn build_market_subscribe(asset_ids: &[String]) -> String {
    serde_json::json!({
        "assets_ids": asset_ids,
        "type": "market",
        "custom_feature_enabled": true,
    })
    .to_string()
}

/// User channel subscribe: auth (apiKey, secret, passphrase), markets = condition IDs.
pub fn build_user_subscribe(auth: &crate::clob_client::UserAuth, markets: &[String]) -> String {
    serde_json::json!({
        "auth": {
            "apiKey": auth.api_key,
            "secret": auth.secret,
            "passphrase": auth.passphrase,
        },
        "markets": markets,
        "type": "user",
    })
    .to_string()
}

/// Run market WSS until `run_for` elapses or connection closes. Same message handling as Python.
pub async fn run_market_wss_until<F>(
    url: &str,
    subscribe_payload: &str,
    run_for: Duration,
    mut on_message: F,
) -> Result<()>
where
    F: FnMut(&str, &Value) + Send,
{
    let (ws_stream, _) = connect_async(url).await?;
    info!("Market WSS connected: {}", url);
    
    let (mut write, mut read) = ws_stream.split();
    write.send(Message::Text(subscribe_payload.to_string())).await?;

    let mut ping = interval(Duration::from_secs(PING_INTERVAL_SECS));
    ping.tick().await;

    let deadline = sleep(run_for);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => break,
            Some(Ok(msg)) = read.next() => {
                match msg {
                    Message::Text(t) => {
                        if t.trim() == "PONG" {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<Value>(&t) {
                            if let Some(arr) = v.as_array() {
                                for ev in arr {
                                    let event_type = ev.get("event_type").and_then(|x| x.as_str()).unwrap_or("");
                                    on_message(event_type, ev);
                                }
                            } else if v.is_object() {
                                let event_type = v.get("event_type").and_then(|x| x.as_str()).unwrap_or("");
                                on_message(event_type, &v);
                            }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = ping.tick() => {
                let _ = write.send(Message::Text("PING".to_string())).await;
            }
        }
    }
    Ok(())
}

/// Run user WSS until `run_for` elapses or connection closes.
pub async fn run_user_wss_until<F>(
    url: &str,
    subscribe_payload: &str,
    run_for: Duration,
    mut on_message: F,
) -> Result<()>
where
    F: FnMut(&str, &Value) + Send,
{
    let (ws_stream, _) = connect_async(url).await?;
    info!("User WSS connected: {}", url);

    let (mut write, mut read) = ws_stream.split();
    write.send(Message::Text(subscribe_payload.to_string())).await?;

    let mut ping = interval(Duration::from_secs(PING_INTERVAL_SECS));
    ping.tick().await;

    let deadline = sleep(run_for);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => break,
            Some(Ok(msg)) = read.next() => {
                match msg {
                    Message::Text(t) => {
                        if t.trim() == "PONG" {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<Value>(&t) {
                            if let Some(arr) = v.as_array() {
                                for ev in arr {
                                    let event_type = ev.get("event_type").and_then(|x| x.as_str()).unwrap_or("");
                                    on_message(event_type, ev);
                                }
                            } else if v.is_object() {
                                let event_type = v.get("event_type").and_then(|x| x.as_str()).unwrap_or("");
                                on_message(event_type, &v);
                            }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = ping.tick() => {
                let _ = write.send(Message::Text("PING".to_string())).await;
            }
        }
    }
    Ok(())
}
