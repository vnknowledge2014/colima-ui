# Common errors

A lookup table for the messages ColimaUI surfaces most often.

## Permission denied

**Symptom.** *permission denied*, *operation not permitted*, or a mounted
directory that reads as empty inside a container.

**Cause.** Colima mounts host directories into the VM with your user's
ownership. A container running as a different UID cannot write to them, and
macOS additionally withholds Documents, Desktop and Downloads from the VM until
you grant access.

**Fix.** Grant the disk access in System Settings → Privacy & Security → Files
and Folders, then restart the instance. For UID mismatches, run the container as
your own user:

```bash
docker run --user "$(id -u):$(id -g)" -v "$PWD:/work" -w /work alpine sh
```

## Operation timed out

**Symptom.** *timed out*, *deadline exceeded*, or the UI hangs then reports a
timeout.

**Cause.** The VM is alive but too busy to answer — usually a large image pull,
a build, or memory pressure that has pushed the VM into swap.

**Fix.** Wait for the running operation, or check what is consuming the VM:

```bash
colima ssh -- top -b -n 1 | head -20
```

If it recurs while idle, the VM is undersized — see
[performance tuning](performance-tuning).

## Network failures

**Symptom.** *connection refused*, *could not resolve host*, *x509 certificate*.

**Cause.** DNS inside the VM is separate from the host's. Corporate VPNs and
proxies that the host resolves through are frequently not visible to the VM, and
a TLS-intercepting proxy's CA is not installed inside it.

**Fix.** Set an explicit DNS server in Settings → Colima Config (for example
`1.1.1.1`) and restart the instance. Verify from inside:

```bash
colima ssh -- nslookup registry-1.docker.io
```

## Port is already allocated

**Symptom.** *bind: address already in use* when starting a container.

**Cause.** Another process on the host already holds the port. Colima forwards
published ports to the host, so host-side conflicts apply.

**Fix.**

```bash
lsof -nP -iTCP:8080 -sTCP:LISTEN
```

Stop that process, or publish on a different host port.

## No space left on device

**Symptom.** Builds and pulls fail with *no space left on device* while the host
still shows free space.

**Cause.** The VM has its own fixed-size disk. Filling it is independent of the
host being full.

**Fix.** Reclaim inside the VM first:

```bash
docker system df
docker system prune -a --volumes
```

If it stays full, raise the disk size in Settings → Colima Config. A disk can
only grow, never shrink.

## Related

- [Start a Colima instance](start-colima)
- [Performance tuning](performance-tuning)
