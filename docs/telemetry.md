# Telemetry & Network Egress

What leaves this machine, when, and what a receiving server can infer from it.

**There is no analytics.** No PostHog, Sentry, Segment, Mixpanel, or equivalent
is present in either the frontend or the Rust backend — no SDK, no dependency,
no event pipeline. Nothing reports feature usage, crashes, or identity anywhere.

Every request below is either something a person asked for by clicking, or the
one background feed described first — which can be switched off.

## Automatic requests

### Announcements feed

The only request the app makes that nobody clicked.

| | |
|---|---|
| **URL** | `https://raw.githubusercontent.com/vnknowledge2014/colima-ui/main/announcements.json` |
| **Method** | `GET`, no body |
| **When** | At start-up and every six hours |
| **Off switch** | Settings → Notifications. Off means **no request is made**, not a discarded response |
| **Code** | `src-tauri/src/commands/announcements.rs` (`fetch_feed`) |

What GitHub's CDN can observe: the machine's **IP address**, the **time** of the
request, and the default `reqwest` User-Agent. That is the whole of it.

What is deliberately *not* sent:

- **No cookie store** is built for this client, so nothing persists across
  requests that could correlate two of them into a session.
- **No app version, locale, install id, or machine identifier.** Audience and
  version filtering happen *on the client*, after the full feed arrives — the
  server never learns which entries were relevant, only that someone fetched the
  same static file everyone fetches.
- **No content from the machine**: no container, image, volume, or project names.

Because the file is static and identical for every user, the request discloses no
more than visiting the repository page in a browser would.

### Update check

`https://github.com/vnknowledge2014/colima-ui/releases/latest/download/latest.json`
(`src-tauri/tauri.conf.json` → `updater.endpoints`), reached only when someone
presses **Check for updates** in Settings. There is no start-up check today, so
this is user-initiated in practice. Same disclosure profile as above: IP, time,
User-Agent.

> The updater is wired but **not operational**: release bundles are unsigned
> because CI never sets `TAURI_SIGNING_PRIVATE_KEY`, so signature verification
> fails on every client. See `docs/README.md` and the release workflow.

## User-initiated requests

These run only when a feature is used, and go to endpoints the user chose.

| Feature | Destination | Carries |
|---|---|---|
| AI chat | The configured provider (Anthropic, OpenAI, Google, Ollama, OpenRouter, Groq, Together, Mistral, DeepSeek, or a custom endpoint) | The conversation, plus any diagnostic output the user or the agent attached |
| Web search (AI tool) | The configured SearXNG instance, or DuckDuckGo as fallback | The search query |
| Page fetch (AI tool) | The URL the agent chose to read | Nothing beyond the request itself |
| Security scan | Trivy's own vulnerability database mirror | Nothing about local images — Trivy downloads data *in*; the image list is never sent out |
| Model pull | The Ollama registry | The model name |

Two consequences worth stating plainly:

- **AI chat is the one place local content genuinely leaves the machine.** Error
  text, logs, and command output routed to the agent go to a third-party LLM.
  `globalToast()` is the choke point where that text is redacted first
  (`src/lib/redact.ts`), and `src-tauri/src/redact.rs` covers the Rust side.
  Redaction masks credential shapes and home-directory account segments — it is
  not a guarantee that arbitrary paths or payloads are scrubbed.
- **Security scanning sends nothing about your images.** The score is computed
  locally, and base-image alternatives come from `resources/catalog/v1.json`
  shipped inside the app, not from a lookup service.

## Local-only data

Never transmitted, stored on this machine only:

| Data | Location |
|---|---|
| Knowledge bank, settings, AI provider credentials, presets | `~/.colima-ui/knowledge.db` |
| Activity record and metric samples | same SQLite database |
| Notification and transfer history | in memory, session-scoped, never persisted |
