# Performance tuning

## Symptom

Builds are slow, file changes take seconds to appear inside a container, or the
whole machine becomes sluggish while the VM runs.

## Cause

Three settings account for nearly all of it, and the defaults are conservative
because they have to boot on the smallest supported machine:

- **VM type.** `qemu` emulates; `vz` uses Apple's native Virtualization
  framework. On Apple Silicon `vz` is substantially faster.
- **Mount type.** `sshfs` tunnels every file operation over SSH. `virtiofs`
  is a native shared filesystem and is the single largest win for
  bind-mount-heavy workloads such as Node or PHP projects.
- **Size.** Two CPUs and 2 GiB is enough to start a VM, not to build in one.

## Fix

On Apple Silicon, switch the VM and mount type. This recreates nothing — it is a
restart:

```bash
colima stop
colima start --vm-type vz --mount-type virtiofs
```

Then give it room. As a rule, half the host's cores and half its RAM:

```bash
colima stop
colima start --cpu 4 --memory 8 --disk 100
```

Both are editable in Settings → Colima Config, which writes `colima.yaml` and
shows you the diff before applying. Changes take effect on the next restart.

Finally, reclaim space periodically — a full disk looks exactly like a slow one:

```bash
docker system df
docker system prune -a --volumes
```

## What not to do

- Do not give the VM every core. It competes with the host scheduler and the
  machine as a whole gets slower.
- Do not leave Kubernetes enabled on a profile that only runs containers; it
  costs about 1 GiB of memory permanently.
- Do not oversize the disk hoping to change it later. Disks grow but never
  shrink.

## Related

- [Common errors](common-errors)
- [Start a Colima instance](start-colima)
