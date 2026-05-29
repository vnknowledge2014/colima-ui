# ColimaUI + Colima — AI Agent Context Reference

This document provides authoritative knowledge about ColimaUI and Colima for the AI diagnostic agent. Use this as ground truth when answering questions or diagnosing issues.

---

## Section A: ColimaUI Application

### Overview
ColimaUI is a macOS desktop GUI for Colima (Container Linux on Mac), built with Tauri (Rust backend) + React (TypeScript frontend). It manages Colima VM instances, Docker containers/images/volumes/networks, Kubernetes clusters, and Lima VMs through a unified visual interface.

### Pages & Features
| Page | Description |
|------|-------------|
| **Instances** | Manage Colima VM instances — create, start, stop, restart, delete |
| **Containers** | List, start, stop, restart, inspect, exec into Docker containers |
| **Images** | Pull, tag, remove Docker images |
| **Volumes** | Create, inspect, remove Docker volumes |
| **Networks** | Create, inspect, remove Docker networks |
| **Terminal** | Embedded xterm.js terminal with SSH into Colima VM |
| **Compose** | Manage Docker Compose projects (up/down/logs/ps) |
| **Models** | Manage Ollama models (pull, serve, delete) |
| **Kubernetes** | Full K8s dashboard: pods, services, deployments, nodes, events, port-forward |
| **LinuxVMs** | Manage Lima VMs directly (independent of Colima) |
| **Settings** | API keys, AI provider config, SearXNG endpoint |

### Instance Management (Instances Page)
- **Create Instance**: modal with resource sliders (CPU, Memory, Disk), VM type selector, runtime, Kubernetes toggle, Network Address toggle
- **Profiles (Presets)**: stored in `localStorage` with key `colima-ui-custom-presets`
  - 5 built-in Quick Presets: Minimal (1C/1G/20G), Development (2C/4G/60G), Standard (4C/8G/100G), Power (8C/16G/200G), Kubernetes (4C/8G/100G + K8s)
  - AI-optimized presets: generated from host specs via Auto-detect, stored in `colima-ui-detected-presets`
  - Custom profiles: saved by user, stored in `colima-ui-custom-presets`
- **Start/Restart split-button**: main button uses last-used profile per instance (`colima-ui-last-profile-{instanceId}`); dropdown `▾` shows "My Profiles" section first, then "Quick Presets"; badges for `K8s` and `Net` features
- **Profile naming convention**: `default` profile → Colima name `colima`; named profile `dev` → Colima name `colima-dev`

### AI Agent Tools (in AiChatBubble)
The AI uses a custom tool protocol via text markers in responses:
- `[DIAGNOSE]` — reads VM serial logs, process states, lock files. **Always call first for start/stop errors.**
- `[RUN: command]` — auto-executed read-only shell commands (ps, cat, colima status, docker ps, kubectl get, etc.)
- `[RUN_APPROVE: command]` — requires user click before execution (colima start/stop, docker pull, kubectl apply, etc.)
- `[SEARCH: query]` — web search via SearXNG
- `[FETCH: url]` — fetches and converts a web page to markdown

### Backend Architecture
- **Tauri commands** exposed via IPC for Tauri window mode
- **HTTP API** on `127.0.0.1:11420` for browser mode access
- **Knowledge Bank** (SQLite): stores previously successful fixes and anti-patterns
- **SSE stream** at `/api/events` for real-time updates in browser mode

---

## Section B: Colima CLI Reference

### Profile Naming
- Default profile: `colima start` → binary named `colima`, socket at `~/.colima/default/docker.sock`, Lima VM at `~/.lima/colima`
- Named profile: `colima start myprofile` → binary named `colima-myprofile`, Lima VM at `~/.lima/colima-myprofile`

### Key Start Flags
```
colima start [profile] [flags]

--cpu, -c       int     Number of CPUs (default: 2)
--memory, -m    float   Memory in GiB (default: 2)
--disk, -k      int     Disk size in GiB (default: 60)
--vm-type       string  VM type: "vz" or "qemu" (default: "vz" on macOS 13+)
--runtime       string  Container runtime: "docker" or "containerd" (default: "docker")
--kubernetes    bool    Enable Kubernetes (K3s) in the VM
--network-address bool  Assign a reachable IP to the VM (requires vmnet privileges, slower start)
--arch          string  CPU architecture: "host", "aarch64", "x86_64"
--dns           ip      DNS server (can be repeated, e.g. --dns 8.8.8.8)
--mount         path    Mount host path into VM (e.g. --mount /Users:/Users:w)
--edit          bool    Edit colima.yaml config before starting
--verbose       bool    Show verbose debug output
--foreground    bool    Run in foreground (don't daemonize)
```

### VM Types
| Type | Description | Requirements |
|------|-------------|--------------|
| `vz` | Apple Virtualization.framework — **fast, low overhead** | macOS 13+ (Ventura), Apple Silicon or Intel |
| `qemu` | QEMU emulation — **slower, more compatible** | Works on all macOS versions |

**Recommendation**: Use `vz` unless targeting older macOS or needing cross-arch emulation.

