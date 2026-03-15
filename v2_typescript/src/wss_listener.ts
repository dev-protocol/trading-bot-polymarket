import WebSocket from "ws";
import { DEFAULT_MARKET_WSS, DEFAULT_USER_WSS } from "./config.js";
import type { UserAuth } from "./clob_client.js";

const PING_INTERVAL_MS = 10_000;

export function buildMarketSubscribe(assetIds: string[]): string {
  return JSON.stringify({
    assets_ids: assetIds,
    type: "market",
    custom_feature_enabled: true,
  });
}

export function buildUserSubscribe(auth: UserAuth, markets: string[]): string {
  return JSON.stringify({
    auth: {
      apiKey: auth.apiKey,
      secret: auth.secret,
      passphrase: auth.passphrase,
    },
    markets,
    type: "user",
  });
}

function parseMessage(
  data: string,
  onMessage: (eventType: string, ev: Record<string, unknown>) => void
): void {
  if (data.trim() === "PONG") return;
  try {
    const v = JSON.parse(data) as unknown;
    if (Array.isArray(v)) {
      for (const ev of v) {
        const obj = ev as Record<string, unknown>;
        const eventType = (obj.event_type as string) ?? "";
        onMessage(eventType, obj);
      }
    } else if (v && typeof v === "object") {
      const obj = v as Record<string, unknown>;
      const eventType = (obj.event_type as string) ?? "";
      onMessage(eventType, obj);
    }
  } catch {
    //
  }
}

export function runMarketWssUntil(
  url: string,
  subscribePayload: string,
  runForMs: number,
  onMessage: (eventType: string, ev: Record<string, unknown>) => void
): Promise<void> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    const deadline = setTimeout(() => {
      ws.close();
      resolve();
    }, runForMs);
    ws.on("open", () => {
      console.log("Market WSS connected:", url);
      ws.send(subscribePayload);
    });
    ws.on("message", (data: Buffer) => {
      parseMessage(data.toString(), onMessage);
    });
    ws.on("close", () => {
      clearTimeout(deadline);
      resolve();
    });
    ws.on("error", (err: unknown) => {
      clearTimeout(deadline);
      reject(err);
    });
    const pingInterval = setInterval(() => {
      if (ws.readyState === WebSocket.OPEN) ws.send("PING");
    }, PING_INTERVAL_MS);
    ws.on("close", () => clearInterval(pingInterval));
  });
}

export function runUserWssUntil(
  url: string,
  subscribePayload: string,
  runForMs: number,
  onMessage: (eventType: string, ev: Record<string, unknown>) => void
): Promise<void> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    const deadline = setTimeout(() => {
      ws.close();
      resolve();
    }, runForMs);
    ws.on("open", () => {
      console.log("User WSS connected:", url);
      ws.send(subscribePayload);
    });
    ws.on("message", (data: Buffer) => {
      parseMessage(data.toString(), onMessage);
    });
    ws.on("close", () => {
      clearTimeout(deadline);
      resolve();
    });
    ws.on("error", (err: unknown) => {
      clearTimeout(deadline);
      reject(err);
    });
    const pingInterval = setInterval(() => {
      if (ws.readyState === WebSocket.OPEN) ws.send("PING");
    }, PING_INTERVAL_MS);
    ws.on("close", () => clearInterval(pingInterval));
  });
}
