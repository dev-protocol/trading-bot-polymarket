import {
  ClobClient,
  ApiKeyCreds,
  Side,
  OrderType,
  type ClobSigner,
} from "@polymarket/clob-client";
import { Wallet } from "ethers";
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "fs";
import { join } from "path";
import {
  privateKey,
  clobHost,
  signatureType,
  funderAddress,
  dryRun,
} from "./config.js";

interface StoredAuth {
  api_key: string;
  api_secret: string;
  api_passphrase: string;
}

export interface UserAuth {
  apiKey: string;
  secret: string;
  passphrase: string;
}

export interface OrderExecutor {
  placeOrder(
    tokenId: string,
    price: number,
    size: number,
    side: string
  ): Promise<{ ok: boolean; status: string; orderId: string }>;
  cancel(orderId: string): Promise<boolean>;
}

const AUTH_DIR = "auth";
const AUTH_PATH = join(AUTH_DIR, "auth.json");

function loadCredentials(): ApiKeyCreds | null {
  if (!existsSync(AUTH_PATH)) return null;
  try {
    const raw = readFileSync(AUTH_PATH, "utf-8");
    const stored: StoredAuth = JSON.parse(raw);
    return {
      key: stored.api_key,
      secret: stored.api_secret,
      passphrase: stored.api_passphrase,
    };
  } catch {
    return null;
  }
}

function saveCredentials(creds: ApiKeyCreds): void {
  mkdirSync(AUTH_DIR, { recursive: true });
  const stored: StoredAuth = {
    api_key: creds.key,
    api_secret: creds.secret,
    api_passphrase: creds.passphrase,
  };
  writeFileSync(AUTH_PATH, JSON.stringify(stored, null, 2));
}

export function loadUserAuth(): UserAuth | null {
  const stored = loadCredentials();
  if (!stored) return null;
  return {
    apiKey: stored.key,
    secret: stored.secret,
    passphrase: stored.passphrase,
  };
}

async function buildClient(): Promise<ClobClient | null> {
  const pk = privateKey();
  if (!pk) return null;
  const host = clobHost();
  const signer = new Wallet(
    pk.startsWith("0x") ? pk : "0x" + pk
  ) as unknown as ClobSigner;
  const base = new ClobClient(host, 137, signer);
  let creds = loadCredentials();
  if (creds) {
    try {
      const client = new ClobClient(
        host,
        137,
        signer,
        creds,
        signatureType() as 0 | 1 | 2,
        funderAddress() ?? undefined,
        undefined,
        true
      );
      await client.getApiKeys();
      return client;
    } catch {
      creds = null;
    }
  }
  creds = await base.createOrDeriveApiKey();
  saveCredentials(creds);
  const client = new ClobClient(
    host,
    137,
    signer,
    creds,
    signatureType() as 0 | 1 | 2,
    funderAddress() ?? undefined,
    undefined,
    true
  );
  return client;
}

class StubExecutor implements OrderExecutor {
  async placeOrder(
    _tokenId: string,
    _price: number,
    size: number,
    side: string
  ): Promise<{ ok: boolean; status: string; orderId: string }> {
    console.log(
      `STUB place_order side=${side} size=${size} (set DRY_RUN=0 to enable live CLOB orders)`
    );
    return { ok: true, status: "stub", orderId: `stub-${side.toLowerCase()}` };
  }
  async cancel(_orderId: string): Promise<boolean> {
    return true;
  }
}

class FailingExecutor implements OrderExecutor {
  constructor(private message: string) {}
  async placeOrder(): Promise<{ ok: boolean; status: string; orderId: string }> {
    console.error("CLOB executor unavailable:", this.message);
    return { ok: false, status: "", orderId: "" };
  }
  async cancel(): Promise<boolean> {
    return false;
  }
}

class RealClobExecutor implements OrderExecutor {
  private client: ClobClient;
  constructor(client: ClobClient) {
    this.client = client;
  }
  async placeOrder(
    tokenId: string,
    price: number,
    size: number,
    side: string
  ): Promise<{ ok: boolean; status: string; orderId: string }> {
    try {
      const resp = await this.client.createAndPostOrder(
        {
          tokenID: tokenId,
          price,
          side: Side.BUY,
          size,
        },
        { tickSize: "0.001", negRisk: false },
        OrderType.GTC
      );
      const success = resp?.success !== false;
      const status = (resp?.status ?? "unknown") as string;
      const orderId = (resp?.orderID ?? resp?.order_id ?? "") as string;
      return { ok: success, status, orderId };
    } catch (err) {
      console.error("CLOB place_order failed", tokenId, price, size, err);
      return { ok: false, status: "", orderId: "" };
    }
  }
  async cancel(orderId: string): Promise<boolean> {
    try {
      await this.client.cancelOrder({ orderID: orderId });
      return true;
    } catch (err) {
      console.error("CLOB cancel failed", orderId, err);
      return false;
    }
  }
}

export async function defaultExecutor(): Promise<OrderExecutor> {
  if (dryRun()) return new StubExecutor();
  if (!privateKey())
    return new FailingExecutor("PRIVATE_KEY not set for live trading");
  const client = await buildClient();
  if (!client)
    return new FailingExecutor("CLOB client init failed");
  return new RealClobExecutor(client);
}
