#!/usr/bin/env python3
"""Generate the broken-compose evaluation corpus for the auto-fix go/no-go gate.

The corpus is generated rather than hand-collected because no real bug-report
corpus exists yet (Free P2 is unbuilt). Every case is therefore marked
`origin: synthetic`, and the report must not read a synthetic pass rate as
evidence about real user files — synthetic cases are cleaner than reality and
biased toward whatever the author thought of.

Each case is derived from a branch of `categorize()`
(`src-tauri/src/commands/compose_diagnose.rs:45`) so the corpus exercises the
classifier the product actually ships, not an idealised one.

`expected_category` is the author's intent. The harness records what
`categorize()` *actually* returns; disagreements are findings, not bugs to hide.
"""

import json
import pathlib

CORPUS = pathlib.Path(__file__).resolve().parent.parent / "tests" / "compose-corpus"

CASES = [
    # ---- yaml_syntax -------------------------------------------------------
    {
        "id": "yaml-tab-indent",
        "expected_category": "yaml_syntax",
        "what": "Tab character used for indentation; YAML forbids tabs.",
        "fix_theory": "Replace leading tabs with spaces. Purely lexical, no AST needed.",
        "content": "services:\n\tweb:\n\t\timage: nginx:alpine\n",
    },
    {
        "id": "yaml-missing-colon",
        "expected_category": "yaml_syntax",
        "what": "Key/value separator missing after `image`.",
        "fix_theory": "Ambiguous: cannot tell intended key boundary without guessing.",
        "content": "services:\n  web:\n    image nginx:alpine\n    ports:\n      - \"80:80\"\n",
    },
    {
        "id": "yaml-unclosed-quote",
        "expected_category": "yaml_syntax",
        "what": "Unterminated double quote in a port mapping.",
        "fix_theory": "Could append the closing quote, but the intended end position is a guess.",
        "content": "services:\n  web:\n    image: nginx\n    ports:\n      - \"80:80\n",
    },
    {
        "id": "yaml-bad-nesting",
        "expected_category": "yaml_syntax",
        "what": "Service block under-indented, breaking the mapping.",
        "fix_theory": "Structural. Requires understanding intended hierarchy; comments lost on rewrite.",
        "content": "services:\n  web:\n    image: nginx\n   ports:\n      - \"80:80\"\n",
    },
    {
        "id": "yaml-duplicate-key",
        "expected_category": "yaml_syntax",
        "what": "`image` specified twice for one service.",
        "fix_theory": "Deleting one is a semantic choice — which one did the user mean?",
        "content": "services:\n  web:\n    image: nginx:alpine\n    image: nginx:latest\n",
    },
    # ---- schema ------------------------------------------------------------
    {
        "id": "schema-typo-key",
        "expected_category": "schema",
        "what": "`imge` instead of `image` — a one-character typo.",
        "fix_theory": "Edit distance against the known key set gives an unambiguous candidate.",
        "content": "services:\n  web:\n    imge: nginx:alpine\n",
    },
    {
        "id": "schema-ports-scalar",
        "expected_category": "schema",
        "what": "`ports` given as a bare number instead of a sequence.",
        "fix_theory": "Coerce scalar to a single-element quoted sequence. Type-directed, deterministic.",
        "content": "services:\n  web:\n    image: nginx\n    ports: 8080\n",
    },
    {
        "id": "schema-unsupported-option",
        "expected_category": "schema",
        "what": "`restart_policy` at service level (it belongs under `deploy`).",
        "fix_theory": "Relocating it is a real semantic move; safe only with confirmation.",
        "content": "services:\n  web:\n    image: nginx\n    restart_policy: always\n",
    },
    {
        "id": "schema-environment-wrong-type",
        "expected_category": "schema",
        "what": "`environment` given as a string instead of map/list.",
        "fix_theory": "Splitting `KEY=value` into a one-entry list is mechanical.",
        "content": "services:\n  web:\n    image: nginx\n    environment: FOO=bar\n",
    },
    {
        "id": "schema-depends-on-scalar",
        "expected_category": "schema",
        "what": "`depends_on` as a scalar rather than a list.",
        "fix_theory": "Wrap in a list. Mechanical.",
        "content": "services:\n  web:\n    image: nginx\n    depends_on: db\n  db:\n    image: postgres:16\n",
    },
    # ---- undefined_reference ----------------------------------------------
    {
        "id": "undef-volume",
        "expected_category": "undefined_reference",
        "what": "Service mounts `dbdata`, which is never declared top-level.",
        "fix_theory": "Append `volumes: {dbdata: }`. The name is known; nothing is guessed.",
        "content": "services:\n  db:\n    image: postgres:16\n    volumes:\n      - dbdata:/var/lib/postgresql/data\n",
    },
    {
        "id": "undef-network",
        "expected_category": "undefined_reference",
        "what": "Service joins `backend`, which is never declared.",
        "fix_theory": "Append the network declaration. Same shape as the volume case.",
        "content": "services:\n  web:\n    image: nginx\n    networks:\n      - backend\n",
    },
    {
        "id": "undef-secret",
        "expected_category": "undefined_reference",
        "what": "Service uses secret `db_password`, never declared.",
        "fix_theory": "A secret needs a source (file/external) — the app cannot invent one.",
        "content": "services:\n  db:\n    image: postgres:16\n    secrets:\n      - db_password\n",
    },
    {
        "id": "undef-variable",
        "expected_category": "undefined_reference",
        "what": "`${DB_PASSWORD}` interpolated but not set anywhere.",
        "fix_theory": "The value is unknowable. A default would silently change behaviour.",
        "content": "services:\n  db:\n    image: postgres:16\n    environment:\n      POSTGRES_PASSWORD: ${DB_PASSWORD}\n",
    },
    {
        "id": "undef-multiple-volumes",
        "expected_category": "undefined_reference",
        "what": "Two undeclared volumes across two services.",
        "fix_theory": "Same as single case but must fix all, not just the first reported.",
        "content": (
            "services:\n  db:\n    image: postgres:16\n    volumes:\n      - dbdata:/var/lib/postgresql/data\n"
            "  cache:\n    image: redis:7\n    volumes:\n      - cachedata:/data\n"
        ),
    },
    # ---- missing_file ------------------------------------------------------
    {
        "id": "missing-env-file",
        "expected_category": "missing_file",
        "what": "`env_file` points at a file that does not exist.",
        "fix_theory": "Creating it changes runtime behaviour; removing it drops config. Neither is safe alone.",
        "content": "services:\n  web:\n    image: nginx\n    env_file:\n      - ./nonexistent.env\n",
    },
    {
        "id": "missing-build-context",
        "expected_category": "missing_file",
        "what": "Build context directory does not exist.",
        "fix_theory": "Cannot create someone's Dockerfile. Not fixable by the app.",
        "content": "services:\n  app:\n    build:\n      context: ./no-such-dir\n",
    },
    # ---- structure ---------------------------------------------------------
    {
        "id": "structure-services-list",
        "expected_category": "structure",
        "what": "`services` written as a list instead of a mapping.",
        "fix_theory": "Convertible in principle, but a full re-serialisation — comments are lost.",
        "content": "services:\n  - name: web\n    image: nginx\n",
    },
    {
        "id": "structure-no-services",
        "expected_category": "structure",
        "what": "File has only a version key; no services at all.",
        "fix_theory": "Nothing to infer. The user must say what they want to run.",
        "content": "version: \"3.9\"\n",
    },
    # ---- comment preservation probe ---------------------------------------
    {
        "id": "undef-volume-with-comments",
        "expected_category": "undefined_reference",
        "what": "Undeclared volume, in a file carrying comments that must survive the fix.",
        "fix_theory": "Line-append should preserve comments; a YAML round-trip would destroy them.",
        "content": (
            "# Production database stack\n"
            "# Owner: platform team\n"
            "services:\n"
            "  db:\n"
            "    image: postgres:16  # pinned deliberately\n"
            "    volumes:\n"
            "      - dbdata:/var/lib/postgresql/data\n"
        ),
    },
]


def main() -> None:
    CORPUS.mkdir(parents=True, exist_ok=True)
    manifest = []
    for case in CASES:
        path = CORPUS / f"{case['id']}.yml"
        path.write_text(case["content"], encoding="utf-8")
        manifest.append({
            "id": case["id"],
            "file": path.name,
            "expected_category": case["expected_category"],
            "what": case["what"],
            "fix_theory": case["fix_theory"],
            "origin": "synthetic",
        })
    (CORPUS / "manifest.json").write_text(
        json.dumps({"origin_note": "All cases synthetic; no real bug-report corpus exists yet.",
                    "cases": manifest}, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"{len(manifest)} cases written to {CORPUS}")
    by_cat: dict[str, int] = {}
    for m in manifest:
        by_cat[m["expected_category"]] = by_cat.get(m["expected_category"], 0) + 1
    for cat, n in sorted(by_cat.items()):
        print(f"  {cat:22} {n}")


if __name__ == "__main__":
    main()
