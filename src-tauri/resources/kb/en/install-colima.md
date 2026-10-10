# Install Colima

## Symptom

ColimaUI reports `colima: command not found`, or the Setup page shows Colima as
**Missing**. Nothing on the Instances page loads.

## Cause

Colima is a separate command-line tool. ColimaUI is a front end for it — it does
not bundle the VM runtime, so Colima has to be on your `PATH` before anything
works. Launching the app from Finder or the Dock also gives it a shorter `PATH`
than your shell has, so a Colima installed under a non-standard prefix can be
invisible to the app even though it works in a terminal.

## Fix

**macOS (Homebrew):**

```bash
brew install colima docker docker-compose
```

**Linux:**

```bash
# Debian/Ubuntu
sudo apt install colima docker.io docker-compose-v2

# Arch
sudo pacman -S colima docker docker-compose
```

Then confirm the binary answers:

```bash
colima version
colima status
```

If `colima version` works in your terminal but ColimaUI still says Missing,
Colima is installed outside the directories the app searches. Symlink it into
`/usr/local/bin` and restart the app:

```bash
sudo ln -s "$(which colima)" /usr/local/bin/colima
```

## Related

- [Start a Colima instance](start-colima)
- [Install the Docker CLI](install-docker-cli)
