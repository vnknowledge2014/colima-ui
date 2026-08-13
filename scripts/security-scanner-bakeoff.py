#!/usr/bin/env python3
"""Measure Trivy against Grype so the scanner choice is a decision, not a guess.

Step 0 of the security-posture plan asks six questions before any scan code is
written. Each one changes a design decision downstream, so each is measured
rather than assumed:

  cold DB cost      -> can the database ever be bundled? (answer decides whether
                       the app ships a scanner or detects one)
  warm scan latency -> does the UI block on a scan, or run it in the background
                       and notify? The plan's gate is 60s on a ~200MB image.
  offline capability-> the plan promises Free features work with no network once
                       the DB is present. If that is false the promise changes.
  SBOM support      -> phase 3 exports one; if a scanner cannot produce it, that
                       is a second tool to detect.
  JSON stability    -> how defensively the parser in phase 1 has to be written.
  finding agreement -> if two scanners disagree wildly, a score cannot be
                       compared across them and must be stamped with its source.

Corpus is whatever the machine already has. That is deliberate: images a real
user accumulated are a better sample than a hand-picked list, and it keeps the
benchmark free of network variance.

Writes TSV + JSON to the output directory; the report is written by hand from it.
"""

import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

# Per-scan ceiling. A scan slower than this has already failed the UX question,
# so there is nothing to learn by waiting longer.
SCAN_TIMEOUT = 420

SEVERITIES = ["CRITICAL", "HIGH", "MEDIUM", "LOW", "NEGLIGIBLE", "UNKNOWN"]


def run(cmd, timeout=SCAN_TIMEOUT):
    """Run a command, returning (seconds, returncode, stdout, stderr)."""
    start = time.monotonic()
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return time.monotonic() - start, p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        return time.monotonic() - start, -1, "", f"timeout after {timeout}s"


def local_images():
    """Every tagged local image with its size, largest last."""
    out = subprocess.run(
        ["docker", "images", "--format", "{{.Repository}}:{{.Tag}}\t{{.Size}}"],
        capture_output=True, text=True, check=True,
    ).stdout
    seen = {}
    for line in out.splitlines():
        if not line.strip() or "<none>" in line:
            continue
        ref, size = line.split("\t")
        seen[ref] = size
    return sorted(seen.items(), key=lambda kv: parse_size(kv[1]))


def parse_size(s):
    m = re.match(r"([\d.]+)\s*([KMG]?B)", s.strip())
    if not m:
        return 0.0
    val, unit = float(m.group(1)), m.group(2)
    return val * {"B": 1e-6, "KB": 1e-3, "MB": 1.0, "GB": 1000.0}[unit]


def dir_size_mb(path):
    p = Path(path).expanduser()
    if not p.exists():
        return None
    total = sum(f.stat().st_size for f in p.rglob("*") if f.is_file())
    return round(total / 1e6, 1)


def trivy_counts(stdout):
    """Severity histogram from Trivy JSON, plus whether the shape was as expected."""
    doc = json.loads(stdout)
    counts = dict.fromkeys(SEVERITIES, 0)
    unknown_keys = set()
    for result in doc.get("Results") or []:
        for v in result.get("Vulnerabilities") or []:
            sev = (v.get("Severity") or "UNKNOWN").upper()
            if sev not in counts:
                unknown_keys.add(sev)
                sev = "UNKNOWN"
            counts[sev] += 1
    return counts, unknown_keys


def grype_counts(stdout):
    doc = json.loads(stdout)
    counts = dict.fromkeys(SEVERITIES, 0)
    unknown_keys = set()
    for m in doc.get("matches") or []:
        sev = ((m.get("vulnerability") or {}).get("severity") or "UNKNOWN").upper()
        if sev not in counts:
            unknown_keys.add(sev)
            sev = "UNKNOWN"
        counts[sev] += 1
    return counts, unknown_keys


