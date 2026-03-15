//! Approve USDC and CTF allowances for Polymarket trading on Polygon.
//! Checks allowance first; only sends approval txs when not already approved.

use anyhow::Result;
use ethers::abi::{AbiDecode, AbiEncode};
use ethers::prelude::*;
use ethers::types::transaction::eip2718::TypedTransaction;
use std::sync::Arc;
use tracing_ethers::Rwsmap;
use tracing::info;


const CHAIN_ID: u64 = 137;
const USDC_E: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
const CTF: &str = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
const CTF_EXCHANGE: &str = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";
const NEG_RISK_CTF_EXCHANGE: &str = "0xC5d563A36AE78145C45a50134d48A1215220f80a";
const NEG_RISK_ADAPTER: &str = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296";

fn min_approved_amount() -> U256 {
    U256::MAX / 2 + 1
}

fn encode_approve(spender: Address, amount: U256) -> Vec<u8> {
    let mut data = vec![0x09, 0x5e, 0xa7, 0xb3];
    data.extend_from_slice(&(spender, amount).encode());
    data
}

fn encode_allowance(owner: Address, spender: Address) -> Vec<u8> {
    let mut data = vec![0xdd, 0x62, 0xed, 0x3e];
    data.extend_from_slice(&(owner, spender).encode());
    data
}

fn encode_set_approval_for_all(operator: Address, approved: bool) -> Vec<u8> {
    let mut data = vec![0xa2, 0x2c, 0xb4, 0x65];
    data.extend_from_slice(&(operator, approved).encode());
    data
}

fn encode_is_approved_for_all(account: Address, operator: Address) -> Vec<u8> {
    let mut data = vec![0xe9, 0x85, 0xe9, 0xc5];
    data.extend_from_slice(&(account, operator).encode());
    data
}
fn parse_address(s: &str) -> Result<Address> {
    let s = s.trim().trim_start_matches("0x");
    let bytes = hex::decode(s).map_err(|e| anyhow::anyhow!("invalid address hex: {}", e))?;
    if bytes.len() != 20 {
        anyhow::bail!("address must be 20 bytes");
    }
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(Address::from(arr))
}

pub async fn wallet_address_and_balance(private_key: &str, rpc_url: &str) -> Result<(Address, U256)> {
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let wallet = private_key
        .trim()
        .strip_prefix("0x")
        .unwrap_or(private_key)
        .parse::<LocalWallet>()
        .map_err(|e| anyhow::anyhow!("invalid private key: {}", e))?;
    let addr = wallet.address();
    let balance = provider.get_balance(addr, None).await?;
    Ok((addr, balance))
}

async fn check_usdc_allowance(
    provider: &Provider<Http>,
    owner: Address,
    usdc: Address,
    spender: Address,
) -> Result<U256> {
    let tx = TransactionRequest::default()
        .from(owner)
        .to(usdc)
        .data(encode_allowance(owner, spender));
    let out = provider.call(&tx.into(), None).await?;
    U256::decode(out).map_err(|e| anyhow::anyhow!("decode allowance: {}", e))
}

async fn check_ctf_approved_for_all(
    provider: &Provider<Http>,
    account: Address,
    ctf: Address,
    operator: Address,
) -> Result<bool> {
    let tx = TransactionRequest::default()
        .from(account)
        .to(ctf)
        .data(encode_is_approved_for_all(account, operator));
    let out = provider.call(&tx.into(), None).await?;
    let rwsmap = Rwsmap::sum_of_tracing_ethers().await;
    bool::decode(out).map_err(|e| anyhow::anyhow!("decode isApprovedForAll: {}", e))
}

