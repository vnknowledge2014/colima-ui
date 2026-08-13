import { describe, it, expect } from "vitest";
import {
  newIssueUrl,
  REPO_URL,
  isSafeExternalUrl,
  isAllowedAnnouncementLink,
} from "./external-links";

/**
 * The bug reporter's last step is a URL. Clicking it needs a browser, but whether
 * it is *correct* — right repo, prefilled, and not silently dropped by GitHub for
 * being too long — is decidable here.
 */
describe("newIssueUrl", () => {
  it("targets the project's issue tracker", () => {
    const url = new URL(newIssueUrl("t", "b"));
    expect(`${url.origin}${url.pathname}`).toBe(`${REPO_URL}/issues/new`);
  });

  it("prefills both fields", () => {
    const url = new URL(newIssueUrl("[bug] it broke", "### What happened\n\nsteps"));
    expect(url.searchParams.get("title")).toBe("[bug] it broke");
    expect(url.searchParams.get("body")).toBe("### What happened\n\nsteps");
  });

  it("escapes characters that would otherwise cut the query short", () => {
    // A signature routinely contains `&`, `#` and spaces. Unescaped, everything
    // after the first `&` would arrive as a separate parameter and be dropped.
    const signature = "failed: a & b #3 (100%) at c/d?e=f";
    const url = new URL(newIssueUrl(`[bug] ${signature}`, "body"));
    expect(url.searchParams.get("title")).toBe(`[bug] ${signature}`);
    expect(url.searchParams.get("body")).toBe("body");
  });

  it("keeps the whole URL inside what GitHub accepts", () => {
    // GitHub rejects a query string past roughly 8 KB. This is why the bundle is
    // copied rather than embedded — a regression that stuffed it into the body
    // would produce a link that simply fails to open.
    const title = "[bug] " + "x".repeat(200);
    const body = "### What happened\n\n".repeat(20);
    expect(newIssueUrl(title, body).length).toBeLessThan(8000);
  });

  it("survives an empty title and body", () => {
    const url = new URL(newIssueUrl("", ""));
    expect(url.searchParams.get("title")).toBe("");
    expect(url.searchParams.get("body")).toBe("");
  });
});

/**
 * An external link leaves the webview and is resolved by the operating system,
 * where the app's CSP does not apply and nothing can be taken back. Every URL
 * passed here used to be a compile-time constant; announcement links are not.
 */
describe("isSafeExternalUrl", () => {
  it("accepts https", () => {
    expect(isSafeExternalUrl("https://github.com/vnknowledge2014/colima-ui")).toBe(true);
  });

  it("refuses schemes the OS would act on", () => {
    for (const url of [
      "javascript:alert(1)",
      "file:///etc/passwd",
      "data:text/html,<script>alert(1)</script>",
      "vscode://install?x=y",
      "http://github.com",
    ]) {
      expect(isSafeExternalUrl(url)).toBe(false);
    }
  });

  it("refuses anything that is not a URL", () => {
    expect(isSafeExternalUrl("")).toBe(false);
    expect(isSafeExternalUrl("github.com")).toBe(false);
  });
});

describe("isAllowedAnnouncementLink", () => {
  it("allows the vendor's own hosts", () => {
    expect(isAllowedAnnouncementLink("https://github.com/x/y/releases")).toBe(true);
    expect(isAllowedAnnouncementLink("https://www.github.com/x/y")).toBe(true);
  });

  it("refuses a host that is merely similar", () => {
    // The check is on the parsed hostname, so neither a prefix nor a suffix nor
    // a userinfo trick makes something look like an allowed host.
    expect(isAllowedAnnouncementLink("https://github.com.evil.test/x")).toBe(false);
    expect(isAllowedAnnouncementLink("https://evil.test/github.com")).toBe(false);
    expect(isAllowedAnnouncementLink("https://github.com@evil.test/x")).toBe(false);
  });

  it("refuses credentials and a port on an allowed host", () => {
    expect(isAllowedAnnouncementLink("https://user:pw@github.com/x")).toBe(false);
    expect(isAllowedAnnouncementLink("https://github.com:1337/x")).toBe(false);
  });

  it("refuses a bad scheme even on an allowed host", () => {
    expect(isAllowedAnnouncementLink("javascript://github.com/%0aalert(1)")).toBe(false);
  });

  it("treats a missing link as no link", () => {
    expect(isAllowedAnnouncementLink(undefined)).toBe(false);
  });
});