def main():
    out_dir = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
    out_dir.mkdir(parents=True, exist_ok=True)

    for tool in ("trivy", "grype", "docker"):
        if not shutil.which(tool):
            sys.exit(f"{tool} not on PATH")

    report = {"db": {}, "scans": [], "capabilities": {}}

    # --- cold DB cost -----------------------------------------------------
    print("== downloading vulnerability databases (cold cost)", flush=True)
    secs, rc, _, err = run(["trivy", "image", "--download-db-only"], timeout=900)
    report["db"]["trivy"] = {"download_s": round(secs, 1), "rc": rc, "err": err[-200:]}
    print(f"   trivy db: {secs:.1f}s rc={rc}", flush=True)

    secs, rc, _, err = run(["grype", "db", "update"], timeout=900)
    report["db"]["grype"] = {"download_s": round(secs, 1), "rc": rc, "err": err[-200:]}
    print(f"   grype db: {secs:.1f}s rc={rc}", flush=True)

    report["db"]["trivy"]["cache_mb"] = dir_size_mb("~/Library/Caches/trivy")
    report["db"]["grype"]["cache_mb"] = dir_size_mb("~/Library/Caches/grype")

    # --- capability probes ------------------------------------------------
    probe = local_images()[0][0] if local_images() else "busybox:latest"
    for name, cmd in {
        "trivy_sbom_cyclonedx": ["trivy", "image", "--format", "cyclonedx",
                                 "--skip-db-update", probe],
        "trivy_sbom_spdx": ["trivy", "image", "--format", "spdx-json",
                            "--skip-db-update", probe],
        "grype_sbom": ["grype", "--help"],
    }.items():
        secs, rc, stdout, err = run(cmd, timeout=180)
        report["capabilities"][name] = {
            "rc": rc,
            "bytes": len(stdout),
            "note": ("sbom subcommand present" if "sbom" in stdout.lower() else
                     "no sbom generation") if name == "grype_sbom" else "",
        }

    # Offline: with the DB already local, neither tool should need the network.
    secs, rc, _, err = run(["trivy", "image", "--skip-db-update", "--format", "json",
                            probe], timeout=180)
    report["capabilities"]["trivy_offline_flag"] = {"rc": rc, "s": round(secs, 1)}
    secs, rc, _, err = run(["grype", "-o", "json", probe], timeout=180)
    report["capabilities"]["grype_offline_default"] = {"rc": rc, "s": round(secs, 1)}

    # --- warm scans over the corpus --------------------------------------
    corpus = local_images()
    print(f"== scanning {len(corpus)} local images with both tools", flush=True)

    for ref, size in corpus:
        row = {"image": ref, "size": size, "size_mb": round(parse_size(size), 1)}

        secs, rc, stdout, err = run(
            ["trivy", "image", "--skip-db-update", "--format", "json", ref])
        row["trivy_s"] = round(secs, 1)
        row["trivy_rc"] = rc
        if rc == 0 and stdout:
            try:
                counts, unknown = trivy_counts(stdout)
                row["trivy"] = counts
                row["trivy_unknown_sev"] = sorted(unknown)
                row["trivy_total"] = sum(counts.values())
                row["trivy_json_bytes"] = len(stdout)
            except json.JSONDecodeError as e:
                row["trivy_parse_error"] = str(e)
        else:
            row["trivy_err"] = err[-160:]

        secs, rc, stdout, err = run(["grype", "-o", "json", ref])
        row["grype_s"] = round(secs, 1)
        row["grype_rc"] = rc
        if rc == 0 and stdout:
            try:
                counts, unknown = grype_counts(stdout)
                row["grype"] = counts
                row["grype_unknown_sev"] = sorted(unknown)
                row["grype_total"] = sum(counts.values())
                row["grype_json_bytes"] = len(stdout)
            except json.JSONDecodeError as e:
                row["grype_parse_error"] = str(e)
        else:
            row["grype_err"] = err[-160:]

        report["scans"].append(row)
        print(f"   {ref[:52]:52} {row['size_mb']:>8.0f}MB "
              f"trivy {row['trivy_s']:>6.1f}s/{row.get('trivy_total','ERR'):>5} "
              f"grype {row['grype_s']:>6.1f}s/{row.get('grype_total','ERR'):>5}",
              flush=True)

    (out_dir / "bakeoff.json").write_text(json.dumps(report, indent=2))

    with (out_dir / "bakeoff.tsv").open("w") as f:
        f.write("image\tsize_mb\ttrivy_s\ttrivy_total\ttrivy_crit\ttrivy_high\t"
                "grype_s\tgrype_total\tgrype_crit\tgrype_high\n")
        for r in report["scans"]:
            t, g = r.get("trivy", {}), r.get("grype", {})
            f.write(f"{r['image']}\t{r['size_mb']}\t{r['trivy_s']}\t"
                    f"{r.get('trivy_total','')}\t{t.get('CRITICAL','')}\t{t.get('HIGH','')}\t"
                    f"{r['grype_s']}\t{r.get('grype_total','')}\t"
                    f"{g.get('CRITICAL','')}\t{g.get('HIGH','')}\n")

    print(f"\nwrote {out_dir}/bakeoff.json and bakeoff.tsv")


if __name__ == "__main__":
    main()
