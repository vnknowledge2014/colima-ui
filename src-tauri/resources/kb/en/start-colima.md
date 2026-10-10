# Start a Colima instance

## Symptom

Errors reading *Cannot connect to the Docker daemon*, *Colima is not running*,
or *the connection to the server was refused*. Container, image and Kubernetes
pages are empty even though the tools are installed.

## Cause

Colima is installed but its virtual machine is stopped. Every Docker and
Kubernetes command in this app talks to a daemon **inside** that VM, so with the
VM down there is nothing listening. This is the normal state after a reboot —
Colima does not start automatically.

## Fix

```bash
colima start

# or, for a named profile
colima start --profile dev
```

Or press **Start** on the Instances page. The first start of a profile takes one
to two minutes because it downloads and provisions a disk image; later starts
take a few seconds.

If the start fails, the VM's own log says why:

```bash
colima status
colima start --verbose
tail -n 100 ~/.colima/default/daemon/daemon.log
```

The two failures worth knowing:

- **Not enough disk space on the host.** The disk image is allocated up front.
  Free space, or lower the disk size in Settings → Colima Config.
- **A stale VM from an interrupted start.** Recreate it — this deletes the VM's
  containers and images, but not your source code or named volumes' host mounts:

  ```bash
  colima stop
colima delete
colima start
  ```

## Related

- [Common errors](common-errors)
- [Performance tuning](performance-tuning)
