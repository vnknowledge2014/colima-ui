# Install the Docker CLI

## Symptom

Colima is running, but ColimaUI shows `docker: command not found`, or Compose
features are greyed out with *docker compose is unavailable*.

## Cause

Colima provides the Docker **daemon** inside its VM. It does not provide the
`docker` **client** that talks to it — that is a separate package. Docker
Compose v2 is a further separate plugin, which is why Compose can be missing
while plain `docker` works.

## Fix

```bash
brew install docker docker-compose
docker version
docker compose version
```

You do **not** need Docker Desktop. Installing it alongside Colima is in fact
the most common source of confusion here, because it registers its own daemon
and its own context.

If `docker` is installed but commands still fail, the client is pointed at the
wrong daemon. Colima registers a context named `colima` when it starts:

```bash
docker context ls
docker context use colima
docker ps
```

## Related

- [Start a Colima instance](start-colima)
- [Common errors](common-errors)
