#!/usr/bin/env bash
#
# Measure how much of a broken-compose-file corpus `docker compose config`
# catches on its own.
#
# This number decides the Phase 5 go/no-go gate: if Docker's own validator
# already explains most failures, there is no reason to write a rule engine.
#
# Usage:
#   scripts/compose-diagnose-benchmark.sh <corpus-dir> [output.csv]
#
# The corpus directory should contain real broken compose files, one per case,
# named *.yml or *.yaml. Files are never modified.
#
# Output: a per-file verdict on stdout, a summary, and a CSV for the report.

set -uo pipefail

CORPUS_DIR="${1:-}"
OUT_CSV="${2:-compose-benchmark.csv}"

if [ -z "$CORPUS_DIR" ] || [ ! -d "$CORPUS_DIR" ]; then
  echo "usage: $0 <corpus-dir> [output.csv]" >&2
  echo "corpus-dir must contain broken compose files (*.yml / *.yaml)" >&2
  exit 2
fi

if ! docker compose version >/dev/null 2>&1; then
  echo "error: 'docker compose' is not available. Start Colima first." >&2
  exit 2
fi

# This script deliberately does NOT classify errors. An earlier version kept a
# copy of categorize() from compose_diagnose.rs here, and the two drifted apart
# immediately — the copy reported "other" for errors the app classified fine.
# The raw message goes into the CSV instead; group it there if you need buckets.

total=0
caught=0
missed=0

echo "file,caught_by_docker_config,error_first_line" > "$OUT_CSV"

while IFS= read -r -d '' file; do
  total=$((total + 1))
  stderr=$(docker compose -f "$file" config --quiet 2>&1)
  status=$?
  name=$(basename "$file")

  if [ $status -eq 0 ]; then
    # Docker sees nothing wrong. Either the file is fine, or the failure only
    # shows up at runtime — which is exactly the gap a rule engine would fill.
    missed=$((missed + 1))
    echo "MISS  $name (config reports no error)"
    echo "\"$name\",no," >> "$OUT_CSV"
  else
    caught=$((caught + 1))
    first_line=$(echo "$stderr" | head -1 | tr -d '"' | tr '\n' ' ')
    echo "CATCH $name"
    echo "  $first_line"
    echo "\"$name\",yes,\"$first_line\"" >> "$OUT_CSV"
  fi
done < <(find "$CORPUS_DIR" -type f \( -name '*.yml' -o -name '*.yaml' \) -print0)

echo
echo "=============================================="
if [ "$total" -eq 0 ]; then
  echo "No compose files found in $CORPUS_DIR"
  exit 1
fi

pct=$(( caught * 100 / total ))
echo "corpus size      : $total"
echo "caught by config : $caught (${pct}%)"
echo "missed           : $missed"
echo "csv              : $OUT_CSV"
echo
echo "Go/no-go reference (Phase 5, Bước B):"
echo "  >=60% and suggestions useful -> do not build a rule tier"
echo "  <30% and the LLM is also weak -> stop; pick a different flagship"
echo "  in between -> at most 3 rules, only for classes Docker reports poorly"
echo
echo "Note: this measures detection only. Whether the resulting advice is"
echo "useful is a separate, manual judgement that must also be recorded."
echo "=============================================="

if [ "$total" -lt 15 ]; then
  echo
  echo "WARNING: corpus has $total files; Phase 5 asks for at least 15." >&2
  echo "The percentage above is too small a sample to decide the gate on." >&2
fi
