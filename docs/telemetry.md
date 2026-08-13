# Telemetry & Crash Reporting

What ColimaUI can collect, what it never collects, and how the code enforces
that. Written to be checkable against the source: the event list below must
match the `TelemetryEvent` enum in `src-tauri/src/telemetry/events.rs` exactly.

Status: **telemetry collection is inert.** There is no server endpoint yet, so
nothing is transmitted regardless of consent. The consent model, the event
type, and crash-report redaction are in place; the network sink is deferred
until there is somewhere to send to. Crash-report redaction is active now.

## Principles

1. **Off by default.** Consent is asked once, defaults to declined, and is not
   asked again after a decline. No event is recorded without explicit consent.
2. **A closed vocabulary.** Events are a Rust enum with typed fields — there is
   no way to attach a free-form string. This is a barrier at the type level, not
   a promise: code physically cannot record what the enum does not name.
3. **You can see it first.** The app can show the exact events it would send
   before you agree.
4. **Crash reports are redacted.** Panic messages and stack traces are the one
   place free-form text is unavoidable, so every crash report passes through
   `redact()` before it is logged or shown.

## What may be collected (with consent)

Each corresponds to one `TelemetryEvent` variant:

| Event | Fields | Why |
|---|---|---|
| App started | app version, OS, CPU arch | Which platforms to support |
| Activation | (none) | Did the user reach the first working container |
| Feature used | feature id (enum, not free text) | Which features earn their keep |
| Error occurred | `ErrorCode` (from the error contract), not the message | Which failures are common |
| Pro gate reached | capability id | Which Pro features people want |
| Checkout opened | (none) | Funnel measurement |

## What is never collected

Named explicitly so the absence is a commitment, not an oversight:

- Container / image / volume / network names
- File paths
- Environment variables
- AI chat contents or prompts
- API keys or tokens
- IP addresses

The error event carries only the stable `ErrorCode` (e.g. `CommandFailed`),
never the error `detail`, because the detail can contain paths and resource
names.

## Image scanning sends nothing

The Security page reads images through the local container runtime and scores
them against a rule pack and a catalog compiled into the app. No image name,
package list, SBOM or score leaves the machine.

The one outbound connection in that feature belongs to Trivy, fetching its own
vulnerability database — the same request it would make from a terminal, to a
host that learns nothing about which images are being scanned.

There is deliberately no "how does image X score?" service. It would send the
user's image list to us, and that list says what they run and sometimes what
they are building before it is public.

## Announcements: the one connection that is not telemetry

ColimaUI reads a static JSON file of release notes, security advisories and
maintenance notices from
`https://raw.githubusercontent.com/vnknowledge2014/colima-ui/main/announcements.json`
— once at start-up, then every six hours.

Nothing is uploaded. It is a bare `GET` with no query parameters, no cookies, no
custom headers, no user id and no machine id, made by the Rust backend rather
than the webview (`src-tauri/src/commands/announcements.rs`).

It is documented here anyway, because *making a request at all* discloses
something:

| Disclosed to GitHub | Why it is unavoidable |
|---|---|
| Your IP address | Any HTTP request carries one |
| The time of the request | Server logs record it |

The app version is **not** disclosed: the HTTP client sets no user agent, and
which announcements apply to this version is decided locally after the file
arrives.

That is a small, real disclosure, and calling it "not telemetry because it is
only a read" would be a word game. So:

- **It can be switched off** — Settings → Notifications → "Show release notes and
  security advisories". Off means **no request is made**, not a request whose
  result is discarded.
- **Filtering happens locally.** Which announcements apply to this install (paid
  or free, app version) is decided after the file arrives. That question is never
  asked over the network, so the answer never travels.
- **Content is treated as untrusted.** Text is rendered as plain text, never as
  HTML, and a link is only offered when its URL is `https:` on a short host
  allowlist (`src/lib/external-links.ts`).

## Identity

A random UUID generated locally. It is **not** derived from the license, **not**
a machine fingerprint, and is not linked to any account.

## Crash reporting

Separate opt-in from telemetry. A Rust panic hook and the frontend error
handlers route the panic/error text through `redact()` first. This matters
because a panic can carry, verbatim, a request URL that includes an API key
passed as a query parameter — exactly what `redact()` strips.

## Deferred

- The network sink and batching queue (no endpoint exists yet).
- Auto-update, code signing, and notarization (need an Apple Developer ID and
  are tracked in the Phase 4 plan, not here).
