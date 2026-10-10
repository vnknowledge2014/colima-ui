import { describe, it, expect } from "vitest";
import { redact, redactError } from "./redact";

describe("redact", () => {
  it("redacts a Gemini key in a query string", () => {
    const err =
      "HTTP Error 400: https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:streamGenerateContent?key=AIzaSyC1234567890abcdefghijklmnop";
    const out = redact(err);
    expect(out).not.toContain("AIzaSyC1234567890abcdefghijklmnop");
    expect(out).toContain("key=<redacted>");
  });

  it("anonymises the account name in a stack-trace path", () => {
    // crashReporter sends stack traces onward, and those carry absolute paths.
    const out = redact("at loadConfig (/Users/longnd/app/src/lib/config.ts:12:5)");
    expect(out).not.toContain("longnd");
    expect(out).toContain("/app/src/lib/config.ts:12:5");
  });

  it("leaves shared directories named correctly", () => {
    const msg = "reading /Users/Shared/config.yaml";
    expect(redact(msg)).toBe(msg);
  });

  it("redacts an AWS access key id and a JWT", () => {
    expect(redact("creds AKIAIOSFODNN7EXAMPLE here")).not.toContain("AKIAIOSFODNN7EXAMPLE");
    const jwt =
      "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    expect(redact(`token ${jwt} expired`)).not.toContain(jwt);
  });

  it("redacts an unknown provider's key by position", () => {
    const out = redact("GET https://example.invalid/v1/models?api_key=zzzz-not-a-known-shape-9999 failed");
    expect(out).not.toContain("zzzz-not-a-known-shape-9999");
  });

  it("redacts bearer tokens", () => {
    const out = redact("headers: {authorization: Bearer sk-proj-abcdefghijklmnopqrstuvwxyz}");
    expect(out).not.toContain("sk-proj-abcdefghijklmnopqrstuvwxyz");
  });

  it("redacts header-style api keys", () => {
    const out = redact('{"x-api-key": "sk-ant-api03-AAAABBBBCCCCDDDDEEEE"}');
    expect(out).not.toContain("sk-ant-api03-AAAABBBBCCCCDDDDEEEE");
  });

  it("redacts known key shapes anywhere in the string", () => {
    const keys = [
      "sk-abcdefghijklmnopqrstuvwxyz123456",
      "AIzaSyAbCdEfGhIjKlMnOpQrStUvWxYz01234",
      "gsk_abcdefghijklmnopqrstuvwxyz0123",
      "hf_abcdefghijklmnopqrst",
      "ghp_abcdefghijklmnopqrst",
    ];
    for (const key of keys) {
      expect(redact(`failed near ${key} while running`)).not.toContain(key);
    }
  });

  it("redacts non-bearer Authorization schemes", () => {
    const out = redact("Authorization: Basic dXNlcjpwYXNzd29yZA==");
    expect(out).not.toContain("dXNlcjpwYXNzd29yZA==");
  });

  it("redacts the app's own API token", () => {
    const token = `colima-${"a1b2c3d4".repeat(8)}`;
    expect(redact(`GET /api/events?token=${token} returned 401`)).not.toContain(token);
  });

  it("preserves Docker image digests", () => {
    // Redacting these would break diagnostics for everyone to protect a
    // minority of providers.
    const digest = "sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    expect(redact(`image ${digest} not found`)).toContain(digest);
  });

  it("leaves ordinary errors untouched", () => {
    const msg = "Cannot connect to the Docker daemon at unix:///var/run/docker.sock";
    expect(redact(msg)).toBe(msg);
  });

  it("is stable across repeated calls (no regex lastIndex leakage)", () => {
    const input = "a?key=AIzaSyAbCdEfGhIjKlMnOpQrStUvWxYz01234";
    expect(redact(input)).toBe(redact(input));
  });
});

describe("redactError", () => {
  it("handles Error instances", () => {
    const out = redactError(new Error("boom ?key=AIzaSyAbCdEfGhIjKlMnOpQrStUvWxYz01234"));
    expect(out).not.toContain("AIzaSyAbCdEfGhIjKlMnOpQrStUvWxYz01234");
  });

  it("handles plain strings", () => {
    expect(redactError("Bearer sk-abcdefghijklmnopqrstuvwx")).not.toContain(
      "sk-abcdefghijklmnopqrstuvwx",
    );
  });

  it("handles non-Error objects", () => {
    const out = redactError({ message: "sk-abcdefghijklmnopqrstuvwx" });
    expect(out).not.toContain("sk-abcdefghijklmnopqrstuvwx");
  });
});
