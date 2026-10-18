#!/usr/bin/env python3
"""Measure deterministic fixability of broken compose files — the auto-fix gate.

`scripts/compose-diagnose-benchmark.sh` answers "does Docker's own validator
explain the failure?". This answers the next question, the one that decides
whether auto-fix ships as *apply-a-patch* or *suggest-only*:

    for each broken file, can a deterministic rule produce a file that
    (a) passes `docker compose config`, and (b) loses no keys?

For every case the harness runs the real validator, classifies with a port of
`categorize()`, attempts the prototype fixer for that category, then re-validates
and diffs keys. A "fix" that drops a key is counted as a FAILURE, not a pass —
that is exactly the silent-secret-deletion failure the plan calls out.

The fixers here are prototypes for measurement, not shipping code. They are
deliberately conservative: a category with no safe rule returns "no fix", which
is a legitimate and useful gate outcome.

Usage:  python3 scripts/compose-autofix-gate.py [corpus-dir]
"""

import json
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_CORPUS = ROOT / "tests" / "compose-corpus"

# Keys Docker Compose accepts at service level. Enough for typo correction;
# not the full schema, and deliberately not guessed beyond common usage.
SERVICE_KEYS = [
    "image", "build", "command", "entrypoint", "environment", "env_file", "ports",
    "volumes", "networks", "depends_on", "restart", "healthcheck", "labels",
    "container_name", "user", "working_dir", "expose", "deploy", "secrets",
    "extra_hosts", "profiles", "platform", "pull_policy", "stop_grace_period",
]


def run_compose_config(path: pathlib.Path) -> tuple[bool, str]:
    """Run the same validator the product runs. Returns (ok, stderr)."""
    try:
        p = subprocess.run(
            ["docker", "compose", "-f", str(path), "config", "--quiet"],
            capture_output=True, text=True, timeout=30,
        )
        return p.returncode == 0, (p.stderr or p.stdout).strip()
    except subprocess.TimeoutExpired:
        return False, "timeout"
    except FileNotFoundError:
        print("error: docker not found", file=sys.stderr)
        sys.exit(2)


def categorize(raw: str) -> str:
    """Port of categorize() in compose_diagnose.rs:45. Kept branch-for-branch."""
    l = raw.lower()
    if "no such file" in l or ("stat " in l and "no such" in l):
        return "missing_file"
    if any(s in l for s in ("yaml:", "go-yaml", "did not find expected",
                            "could not find expected", "mapping values",
                            "scanning a simple key")):
        return "yaml_syntax"
    if any(s in l for s in ("variable is not set", "undefined volume",
                            "undefined network", "undefined secret")):
        return "undefined_reference"
    # structure BEFORE schema: "services must be a mapping" contains "must be a".
    if ("services must be a mapping" in l or "empty compose file" in l
            or "no services" in l):
        return "structure"
    if any(s in l for s in ("additional propert", "must be a", "invalid type",
                            "unsupported config option", "not allowed")):
        return "schema"
    return "other"


def keys_in(text: str) -> set[str]:
    """Every mapping key appearing in the file, by name.

    Deliberately lexical, not a YAML parse: the point is to notice a key
    vanishing even when the file is too broken to parse, which is when the
    dangerous edits happen.
    """
    return {m.group(1) for m in re.finditer(r"^\s*-?\s*([A-Za-z_][\w.-]*)\s*:", text, re.M)}


# ---------------------------------------------------------------- fixers ----

def fix_tabs(text: str, _err: str) -> str | None:
    """Tabs are never legal YAML indentation. Purely lexical."""
    if "\t" not in text:
        return None
    return "\n".join(re.sub(r"^\t+", lambda m: "  " * len(m.group(0)), ln) for ln in text.split("\n"))


def fix_undefined_toplevel(text: str, err: str) -> str | None:
    """Declare volumes/networks the file already references by name.

    Nothing is invented: the name comes from the error, and an empty declaration
    is what `docker compose` itself documents for a default local volume.
    Appended as text so surrounding comments survive.
    """
    kinds = {"volume": "volumes", "network": "networks"}
    found: dict[str, list[str]] = {}
    for m in re.finditer(r'undefined (volume|network)[:\s"]+([\w.-]+)', err, re.I):
        found.setdefault(kinds[m.group(1).lower()], []).append(m.group(2))
    if not found:
        return None
    out = text.rstrip("\n")
    for section, names in found.items():
        out += f"\n{section}:\n" + "".join(f"  {n}:\n" for n in dict.fromkeys(names))
    return out + "\n" if not out.endswith("\n") else out


def fix_typo_key(text: str, err: str) -> str | None:
    """Correct a service key that is one edit away from a real one."""
    m = re.search(r"additional propert(?:y|ies) '?\"?([\w.-]+)", err, re.I)
    if not m:
        return None
    bad = m.group(1)
    if bad in SERVICE_KEYS:
        return None

    def dist(a: str, b: str) -> int:
        if abs(len(a) - len(b)) > 2:
            return 99
        prev = list(range(len(b) + 1))
        for i, ca in enumerate(a, 1):
            cur = [i]
            for j, cb in enumerate(b, 1):
                cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
            prev = cur
        return prev[-1]

    ranked = sorted(((dist(bad, k), k) for k in SERVICE_KEYS))
    # Require a clear winner: an ambiguous correction is a guess, not a fix.
    if ranked[0][0] > 2 or (len(ranked) > 1 and ranked[0][0] == ranked[1][0]):
        return None
    return re.sub(rf"^(\s*){re.escape(bad)}(\s*:)", rf"\g<1>{ranked[0][1]}\g<2>", text, count=1, flags=re.M)


