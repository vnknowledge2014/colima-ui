# Decoy services with OpenCanary

## What this is

OpenCanary runs a set of fake services — FTP, HTTP, SSH banners and others —
that do nothing except record who touched them. Where Cowrie invites an
attacker in and watches, OpenCanary just answers the door and writes down that
someone knocked.

That difference matters: OpenCanary is what you would actually deploy to detect
something, because it is quiet, cheap, and produces almost no output until a hit
happens.

## Before you run it

Read **Honeypots on Colima** first. The warnings about corporate networks apply
most strongly here — a fake FTP server on a company network looks exactly like
an unauthorised service to whoever runs that network.

The compose file below binds to `127.0.0.1`. On a real deployment you would
change that, and that is the decision the overview article asks you to make
deliberately.

## Run it

OpenCanary needs a config file that says which services to enable. Save this as
`opencanary.conf`:

```json
{
  "device.node_id": "colima-canary",
  "ftp.enabled": true,
  "ftp.port": 21,
  "ftp.banner": "FTP server ready",
  "http.enabled": true,
  "http.port": 80,
  "http.banner": "Apache/2.2.22 (Ubuntu)",
  "http.skin": "nasLogin",
  "logger": {
    "class": "PyLogger",
    "kwargs": {
      "handlers": {
        "console": { "class": "logging.StreamHandler", "stream": "ext://sys.stdout" }
      }
    }
  }
}
```

Save this as `opencanary-compose.yml`:

```yaml
services:
  opencanary:
    image: thinkst/opencanary:latest
    restart: unless-stopped
    ports:
      # Host-side 127.0.0.1 binding is the safety property.
      - "127.0.0.1:2121:21"
      - "127.0.0.1:8080:80"
    volumes:
      - ./opencanary.conf:/root/.opencanary.conf:ro
```

Both files must sit in the same directory, and that directory must be one Colima
shares with the VM — your home directory is, `/tmp` on the host is not. If the
VM cannot see `opencanary.conf`, Docker creates a *directory* with that name
instead of mounting your file. OpenCanary then finds no config, starts zero
services, and `restart: unless-stopped` quietly restarts it forever. The symptom
is a container that looks healthy and never logs a thing.

Start it:

```bash
docker compose -f opencanary-compose.yml up -d
```

## Generate some traffic

```bash
curl -s http://127.0.0.1:8080/ >/dev/null
```

That one request is a "hit". Nothing else should ever produce one.

## What you will see

```bash
docker compose -f opencanary-compose.yml logs -f
```

Quiet, then a JSON line per hit with the source address, the service touched,
and the timestamp. The silence is the feature — anything at all in this log
deserves your attention, which is not true of most logs you own.

The config above is a starting point with two services. OpenCanary supports
many more; add them once you have seen the basic loop work.

## Stop and clean up

```bash
docker compose -f opencanary-compose.yml down
```

## Related

Honeypots on Colima · SSH honeypot with Cowrie · Reading honeypot logs
