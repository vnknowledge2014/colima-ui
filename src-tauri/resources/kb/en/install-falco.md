# Install Falco for Colima

Falco is the CNCF runtime security engine. It watches system calls with eBPF and
raises an alert when something matches one of its rules — a shell opening inside
a container, a sensitive file being read, an unexpected outbound connection.

ColimaUI does not detect anything itself. It reads what Falco reports and shows
it next to the compose services you recognise. Your coverage is whatever your
Falco rules cover.

## Two traps before you start

**1. `brew install falco` installs the wrong program.**
There is an unrelated tool called `falco` — a VCL parser and linter for Fastly.
It is what Homebrew's `falco` formula gives you. It has nothing to do with
runtime security. ColimaUI detects this case and says so rather than pretending
Falco is present.

**2. Falco does not run on macOS.**
It needs a Linux kernel. On a Mac it runs *inside the Colima VM*, not on the
host, so every command below is run through `colima ssh`.

## Install

Falco publishes arm64 and x86_64 packages. On an Apple Silicon Mac this takes
about ten seconds inside the VM.

```sh
colima ssh
sudo bash
curl -fsSL https://falco.org/repo/falcosecurity-packages.asc \
  | gpg --dearmor -o /usr/share/keyrings/falco-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/falco-archive-keyring.gpg] https://download.falco.org/packages/deb stable main" \
  > /etc/apt/sources.list.d/falcosecurity.list
apt-get update -y
FALCO_FRONTEND=noninteractive FALCO_DRIVER_CHOICE=modern_ebpf apt-get install -y falco
```

`modern_ebpf` is the driver to pick: it needs no kernel headers and no module
build, and the kernel Colima ships (6.8) supports it.

## Verify it is actually detecting something

This is the step people skip, and it is the one that matters.

Falco can install cleanly, load its eBPF driver, run as a healthy systemd
service — **and detect nothing at all**, because it loaded zero rules. Nothing in
its status output says so. A tool that is running and blind is worse than one
that is missing, because you believe you are covered.

Check the rule count — read the `rules` array, not a count of `"name"`, which
would also count macros and lists and flatter an engine that has none:

```sh
sudo falco -L -o json_output=true | python3 -c \
  'import sys,json; print(len(json.load(sys.stdin)["rules"]), "rules loaded")'
```

A stock install loads around 25 rules. **If this prints 0, Falco is detecting
nothing.** The usual cause is that the rules fetch (`falcoctl`) was disabled at
install time, leaving the engine with an empty rule set even though
`/etc/falco/falco_rules.yaml` exists on disk. Point Falco at the file explicitly:

```sh
sudo falco -r /etc/falco/falco_rules.yaml
```

ColimaUI shows this state as its own warning rather than as "ready", so if the
Runtime tab says Falco is running without rules, this is what it means.

## Let ColimaUI read the events

Falco ships with file output **disabled**, so by default there is nothing for
ColimaUI to read. Enable JSON output to a file:

```sh
sudo tee /etc/falco/config.d/colimaui.yaml <<'YAML'
json_output: true
json_include_output_property: true
file_output:
  enabled: true
  keep_alive: false
  filename: /var/log/falco/events.json
YAML
sudo mkdir -p /var/log/falco
sudo systemctl restart falco-modern-bpf.service
```

Then open **Security → Runtime** in ColimaUI and press *Start reading*.

## Confirm end to end

Trigger a rule that ships with the default set:

```sh
docker run --rm alpine cat /etc/shadow
```

Within a few seconds a *Read sensitive file untrusted* event should appear in the
Runtime tab, tagged with the container it came from.

## Expected noise on Apple Silicon

On arm64 Falco logs these at every start:

```
libbpf: failed to determine tracepoint 'syscalls/sys_enter_open' perf event ID
libpman: failure while attaching TOCTOU mitigation program for 'open'
```

This is harmless. `open` and `creat` do not exist as syscalls on arm64 — it uses
`openat` instead — so only the TOCTOU hardening for those two is unavailable.
Detection works normally, which you can confirm with the `/etc/shadow` test
above.

## What this does not give you

Falco detects behaviour that its rules describe. It is not a rootkit scanner, it
does not stop anything, and a rule set nobody has tuned will produce false
positives on ordinary work — package installs read sensitive files too. Treat
events as a starting point for a question, not as a verdict.
