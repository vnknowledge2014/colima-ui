import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { consumeAuthCallbackError, handleAuthCallback, startOAuth } from "./account-oauth";
import {
  accountState,
  loadSession,
  signInWithCredentials,
  signOut,
  signUpWithCredentials,
} from "./account.svelte";
import { fetchEntitlement } from "./api/subscription";
import {
  PKCE_VERIFIER_KEY,
  SESSION_STORAGE_KEY,
  clearMemoryStorage,
  getSurrealAuth,
  isJwtExpired,
  keychainStorage,
} from "./surrealdb";

vi.mock("./external-links", () => ({
  openExternal: vi.fn().mockResolvedValue(true),
}));

vi.mock("./pro.svelte", () => ({
  loadEntitlement: vi.fn().mockResolvedValue(undefined),
  refreshEntitlement: vi.fn().mockResolvedValue(undefined),
}));

describe("SurrealDB Auth, Keychain Fallback & Entitlement", () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    clearMemoryStorage();
    accountState.user = null;
    accountState.loaded = false;
    accountState.busy = false;
    accountState.error = "";
    accountState.ephemeral = false;
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    vi.restoreAllMocks();
  });

  it("retains session and PKCE verifier in memoryStorage when OS keychain is unavailable", async () => {
    await keychainStorage.setItem(PKCE_VERIFIER_KEY, "verifier-123");
    expect(await keychainStorage.getItem(PKCE_VERIFIER_KEY)).toBe("verifier-123");

    await keychainStorage.removeItem(PKCE_VERIFIER_KEY);
    expect(await keychainStorage.getItem(PKCE_VERIFIER_KEY)).toBeNull();
  });

  it("signs up and signs in against SurrealDB /signup, /signin, and /sql $auth", async () => {
    globalThis.fetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/signup") || url.endsWith("/signin")) {
        return new Response(
          JSON.stringify({ code: 200, details: "Authentication succeeded", token: "jwt-token-abc" }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        );
      }
      if (url.endsWith("/sql")) {
        return new Response(
          JSON.stringify([
            {
              status: "OK",
              result: [
                {
                  id: "user:alice",
                  email: "alice@example.com",
                  name: "Alice",
                  avatar: "",
                  provider: "surrealdb",
                },
              ],
            },
          ]),
          { status: 200, headers: { "Content-Type": "application/json" } },
        );
      }
      return new Response("Not Found", { status: 404 });
    }) as typeof fetch;

    await loadSession();
    await signUpWithCredentials("alice@example.com", "Secret123!", "Alice");

    expect(accountState.user).toEqual({
      id: "user:alice",
      email: "alice@example.com",
      name: "Alice",
      avatar: "",
      provider: "surrealdb",
    });
    expect(await keychainStorage.getItem(SESSION_STORAGE_KEY)).toContain("jwt-token-abc");

    await signOut();
    expect(accountState.user).toBeNull();
    expect(await keychainStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();

    await signInWithCredentials("alice@example.com", "Secret123!");
    expect(accountState.user?.id).toBe("user:alice");
  });

  it("preserves cached session identity when SurrealDB is offline during getSession", async () => {
    await keychainStorage.setItem(
      SESSION_STORAGE_KEY,
      JSON.stringify({
        access_token: "offline-jwt",
        user: {
          id: "user:offline",
          email: "offline@example.com",
          user_metadata: { full_name: "Offline User" },
          app_metadata: { provider: "surrealdb" },
        },
      }),
    );

    globalThis.fetch = vi.fn().mockRejectedValue(new Error("ECONNREFUSED")) as typeof fetch;

    const client = getSurrealAuth();
    const res = await client!.auth.getSession();
    expect(res.data.session?.user.id).toBe("user:offline");
  });

  it("evicts expired or 401-rejected sessions during getSession", async () => {
    const expiredPayload = btoa(JSON.stringify({ ID: "user:expired", exp: 1700000000 }));
    const expiredJwt = `eyJhbGciOiJIUzI1NiJ9.${expiredPayload}.sig`;
    expect(isJwtExpired(expiredJwt, 1800000000000)).toBe(true);

    await keychainStorage.setItem(
      SESSION_STORAGE_KEY,
      JSON.stringify({
        access_token: expiredJwt,
        user: { id: "user:expired", email: "exp@example.com" },
      }),
    );

    const client = getSurrealAuth()!;
    const res1 = await client.auth.getSession();
    expect(res1.data.session).toBeNull();
    expect(await keychainStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();

    // Non-expired token rejected by server with HTTP 401 must also be evicted
    await keychainStorage.setItem(
      SESSION_STORAGE_KEY,
      JSON.stringify({
        access_token: "revoked-jwt",
        user: { id: "user:revoked", email: "rev@example.com" },
      }),
    );
    globalThis.fetch = vi.fn(async () => new Response("Unauthorized", { status: 401 })) as typeof fetch;
    const res2 = await client.auth.getSession();
    expect(res2.data.session).toBeNull();
    expect(await keychainStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
  });

  it("completes PKCE OAuth round-trip using in-memory verifier fallback and surfaces state errors", async () => {
    let openedUrl = "";
    const { openExternal } = await import("./external-links");
    vi.mocked(openExternal).mockImplementation(async (u: string) => {
      openedUrl = u;
      return true;
    });

    globalThis.fetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/oauth/token")) {
        return new Response(JSON.stringify({ code: 200, token: "oauth-jwt" }), { status: 200 });
      }
      if (url.endsWith("/sql")) {
        return new Response(
          JSON.stringify([
            {
              status: "OK",
              result: [
                {
                  id: "user:github1",
                  email: "gh@example.com",
                  name: "GH User",
                  avatar: "",
                  provider: "github",
                },
              ],
            },
          ]),
          { status: 200 },
        );
      }
      return new Response("Not Found", { status: 404 });
    }) as typeof fetch;

    await startOAuth("github");
    expect(openedUrl).toContain("/oauth/authorize?");
    const authorizeParams = new URL(openedUrl).searchParams;
    expect(authorizeParams.get("code_challenge_method")).toBe("S256");
    expect(authorizeParams.get("code_challenge")).toBeTruthy();
    const redirectTo = authorizeParams.get("redirect_to")!;
    const state = new URL(redirectTo).searchParams.get("state")!;
    expect(state).toHaveLength(32);

    const ok = await handleAuthCallback(`colimaui://auth-callback?state=${state}&code=authcode99`);
    expect(ok).toBe(true);

    // State mismatch on replay must fail and record a callback error
    const replayOk = await handleAuthCallback(
      `colimaui://auth-callback?state=${state}&code=authcode99`,
    );
    expect(replayOk).toBe(false);
    expect(consumeAuthCallbackError()).toContain("state mismatch");
  });

  it("queries SurrealDB subscription table for entitlement and returns null when offline", async () => {
    globalThis.fetch = vi.fn(async () => {
      return new Response(
        JSON.stringify([
          {
            status: "OK",
            result: [
              {
                user_id: "user:alice",
                entitled: true,
                tier: "pro_teams",
                seats_total: 5,
                seats_claimed: 2,
                expires_at: "2030-01-01T00:00:00Z",
              },
            ],
          },
        ]),
        { status: 200 },
      );
    }) as typeof fetch;

    const payload = await fetchEntitlement("jwt-token-abc");
    expect(payload).toEqual({
      user_id: "user:alice",
      entitled: true,
      tier: "pro_teams",
      seats_total: 5,
      seats_claimed: 2,
      expires_at: "2030-01-01T00:00:00Z",
    });

    // Offline must return null so cached entitlement is never downgraded
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("Network down")) as typeof fetch;
    expect(await fetchEntitlement("jwt-token-abc")).toBeNull();
  });
});
