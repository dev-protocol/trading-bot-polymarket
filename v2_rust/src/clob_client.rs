//! CLOB: native Rust order execution + user-channel auth.

use crate::config;
use alloy::signers::Signer as _;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{Context, Result};
use polymarket_client_sdk::auth::{Credentials, ExposeSecret, Normal, Uuid, state::Authenticated};
use polymarket_client_sdk::clob::types::{OrderStatusType, OrderType, Side, SignatureType};
use polymarket_client_sdk::clob::{Client, Config};
use polymarket_client_sdk::types::{Address, Decimal, U256};
use polymarket_client_sdk::POLYGON;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::task::block_in_place;

type AuthClient = Client<Authenticated<Normal>>;

/// Order executor: place BUY orders and cancel orders.
pub trait OrderExecutor: Send + Sync {
    fn place_order(
        &self,
        token_id: &str,
        price: f64,
        size: f64,
        side: &str,
    ) -> (bool, String, String);
    fn cancel(&self, order_id: &str) -> bool;
}

/// Stub executor for DRY_RUN.
pub struct StubExecutor;

impl OrderExecutor for StubExecutor {
    fn place_order(
        &self,
        _token_id: &str,
        _price: f64,
        size: f64,
        side: &str,
    ) -> (bool, String, String) {
        tracing::info!(
            "STUB place_order side={} size={} (set DRY_RUN=0 to enable live Rust CLOB orders)",
            side,
            size
        );
        (true, "stub".to_string(), format!("stub-{}", side.to_lowercase()))
    }

    fn cancel(&self, order_id: &str) -> bool {
        tracing::info!("STUB cancel order_id={}", order_id);
        true
    }
}

/// Failing executor for live mode when auth or client init is unavailable.
pub struct FailingExecutor {
    message: String,
}

impl FailingExecutor {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl OrderExecutor for FailingExecutor {
    fn place_order(
        &self,
        _token_id: &str,
        _price: f64,
        _size: f64,
        _side: &str,
    ) -> (bool, String, String) {
        tracing::error!("Real Rust CLOB executor unavailable: {}", self.message);
        (false, String::new(), String::new())
    }

