# Reading honeypot logs

## The one rule

A honeypot log entry is not noise to be filtered. Nothing legitimate connects
to a honeypot, so every entry means something touched a service that had no
business being touched. Treat volume as a signal, not as a problem to tune away.

This is the opposite of how you read application logs, and it is why honeypots
are worth running at all.

## Before you act on what you read

A honeypot log records an attacker's claims about themselves, and those claims
are cheap to fake.

- **Source addresses are frequently spoofed or borrowed.** The IP that hit you
  is often a compromised third party, not the operator. Do not retaliate, do not
  scan it back, and do not put it on a blocklist you cannot easily undo.
- **Files fetched into a honeypot are hostile files.** Cowrie stores what an
  attacker tried to download. Do not open, run, or unpack them on your own
  machine because you are curious.
- **Do not publish raw logs.** They contain third-party addresses and sometimes
  credentials reused from real breaches. Redact before you share.

## Watching them in ColimaUI

The honeypots in these articles are ordinary Compose projects, so the tools you
already have apply:

- **Compose page** — find the project, open the service, and read its logs
  without leaving the app. Fastest way to watch a session as it happens.
- **Containers page** — confirm the honeypot is actually up and see how long it
  has been running.
- **Activity page** — CPU and memory over time. A honeypot that suddenly starts
  using real CPU is itself worth investigating: it means something is doing more
  than knocking.

## What to look for

**Credentials tried.** The username and password list an attacker uses tells
you what they think you are. A list full of router defaults means you were
scanned indiscriminately; your actual application's username appearing means
something targeted you.

**Commands run.** In Cowrie, the first command after login is the most
informative one. `uname -a` is reconnaissance. A `wget` or `curl` to an IP
address is an attempt to pull down a payload — and the URL it fetched is
concrete intelligence you can act on.

**Repetition.** The same source address returning is different from a thousand
one-shot connections. The second pattern is background internet noise; the
first is someone who noticed you.

**Timing.** Hits arriving in a burst minutes after you exposed a service means
mass scanning found you. That is normal on a public address and tells you
nothing about your own security — it tells you the internet is busy.

## Keeping the output

Both honeypots write structured JSON alongside the human-readable log:

```bash
# Cowrie — copy it out, then read it on the host.
docker compose -f cowrie-compose.yml cp \
  cowrie:/cowrie/cowrie-git/var/log/cowrie/cowrie.json ./cowrie.json
tail -20 cowrie.json
```

Copying rather than `exec cat` is not a style preference: the Cowrie image ships
no shell and no `cat`, so `exec` fails with `executable file not found`. `cp`
goes through the daemon and needs nothing inside the container.

If you keep a honeypot running for more than a demo, copy that file out on a
schedule. Container logs rotate and volumes get pruned; the record is only
valuable if it survives.

## When to stop

If you started a honeypot to learn how one works, stop it once you have seen a
session end-to-end. A service left running with no one reading its output is
not detection — it is just an extra thing on your machine that answers the
network.

## Related

Honeypots on Colima · SSH honeypot with Cowrie · Decoy services with OpenCanary
