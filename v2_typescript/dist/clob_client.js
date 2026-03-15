import { ClobClient, Side, OrderType, } from "@polymarket/clob-client";
import { Wallet } from "ethers";
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "fs";
import { join } from "path";
import { privateKey, clobHost, signatureType, funderAddress, dryRun, } from "./config.js";
const AUTH_DIR = "auth";
const AUTH_PATH = join(AUTH_DIR, "auth.json");
function loadCredentials() {
    if (!existsSync(AUTH_PATH))
        return null;
    try {
        const raw = readFileSync(AUTH_PATH, "utf-8");
        const stored = JSON.parse(raw);
        return {
            key: stored.api_key,
            secret: stored.api_secret,
            passphrase: stored.api_passphrase,
        };
    }
    catch {
        return null;
    }
}
function saveCredentials(creds) {
    mkdirSync(AUTH_DIR, { recursive: true });
    const stored = {
        api_key: creds.key,
        api_secret: creds.secret,
        api_passphrase: creds.passphrase,
    };
    writeFileSync(AUTH_PATH, JSON.stringify(stored, null, 2));
}
export function loadUserAuth() {
    const stored = loadCredentials();
    if (!stored)
        return null;
    return {
        apiKey: stored.key,
        secret: stored.secret,
        passphrase: stored.passphrase,
    };
}
async function buildClient() {
    const pk = privateKey();
    if (!pk)
        return null;
    const host = clobHost();
    const signer = new Wallet(pk.startsWith("0x") ? pk : "0x" + pk);
    const base = new ClobClient(host, 137, signer);
    let creds = loadCredentials();
    if (creds) {
        try {
            const client = new ClobClient(host, 137, signer, creds, signatureType(), funderAddress() ?? undefined, undefined, true);
            await client.getApiKeys();
            return client;
        }
        catch {
            creds = null;
        }
    }
    creds = await base.createOrDeriveApiKey();
    saveCredentials(creds);
    const client = new ClobClient(host, 137, signer, creds, signatureType(), funderAddress() ?? undefined, undefined, true);
    return client;
}
class StubExecutor {
    async placeOrder(_tokenId, _price, size, side) {
        console.log(`STUB place_order side=${side} size=${size} (set DRY_RUN=0 to enable live CLOB orders)`);
        return { ok: true, status: "stub", orderId: `stub-${side.toLowerCase()}` };
    }
    async cancel(_orderId) {
        return true;
    }
}
class FailingExecutor {
    message;
    constructor(message) {
        this.message = message;
    }
    async placeOrder() {
        console.error("CLOB executor unavailable:", this.message);
        return { ok: false, status: "", orderId: "" };
    }
    async cancel() {
        return false;
    }
}
class RealClobExecutor {
    client;
    constructor(client) {
        this.client = client;
    }
    async placeOrder(tokenId, price, size, side) {
        try {
            const resp = await this.client.createAndPostOrder({
                tokenID: tokenId,
                price,
                side: Side.BUY,
                size,
            }, { tickSize: "0.001", negRisk: false }, OrderType.GTC);
            const success = resp?.success !== false;
            const status = (resp?.status ?? "unknown");
            const orderId = (resp?.orderID ?? resp?.order_id ?? "");
            return { ok: success, status, orderId };
        }
        catch (err) {
            console.error("CLOB place_order failed", tokenId, price, size, err);
            return { ok: false, status: "", orderId: "" };
        }
    }
    async cancel(orderId) {
        try {
            await this.client.cancelOrder({ orderID: orderId });
            return true;
        }
        catch (err) {
            console.error("CLOB cancel failed", orderId, err);
            return false;
        }
    }
}
export async function defaultExecutor() {
    if (dryRun())
        return new StubExecutor();
    if (!privateKey())
        return new FailingExecutor("PRIVATE_KEY not set for live trading");
    const client = await buildClient();
    if (!client)
        return new FailingExecutor("CLOB client init failed");
    return new RealClobExecutor(client);
}