    fn cancel(&self, _order_id: &str) -> bool {
        tracing::error!("Real Rust CLOB executor unavailable: {}", self.message);
        false
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredAuth {
    api_key: String,
    api_secret: String,
    api_passphrase: String,
}

/// User channel WSS auth: api_key, secret, passphrase. Same keys as CLOB API.
#[derive(Clone, Debug)]
pub struct UserAuth {
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
}

/// Native Rust executor using `polymarket-client-sdk`.
pub struct RustClobExecutor {
    signer: PrivateKeySigner,
    client: AuthClient,
}

impl RustClobExecutor {
    pub fn new() -> Result<Self, String> {
        let handle = Handle::current();
        block_in_place(|| {
            handle
                .block_on(async { Self::new_async().await })
                .map_err(|e| e.to_string())
        })
    }

    async fn new_async() -> Result<Self> {
        let signer = build_signer()?;
        let creds_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("auth").join("auth.json");

        let client = if let Some(saved) = load_credentials(&creds_path)? {
            match authenticate_with(Some(saved.clone()), &signer).await {
                Ok(client) => client,
                Err(err) => {
                    tracing::warn!("Saved CLOB credentials rejected, regenerating: {}", err);
                    let fresh = create_or_derive_and_save(&creds_path, &signer).await?;
                    authenticate_with(Some(fresh), &signer).await?
                }
            }
        } else {
            let fresh = create_or_derive_and_save(&creds_path, &signer).await?;
            authenticate_with(Some(fresh), &signer).await?
        };

        // Validate immediately so live mode fails fast on bad auth.
        client.api_keys().await.context("CLOB auth validation failed")?;

        Ok(Self { signer, client })
    }
}

impl OrderExecutor for RustClobExecutor {
    fn place_order(
        &self,
        token_id: &str,
        price: f64,
        size: f64,
        side: &str,
    ) -> (bool, String, String) {
        let handle = Handle::current();
        let result: Result<(String, String)> = block_in_place(|| {
            handle.block_on(async {
                let token_id = U256::from_str(token_id).context("invalid token_id")?;
                let price = Decimal::from_str(&price.to_string()).context("invalid price")?;
                let size = Decimal::from_str(&size.to_string()).context("invalid size")?;

                let order = self
                    .client
                    .limit_order()
                    .token_id(token_id)
                    .price(price)
                    .size(size)
                    .side(Side::Buy)
                    .order_type(OrderType::GTC)
                    .build()
                    .await
                    .context("build limit order")?;

                let signed = self
                    .client
                    .sign(&self.signer, order)
                    .await
                    .context("sign order")?;

                let response = self
                    .client
                    .post_order(signed)
                    .await
                    .context("post order")?;

                if !response.success {
                    anyhow::bail!(
                        "{}",
                        response
                            .error_msg
                            .unwrap_or_else(|| "order rejected".to_string())
                    );
                }

                let status = match response.status {
                    OrderStatusType::Live => "live",
                    OrderStatusType::Matched => "matched",
                    OrderStatusType::Delayed => "delayed",
                    OrderStatusType::Unmatched => "unmatched",
                    OrderStatusType::Canceled => "canceled",
                    OrderStatusType::Unknown(_) => "unknown",
                    _ => "unknown",
                }
                .to_string();

                Ok((status, response.order_id))
            })
        });

        match result {
            Ok((status, order_id)) => (true, status, order_id),
            Err(err) => {
                tracing::error!(
                    "CLOB place_order failed side={} asset={} price={} size={} error={:#}",
                    side,
                    token_id,
                    price,
                    size,
                    err
                );
                (false, String::new(), String::new())
            }
        }
    }

    fn cancel(&self, order_id: &str) -> bool {
        let handle = Handle::current();
        let result: Result<bool> = block_in_place(|| {
            handle.block_on(async {
                let response = self
                    .client
                    .cancel_order(order_id)
                    .await
                    .context("cancel order")?;
                Ok(response.canceled.iter().any(|id| id == order_id))
            })
        });

        match result {
            Ok(ok) => ok,
            Err(err) => {
                tracing::error!("CLOB cancel failed order_id={} error={}", order_id, err);
                false
            }
        }
    }
}

fn build_signer() -> Result<PrivateKeySigner> {
    let private_key = config::private_key().context("PRIVATE_KEY not set")?;
    let signer = PrivateKeySigner::from_str(&private_key)?.with_chain_id(Some(POLYGON));
    Ok(signer)
}

fn config_builder() -> Config {
    Config::builder().use_server_time(true).build()
}

fn map_signature_type(value: u32) -> SignatureType {
    match value {
        1 => SignatureType::Proxy,
        2 => SignatureType::GnosisSafe,
        _ => SignatureType::Eoa,
    }
}

fn parse_address(value: &str) -> Result<Address> {
    Ok(Address::from_str(value)?)
}

async fn authenticate_with(
    credentials: Option<Credentials>,
    signer: &PrivateKeySigner,
) -> Result<AuthClient> {
    let host = config::clob_host();
    let base = Client::new(&host, config_builder())?;
    let mut builder = base.authentication_builder(signer);

    if let Some(credentials) = credentials {
        builder = builder.credentials(credentials);
    }

    let signature_type = map_signature_type(config::signature_type());
    builder = builder.signature_type(signature_type);

    if let Some(funder) = config::funder_address() {
        builder = builder.funder(parse_address(&funder)?);
    }

    let client = builder.authenticate().await?;
    Ok(client)
}

async fn create_or_derive_and_save(path: &Path, signer: &PrivateKeySigner) -> Result<Credentials> {
    let host = config::clob_host();
    let base = Client::new(&host, config_builder())?;
    let creds = base.create_or_derive_api_key(signer, None).await?;
    save_credentials(path, &creds)?;
    Ok(creds)
}

fn load_credentials(path: &Path) -> Result<Option<Credentials>> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let stored: StoredAuth = serde_json::from_str(&raw)?;
    let uuid = Uuid::parse_str(&stored.api_key).context("invalid api_key in auth.json")?;
    Ok(Some(Credentials::new(
        uuid,
        stored.api_secret,
        stored.api_passphrase,
    )))
}

fn save_credentials(path: &Path, creds: &Credentials) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let stored = StoredAuth {
        api_key: creds.key().to_string(),
        api_secret: creds.secret().expose_secret().to_string(),
        api_passphrase: creds.passphrase().expose_secret().to_string(),
    };
    std::fs::write(path, serde_json::to_string_pretty(&stored)?)?;
    Ok(())
}

/// Load user auth from auth/auth.json.
pub fn load_user_auth(auth_dir: &Path) -> Option<UserAuth> {
    let path = auth_dir.join("auth.json");
    let stored = load_credentials(&path).ok()??;
    Some(UserAuth {
        api_key: stored.key().to_string(),
        secret: stored.secret().expose_secret().to_string(),
        passphrase: stored.passphrase().expose_secret().to_string(),
    })
}

/// Use the real Rust CLOB executor in live mode and a stub in DRY_RUN.
pub fn default_executor() -> Arc<dyn OrderExecutor> {
    if config::dry_run() {
        return Arc::new(StubExecutor);
    }
    if config::private_key().is_none() {
        return Arc::new(FailingExecutor::new(
            "PRIVATE_KEY not set for live trading".to_string(),
        ));
    }
    match RustClobExecutor::new() {
        Ok(executor) => Arc::new(executor),
        Err(err) => Arc::new(FailingExecutor::new(err)),
    }
}