def fix_scalar_to_list(text: str, err: str) -> str | None:
    """Wrap a scalar in a list where the schema demands an array."""
    m = re.search(r"(\w+) must be a (?:list|array)", err, re.I)
    key = m.group(1) if m else None
    if not key:
        m2 = re.search(r"services\.\w+\.(\w+) must be a", err, re.I)
        key = m2.group(1) if m2 else None
    if not key:
        return None
    pat = re.compile(rf"^(\s*){re.escape(key)}:[ \t]+(?!$)([^\n#]+?)[ \t]*$", re.M)
    m3 = pat.search(text)
    if not m3:
        return None
    indent, value = m3.group(1), m3.group(2).strip()
    if value.startswith(("[", "{", "|", ">", "&", "*")):
        return None
    return pat.sub(f'{indent}{key}:\n{indent}  - "{value}"', text, count=1)


FIXERS = {
    "yaml_syntax": [fix_tabs],
    "undefined_reference": [fix_undefined_toplevel],
    "schema": [fix_typo_key, fix_scalar_to_list],
    "missing_file": [],   # cannot invent someone's file
    "structure": [],      # needs a full re-serialisation; comments lost
    "other": [],
}


def main() -> None:
    corpus = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_CORPUS
    manifest = json.loads((corpus / "manifest.json").read_text())
    cases = {c["id"]: c for c in manifest["cases"]}

    rows = []
    for path in sorted(corpus.glob("*.yml")):
        case = cases.get(path.stem, {})
        original = path.read_text(encoding="utf-8")

        ok, err = run_compose_config(path)
        if ok:
            rows.append({"id": path.stem, "expected": case.get("expected_category", "?"),
                         "actual": "VALID", "outcome": "not-broken", "detail": "file validates; bad corpus case"})
            continue

        actual = categorize(err)
        fixed_text, applied = None, None
        for fixer in FIXERS.get(actual, []):
            candidate = fixer(original, err)
            if candidate and candidate != original:
                fixed_text, applied = candidate, fixer.__name__
                break

        if fixed_text is None:
            rows.append({"id": path.stem, "expected": case.get("expected_category", "?"),
                         "actual": actual, "outcome": "no-fix", "detail": "no deterministic rule"})
            continue

        with tempfile.TemporaryDirectory() as td:
            tmp = pathlib.Path(td) / path.name
            # Copy siblings so relative paths (env_file, build context) resolve
            # the same way they would in place.
            for sib in path.parent.glob("*"):
                if sib.is_file() and sib.name != "manifest.json":
                    shutil.copy(sib, pathlib.Path(td) / sib.name)
            tmp.write_text(fixed_text, encoding="utf-8")
            ok2, err2 = run_compose_config(tmp)

        lost = keys_in(original) - keys_in(fixed_text)
        comments_before = original.count("#")
        comments_after = fixed_text.count("#")

        if lost:
            outcome, detail = "UNSAFE", f"lost keys: {sorted(lost)}"
        elif ok2:
            outcome = "fixed"
            detail = f"{applied}; comments {comments_before}->{comments_after}"
        else:
            outcome, detail = "partial", f"{applied}; still invalid: {err2.splitlines()[0][:70] if err2 else '?'}"
        rows.append({"id": path.stem, "expected": case.get("expected_category", "?"),
                     "actual": actual, "outcome": outcome, "detail": detail})

    # ------------------------------------------------------------- report --
    print(f"{'case':32} {'expected':20} {'actual':20} {'outcome':10} detail")
    print("-" * 130)
    for r in rows:
        print(f"{r['id']:32} {r['expected']:20} {r['actual']:20} {r['outcome']:10} {r['detail']}")

    by_cat: dict[str, dict[str, int]] = {}
    for r in rows:
        c = by_cat.setdefault(r["actual"], {"total": 0, "fixed": 0, "unsafe": 0})
        c["total"] += 1
        c["fixed"] += r["outcome"] == "fixed"
        c["unsafe"] += r["outcome"] == "UNSAFE"

    print(f"\n{'category':22} {'fixed':>7} {'total':>7} {'rate':>7} {'unsafe':>7}")
    print("-" * 55)
    for cat, c in sorted(by_cat.items()):
        rate = f"{100 * c['fixed'] // c['total']}%" if c["total"] else "-"
        print(f"{cat:22} {c['fixed']:>7} {c['total']:>7} {rate:>7} {c['unsafe']:>7}")

    total = len(rows)
    fixed = sum(r["outcome"] == "fixed" for r in rows)
    unsafe = sum(r["outcome"] == "UNSAFE" for r in rows)
    mismatched = sum(r["expected"] not in (r["actual"], "?") for r in rows)
    print(f"\nOVERALL  fixed {fixed}/{total} ({100 * fixed // total if total else 0}%)"
          f"  unsafe {unsafe}  category-mismatch {mismatched}")

    (ROOT / "plans" / "reports" / "compose-autofix-gate-results.json").write_text(
        json.dumps({"rows": rows, "by_category": by_cat,
                    "totals": {"fixed": fixed, "total": total, "unsafe": unsafe,
                               "category_mismatch": mismatched}}, indent=2) + "\n",
        encoding="utf-8")


if __name__ == "__main__":
    main()
