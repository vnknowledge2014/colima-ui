# Harden a container image

## What the Security page is checking

Two separate questions. **Known vulnerabilities** come from Trivy: packages
inside the image that have a published advisory. **Configuration** comes from
rules ColimaUI applies to the image itself — how it runs, where it came from,
how old it is. An image can be perfectly patched and still be badly configured,
which is why the score has four parts rather than one.

## The changes worth making first

**Do not run as root.** With no `USER` instruction the process starts as uid 0.
Create an account in the image and switch to it at the end of the Dockerfile:

```dockerfile
RUN adduser --system --no-create-home app
USER app
```

If the service needs port 80, listen on 8080 inside the container and map it
outside with `-p 80:8080`. Binding a low port is the usual reason an image is
still running as root.

**Take credentials out of the image.** Anything in `ENV` travels with every copy
of the image and is visible in `docker inspect`. Pass it at run time instead:

```bash
docker run --env-file ./secrets.env myapp:1.2.3
```

Rebuilding without the value is not enough on its own — the layer that contained
it still exists in any registry it was pushed to. Rotate the credential.

**Pin what you deploy.** A tag is a position, not an image: `latest`, `stable`
and even `1.27` are republished as patches land. Tag releases with a full
version, and deploy by digest where it matters:

```bash
docker pull nginx@sha256:...
```

**Rebuild on a schedule.** Base images receive package updates continuously. An
image that has not been rebuilt for six months cannot contain any fix published
since it was built, however clean today's scan looks.

**Record where it came from.** Two labels turn an unknown image on a machine
into something traceable:

```dockerfile
LABEL org.opencontainers.image.source="https://github.com/you/app"
LABEL org.opencontainers.image.revision="$GIT_SHA"
```

## Making the image smaller usually makes it safer

Most findings come from packages the application never calls. A `-slim` or
`-alpine` base, or a multi-stage build that ships only the compiled artefact,
removes them along with the reason to patch them. The Security page suggests
alternatives for common base images.

Be aware of what a swap changes: Alpine uses musl rather than glibc, so native
dependencies may need rebuilding, and distroless images have no shell at all —
which is exactly why they are safer, and exactly why debugging inside them is
harder.

## About the score

The number is for sorting a list. The four components are the answer: they say
whether the problem is CVEs, configuration, provenance or age, and those need
different work.

The score is only comparable against another score computed with the same
scanner, the same vulnerability database date and the same rule pack version —
which is why all three are printed next to it. Different scanners disagree by
more than tenfold on the same image; that is a property of vulnerability data,
not a bug in either tool.

## Strictness levels

L1 covers what almost every image should do. L2 adds recommendations that are
right for most services. L3 is strict, and some of it will not apply to a given
image. A higher level can only ever lower a score — it enables more rules, never
fewer.

## Where the rules come from

ColimaUI's rules are written by this project. They reference section numbers from
published standards (for example CIS Docker Benchmark §4.1) so you can find the
original discussion, but the wording, the scoring and the advice are ours.
ColimaUI is not certified by, affiliated with, or endorsed by CIS or OWASP.
