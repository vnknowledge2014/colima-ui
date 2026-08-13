# Honeypots on Colima

## What a honeypot is

A honeypot is a service that exists only to be attacked. Nothing legitimate
ever connects to it, so every connection it records is, by definition,
something you did not expect. That is the whole idea: no false positives from
normal traffic, because there is no normal traffic.

Containers make honeypots practical to run locally. Each one is a throwaway
process with its own filesystem, and Colima already keeps them inside a VM.

## Two very different reasons to run one

Be clear which one you are doing, because they have opposite risk profiles.

**To learn.** You run it on your own machine, bound to localhost, and connect
to it yourself to see what the logs look like. Nothing hostile reaches it. This
is safe and is what the articles here describe.

**To detect.** You place it on a network where nothing should touch it, and
treat any hit as a real signal. This is genuinely useful and genuinely
consequential — see the warnings below before you go near it.

## Before you expose one to a real network

These are not formalities.

- **A honeypot on the public internet attracts hostile traffic to your IP
  address.** That is its job. You are choosing to become a target, and the
  traffic does not stop when you lose interest.
- **Check your workplace policy first.** On a corporate network, running a
  service that impersonates infrastructure can violate policy even when your
  intent is defensive. Ask before, not after.
- **Your hosting provider may have rules too.** Some explicitly forbid it;
  others require notice.
- **A compromised honeypot is a foothold.** These services are deliberately
  weak. If one is broken out of, the attacker is now inside whatever network
  you put it on.

Every compose file in these articles binds to `127.0.0.1` for this reason. If
you change that to `0.0.0.0`, you have made a decision — make it deliberately.

## Where to start

Read **SSH honeypot with Cowrie** first. It gives the clearest logs and you can
generate traffic yourself in one command, so you see the whole loop working
before deciding whether you want more.

**Decoy services with OpenCanary** is closer to how these get used for real
detection: quiet until something touches it.

**Reading honeypot logs** covers what to do with the output once it exists.

## Related

SSH honeypot with Cowrie · Decoy services with OpenCanary · Reading honeypot logs