### Key File Paths
| Path | Description |
|------|-------------|
| `~/.colima/{profile}/colima.yaml` | Instance config file (cpu, memory, disk, runtime, kubernetes, network, mounts, dns) |
| `~/.colima/{profile}/docker.sock` | Docker socket for this profile |
| `~/.lima/{lima-profile}/serial.log` | VM serial console log — **primary diagnostic source** |
| `~/.lima/{lima-profile}/ha.sock` | Lima host agent socket |
| `~/.colima/_lima/_config/override.yaml` | Lima global override config (advanced) |

**Lima profile mapping**: Colima default → `colima`; Colima named `dev` → `colima-dev`

### Common Commands
```bash
colima status                    # Status of default instance
colima status myprofile          # Status of named profile
colima list                      # List all instances with status
colima version                   # Show Colima version
colima stop                      # Stop default instance
colima stop --force              # Force stop (fixes "Broken" status)
colima restart                   # Restart default instance
colima delete                    # Delete default instance
colima delete --data             # Delete instance AND container data (from v0.9.0)
colima ssh                       # SSH into VM
colima ssh -- uname -a           # Run command in VM without interactive session
colima update                    # Update container runtime (Docker/containerd) without full reinstall (v0.7.6+)
colima edit                      # Edit colima.yaml and restart
```

### colima.yaml Schema (Key Fields)
```yaml
cpu: 4
memory: 8              # GiB
disk: 100              # GiB
vmType: vz             # "vz" or "qemu"
runtime: docker        # "docker" or "containerd"
kubernetes:
  enabled: false
  version: ""          # empty = use bundled K3s version
network:
  address: false       # true = assign routable IP (requires sudo)
  dns: []              # custom DNS servers
mounts:
  - location: /path/on/host
    writable: false
provision: []          # Lima provision scripts
```

---

## Section C: Common Issues & Behaviors

### Kubernetes (K3s)
- K3s runs **inside** the Colima VM — it is NOT a separate process on the host
- Enabling/disabling Kubernetes **requires a full instance restart** — no hot-toggle
- K3s version is bundled with Colima; use `colima update` to get newer K3s
- `kubectl` on host uses `~/.kube/config` auto-configured by Colima when K3s starts
- K3s does NOT require `--network-address` — it works via localhost port-forward by default
- If you need pods/services accessible from host by IP (not port-forward), then enable `--network-address`

### Network Address (`--network-address`)
- **Off (default)**: VM accessible only via `localhost` port-forwarding; docker socket at `~/.colima/{profile}/docker.sock`
- **On**: VM gets a routable IP (e.g. `192.168.106.x`) — directly accessible from host
- Requires elevated privileges (vmnet framework) — may prompt for password or fail without proper permissions
- Causes **slower startup** (~10-20s extra)
- Use case: microservices development where services need to call each other by IP, or Incus containers

### "Broken" Status
- Happens after macOS restart or ungraceful shutdown
- Fix: `colima stop --force` → status changes to Stopped → `colima start` normally

### Docker Bind Mounts Showing Empty
- Cause: mount source not in `/Users/$USER` and not configured in colima.yaml `mounts`
- Fix: add path to `mounts` in `~/.colima/{profile}/colima.yaml`, then `colima restart`

### Colima No Internet / DNS Failures
- Fix: `colima start --dns 8.8.8.8 --dns 1.1.1.1`
- Or add to colima.yaml: `network: dns: ["8.8.8.8", "1.1.1.1"]`

### Disk Space Recovery
- From v0.5.0: `colima restart` automatically releases unused disk space
- Manual: `colima ssh -- sudo fstrim -a`
- Increase disk: edit `disk:` in colima.yaml (auto-expanded on next start, v0.5.3+)

### FATA error starting vm / exit status 1
- Enable debug: `colima start --verbose`
- Common causes:
  1. No hardware virtualization support
  2. Wrong arch binary (x86_64 Homebrew on M1)
  3. Port conflict on 6443 (K8s API)
  4. Previous lock file not cleaned — check `~/.lima/{profile}/`

### Lima Serial Log (Primary Diagnostic)
Always check `~/.lima/colima/serial.log` (or `~/.lima/colima-{profile}/serial.log`) for start failures. This is the most informative source — use `[DIAGNOSE]` or `[RUN: tail -100 ~/.lima/colima/serial.log]`.

---

## Section D: Colima FAQ (Key Excerpts)

### VM IP not reachable
Reachable IP is not enabled by default due to root privilege requirements and slower startup.
Enable: `colima start --network-address` or set `network: address: true` in colima.yaml.

### How to recover disk space
- Auto (v0.5.0+): `colima restart`
- Manual: `colima ssh -- sudo fstrim -a`

### How to increase disk size
Edit `disk:` value in colima.yaml — auto-expanded on next `colima start` (v0.5.3+).

### How to update Colima
```bash
brew upgrade colima
colima delete    # reset to use new VM image
colima start
```
Or test without affecting existing setup: `colima start debug`

### How to update Docker/containerd runtime only (without full Colima upgrade)
```bash
colima update    # available from v0.7.6
```

### Lima overrides (advanced)
Global override: `~/.colima/_lima/_config/override.yaml`
Provision scripts can be added via Lima overrides or directly in colima.yaml under `provision:`.

### Delete container data
- `colima delete` — deletes VM, preserves container data disk (v0.9.0+)
- `colima delete --data` — deletes everything including container data
