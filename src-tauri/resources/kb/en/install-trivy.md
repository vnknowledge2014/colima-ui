# Install Trivy for image scanning

## Symptom

The Security page reports that the scanner is missing, or scanning an image does
nothing.

## Cause

ColimaUI does not bundle a vulnerability scanner. It drives Trivy, which has to
be installed on your host. The reason it is not bundled is size: Trivy's
vulnerability database alone is about 1.2 GB, and it changes daily — shipping a
copy would mean shipping a stale one.

## Fix

macOS:

```bash
brew install trivy
```

Linux: follow the install instructions for your distribution at
`https://trivy.dev/latest/getting-started/installation/`.

Verify:

```bash
trivy --version
```

Then return to ColimaUI and refresh the Security page.

## The first scan downloads a database

The first scan (or the first one after several days) downloads the vulnerability
database before it can scan anything. That step is shown separately in the
progress display, because it is the slow part — roughly 40 seconds on a fast
connection, against about 2.5 seconds for scanning a 200 MB image afterwards.

Once the database is on disk, scanning works without a network connection.

## What is sent where

Scanning reads the image from your local container runtime. Image names, package
lists and scan results stay on your machine. The only outbound request is Trivy
fetching its own vulnerability database.

## When a single image fails to scan

Some images cannot be read by the scanner — usually a layer it cannot unpack.
That failure applies to that one image; every other image still scans. The error
text from the scanner is shown as-is, because it is the only clue to why.
