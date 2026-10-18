/**
 * The Polar half of the entitlement oracle.
 *
 * Plain `fetch` against the documented REST API rather than the SDK: one
 * endpoint is needed, and a dependency that has to be pinned, audited and kept
 * working under Deno is a poor trade for one GET.
 *
 * `POLAR_ACCESS_TOKEN` is an organization-wide secret. It exists only in this
 * function's environment — never in the app bundle, never in the repo, never in
 * a log line.
 */

/** Sandbox unless explicitly told otherwise, mirroring the retired license client. */
export function apiBase(): string {
  return Deno.env.get("POLAR_ENV") === "production"
    ? "https://api.polar.sh"
    : "https://sandbox-api.polar.sh";
}

/** A subscription as it appears inside a customer state. */
export interface CustomerStateSubscription {
  id: string;
  status: string;
  product_id: string;
  current_period_end: string | null;
  cancel_at_period_end?: boolean;
  ends_at?: string | null;
  /** Seat count. Present only on seat-based products; absent is not an error. */
  seats?: number | null;
  quantity?: number | null;
}

export interface CustomerStateBenefitGrant {
  id: string;
  /** Not always present depending on benefit type; read defensively. */
  subscription_id?: string | null;
}

export interface CustomerState {
  id: string;
  /** "individual" | "team" — a customer becomes "team" on its first seat purchase. */
  type?: string;
  active_subscriptions: CustomerStateSubscription[];
  granted_benefits: CustomerStateBenefitGrant[];
}

/**
 * Three outcomes, and keeping them apart is the point of this type.
 *
 * `unreachable` is **not** `not_found`. A network failure must leave the
 * caller's cached entitlement standing; only a definite answer from Polar may
 * end someone's Pro. Collapsing these two into "no subscription" is the bug
 * that would downgrade paying customers whenever Polar had a bad minute.
 */
export type CustomerStateResult =
  | { kind: "ok"; state: CustomerState }
  | { kind: "not_found" }
  | { kind: "unreachable"; reason: string };

/**
 * Look up a customer by the external id we set at checkout — the Supabase user
 * id. Matching on that rather than email is what avoids the "I paid but it says
 * Free" class of failure when someone pays from a different address than they
 * signed in with.
 */
export async function getCustomerStateByExternalId(
  externalId: string,
): Promise<CustomerStateResult> {
  const token = Deno.env.get("POLAR_ACCESS_TOKEN");
  if (!token) {
    // Not configured is not "no subscription": saying "not entitled" here would
    // strip Pro from every paying customer the moment the secret went missing.
    return { kind: "unreachable", reason: "POLAR_ACCESS_TOKEN not set" };
  }

  const url = `${apiBase()}/v1/customers/external/${encodeURIComponent(externalId)}/state`;

  let res: Response;
  try {
    res = await fetch(url, {
      headers: { Authorization: `Bearer ${token}`, Accept: "application/json" },
    });
  } catch (e) {
    return { kind: "unreachable", reason: e instanceof Error ? e.message : "fetch failed" };
  }

  // 404 is a real answer: Polar has never seen this user, so they have not
  // bought anything. That is a definite "no", unlike the cases below.
  if (res.status === 404) return { kind: "not_found" };

  if (!res.ok) {
    // 401/403 (bad or revoked token) and 5xx are all "we could not find out".
    // None of them are evidence that the customer stopped paying.
    return { kind: "unreachable", reason: `Polar returned ${res.status}` };
  }

  try {
    const state = (await res.json()) as CustomerState;
    if (!Array.isArray(state?.active_subscriptions)) {
      return { kind: "unreachable", reason: "unexpected response shape" };
    }
    return { kind: "ok", state };
  } catch {
    return { kind: "unreachable", reason: "unparseable response" };
  }
}

/**
 * Verify a Polar webhook signature (standard-webhooks HMAC-SHA256).
 *
 * Unused today — the webhook is optional and not deployed — but kept beside the
 * client so that if anyone adds the endpoint later, the verification is already
 * here rather than something to remember. An unverified webhook endpoint is a
 * public "grant me a subscription" button.
 */
export async function verifyWebhookSignature(
  rawBody: string,
  headers: Headers,
): Promise<boolean> {
  const secret = Deno.env.get("POLAR_WEBHOOK_SECRET");
  if (!secret) return false;

  const id = headers.get("webhook-id");
  const timestamp = headers.get("webhook-timestamp");
  const signatureHeader = headers.get("webhook-signature");
  if (!id || !timestamp || !signatureHeader) return false;

  // Reject anything older than five minutes so a captured delivery cannot be
  // replayed indefinitely.
  const age = Math.abs(Date.now() / 1000 - Number(timestamp));
  if (!Number.isFinite(age) || age > 300) return false;

  const key = await crypto.subtle.importKey(
    "raw",
    base64ToBytes(secret.replace(/^whsec_/, "")),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const mac = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(`${id}.${timestamp}.${rawBody}`),
  );
  const expected = bytesToBase64(new Uint8Array(mac));

  // The header carries space-separated `v1,<sig>` entries.
  return signatureHeader
    .split(" ")
    .map((part) => part.split(",")[1])
    .some((sig) => sig !== undefined && timingSafeEqual(sig, expected));
}

/**
 * Returns `Uint8Array<ArrayBuffer>`, not the default `Uint8Array<ArrayBufferLike>`.
 *
 * `crypto.subtle.importKey` accepts a `BufferSource`, which requires a view over
 * a real `ArrayBuffer` — `ArrayBufferLike` also admits `SharedArrayBuffer` and is
 * rejected. Allocating the buffer explicitly pins the type; `Uint8Array.from`
 * does not.
 */
function base64ToBytes(b64: string): Uint8Array<ArrayBuffer> {
  const binary = atob(b64);
  const out = new Uint8Array(new ArrayBuffer(binary.length));
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}

function bytesToBase64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes));
}

/** Constant-time compare, so a wrong signature leaks nothing through timing. */
function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}