pub async fn approve_allowance(private_key: &str, rpc_url: &str, include_neg_risk: bool) -> Result<()> {
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let wallet = private_key
        .trim()
        .strip_prefix("0x")
        .unwrap_or(private_key)
        .parse::<LocalWallet>()
        .map_err(|e| anyhow::anyhow!("invalid private key: {}", e))?;
    let wallet = wallet.with_chain_id(CHAIN_ID);
    let client = SignerMiddleware::new(provider.clone(), wallet);
    let client = Arc::new(client);

    let max_u256 = U256::max_value();
    let from = client.address();
    let min_approved = min_approved_amount();

    let usdc = parse_address(USDC_E)?;
    let ctf_addr = parse_address(CTF)?;
    let ctf_exchange_addr = parse_address(CTF_EXCHANGE)?;

    let allowance_ctf = check_usdc_allowance(&provider, from, usdc, ctf_addr).await?;
    if allowance_ctf >= min_approved {
        info!("USDC allowance(CTF) already sufficient ({})", allowance_ctf);
    } else {
        let tx = TransactionRequest::default()
            .from(from)
            .to(usdc)
            .data(encode_approve(ctf_addr, max_u256))
            .gas(100_000u64);
        let typed = TypedTransaction::Legacy(tx.into());
        let pending = client.send_transaction(typed, None).await?;
        let receipt = pending.await?.ok_or_else(|| anyhow::anyhow!("no receipt for USDC approve"))?;
        info!("USDC approve(CTF) tx: {:?} success={}", receipt.transaction_hash, receipt.status == Some(1.into()));
    }

    let ctf_ok = check_ctf_approved_for_all(&provider, from, ctf_addr, ctf_exchange_addr).await?;
    if ctf_ok {
        info!("CTF already setApprovalForAll(CTF_EXCHANGE)");
    } else {
        let tx2 = TransactionRequest::default()
            .from(from)
            .to(ctf_addr)
            .data(encode_set_approval_for_all(ctf_exchange_addr, true))
            .gas(100_000u64);
        let typed2 = TypedTransaction::Legacy(tx2.into());
        let pending2 = client.send_transaction(typed2, None).await?;
        let receipt2 = pending2.await?.ok_or_else(|| anyhow::anyhow!("no receipt for CTF setApprovalForAll"))?;
        info!("CTF setApprovalForAll(CTF_EXCHANGE) tx: {:?} success={}", receipt2.transaction_hash, receipt2.status == Some(1.into()));
    }

    if include_neg_risk {
        let neg_ctf = parse_address(NEG_RISK_CTF_EXCHANGE)?;
        let neg_adapter = parse_address(NEG_RISK_ADAPTER)?;

        let allow_neg_ctf = check_usdc_allowance(&provider, from, usdc, neg_ctf).await?;
        if allow_neg_ctf >= min_approved {
            info!("USDC allowance(NEG_RISK_CTF_EXCHANGE) already sufficient");
        } else {
            let tx3 = TransactionRequest::default()
                .from(from)
                .to(usdc)
                .data(encode_approve(neg_ctf, max_u256))
                .gas(100_000u64);
            let typed3 = TypedTransaction::Legacy(tx3.into());
            let pending3 = client.send_transaction(typed3, None).await?;
            let r3 = pending3.await?.ok_or_else(|| anyhow::anyhow!("no receipt"))?;
            info!("USDC approve(NEG_RISK_CTF_EXCHANGE) tx: {:?}", r3.transaction_hash);
        }

        let ctf_neg_ok = check_ctf_approved_for_all(&provider, from, ctf_addr, neg_ctf).await?;
        if ctf_neg_ok {
            info!("CTF already setApprovalForAll(NEG_RISK_CTF_EXCHANGE)");
        } else {
            let tx4 = TransactionRequest::default()
                .from(from)
                .to(ctf_addr)
                .data(encode_set_approval_for_all(neg_ctf, true))
                .gas(100_000u64);
            let typed4 = TypedTransaction::Legacy(tx4.into());
            let pending4 = client.send_transaction(typed4, None).await?;
            let r4 = pending4.await?.ok_or_else(|| anyhow::anyhow!("no receipt"))?;
            info!("CTF setApprovalForAll(NEG_RISK_CTF_EXCHANGE) tx: {:?}", r4.transaction_hash);
        }

        let allow_adapter = check_usdc_allowance(&provider, from, usdc, neg_adapter).await?;
        if allow_adapter >= min_approved {
            info!("USDC allowance(NEG_RISK_ADAPTER) already sufficient");
        } else {
            let tx5 = TransactionRequest::default()
                .from(from)
                .to(usdc)
                .data(encode_approve(neg_adapter, max_u256))
                .gas(100_000u64);
            let typed5 = TypedTransaction::Legacy(tx5.into());
            let pending5 = client.send_transaction(typed5, None).await?;
            let r5 = pending5.await?.ok_or_else(|| anyhow::anyhow!("no receipt"))?;
            info!("USDC approve(NEG_RISK_ADAPTER) tx: {:?}", r5.transaction_hash);
        }

        let ctf_adapter_ok = check_ctf_approved_for_all(&provider, from, ctf_addr, neg_adapter).await?;
        if ctf_adapter_ok {
            info!("CTF already setApprovalForAll(NEG_RISK_ADAPTER)");
        } else {
            let tx6 = TransactionRequest::default()
                .from(from)
                .to(ctf_addr)
                .data(encode_set_approval_for_all(neg_adapter, true))
                .gas(100_000u64);
            let typed6 = TypedTransaction::Legacy(tx6.into());
            let pending6 = client.send_transaction(typed6, None).await?;
            let r6 = pending6.await?.ok_or_else(|| anyhow::anyhow!("no receipt"))?;
            info!("CTF setApprovalForAll(NEG_RISK_ADAPTER) tx: {:?}", r6.transaction_hash);
        }
    }

    info!("Allowance check/approval done");
    Ok(())
}
