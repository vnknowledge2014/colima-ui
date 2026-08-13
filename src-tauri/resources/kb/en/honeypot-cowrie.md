# SSH honeypot with Cowrie

## What this is

Cowrie pretends to be an SSH server with weak credentials. When someone logs in,
it hands them a fake shell: commands appear to work, the filesystem looks real,
and nothing they type touches your machine. It records the session — every
credential tried, every command run, every file fetched.

It can serve Telnet too, but that is switched off in the shipped configuration,
so this article only publishes the SSH port.

It is the best starting point because the logs are readable without tooling.

## Before you run it

Read **Honeypots on Colima** first if you have not. The compose file below binds
to `127.0.0.1`, so only your own machine can reach it. Changing that exposes a
deliberately weak SSH service to whatever network you attach it to.

Cowrie does not run as root and does not need privileged mode. If a guide tells
you to add `privileged: true`, you are reading a guide for something else.

## Run it

Save as `cowrie-compose.yml`:

```yaml
services:
  cowrie:
    image: cowrie/cowrie:latest
    restart: unless-stopped
    ports:
      # Host-side 127.0.0.1 binding is the safety property. Do not remove it
      # without reading the warnings in the overview article.
      - "127.0.0.1:2222:2222"
    volumes:
      # Cowrie's working directory is /cowrie/cowrie-git, not /cowrie. Mounting
      # /cowrie/var instead would create an empty directory the honeypot never
      # writes to, and every recorded session would vanish on the next `down -v`.
      - cowrie-var:/cowrie/cowrie-git/var
      - cowrie-etc:/cowrie/cowrie-git/etc

volumes:
  cowrie-var:
  cowrie-etc:
```

Start it:

```bash
docker compose -f cowrie-compose.yml up -d
```

## Generate some traffic

Connect to it yourself. Any password works — that is the point:

```bash
ssh -p 2222 root@127.0.0.1
```

Accept the host key, type any password, and you land in the fake shell. Try
`ls`, `cat /etc/passwd`, `wget http://example.com/x`. None of it touches your
system. Type `exit` when done.

## What you will see

```bash
docker compose -f cowrie-compose.yml logs -f
```

Each line is a structured event: connection opened, credentials attempted,
command executed, session closed. The same events land as JSON in
`/cowrie/cowrie-git/var/log/cowrie/cowrie.json` inside the container, which is
the format worth parsing if you keep it running.

See **Reading honeypot logs** for what to do with them.

## Stop and clean up

```bash
docker compose -f cowrie-compose.yml down -v
```

The `-v` removes the recorded sessions too. Drop it if you want to keep them.

## Related

Honeypots on Colima · Reading honeypot logs
