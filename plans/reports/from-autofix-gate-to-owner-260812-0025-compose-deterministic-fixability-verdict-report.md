# Compose auto-fix — go/no-go gate result

**Date:** 2026-08-12 · **Gate:** `260810-2258-colimaui-commercial-foundation/phase-05-compose-auto-fix-spike.md`
**Executed as:** Bước 0 of `260811-2245-pro-tier-features/phase-01`
**Artifacts:** `tests/compose-corpus/` (20 cases), `scripts/build-compose-corpus.py`,
`scripts/compose-autofix-gate.py`, `plans/reports/compose-autofix-gate-results.json`

## Verdict: **conditional go — narrow scope, apply for two categories only**

Ship *apply-a-patch* for `undefined_reference` (top-level volume/network
declarations) and `schema` (scalar→list coercion). Everything else ships as
**suggestion only**, with no apply button.

35% of the corpus was fixed deterministically overall. That number alone does not
justify a blanket auto-fix feature, and the per-category split is what matters:

| Category | Fixed | Total | Rate | Verdict |
|---|---:|---:|---:|---|
| `undefined_reference` | 3 | 5 | **60%** | **Apply** — declaration append, comment-safe |
| `schema` | 3 | 6 | **50%** | **Apply** — type coercion only, not key relocation |
| `yaml_syntax` | 1 | 5 | 20% | Suggest only — one lexical win (tabs), rest need AST |
| `missing_file` | 0 | 1 | 0% | Suggest only — cannot invent a user's file |
| `other` | 0 | 1 | 0% | Suggest only |

The plan anticipated this outcome explicitly: *"Nếu tỉ lệ thấp, kết luận đúng có
thể là 'chỉ ship gợi ý, không ship apply'."* The honest reading is in between —
two categories have genuinely safe rules; the rest do not.

## Method

Every case runs the product's real validator (`docker compose config --quiet`)
and a branch-for-branch port of `categorize()`. A prototype fixer is attempted,
the result re-validated, and keys diffed. **A fix that drops a key counts as a
failure, not a pass** — that is the silent-secret-deletion mode the plan calls out.

## Four findings, each verified directly against Docker Compose 5.4.0

### 1. `categorize()`'s `structure` branch is dead code — shipped bug

`compose_diagnose.rs:70` tests `services must be a mapping` and `no services`.
Docker emits exactly `services must be a mapping` for a list-shaped `services:` —
but that string contains **`must be a`**, which the *earlier* `schema` branch
(`:66`) already matches. The `structure` branch never sees it.

The other half fails differently: Docker says `empty compose file`, not
`no services`, so that string never matches either.

**Both arms are unreachable.** Confirmed: the list case classifies as `schema`,
the empty case as `other`. Fix is cheap — move the `structure` test above
`schema`, or narrow `schema`'s `must be a`. This is independent of auto-fix and
is worth fixing regardless.

### 2. An unset variable does not fail validation — it silently becomes `""`

```
$ docker compose -f undef-variable.yml config      # exit 0
warning: The "DB_PASSWORD" variable is not set. Defaulting to a blank string.
    POSTGRES_PASSWORD: ""
```

`categorize()` has a `variable is not set` arm, but `config --quiet` exits 0, so
the diagnose flow never reaches it. Worse than a classification gap: **an unset
database password validates clean as an empty password.** Detecting this requires
reading Docker's *warnings*, which the current path discards.

### 3. Missing build context also passes validation

`docker compose config` does not check that `build.context` exists — exit 0. The
`missing_file` category is therefore much narrower in practice than the name
suggests; it covers `env_file` and similar, not build inputs.

### 4. Docker reports only the *first* undefined reference

`undef-multiple-volumes` has two. Fixing the reported one surfaces the next, so
the run ended `partial`. **Any fixer must iterate to a fixpoint**, re-validating
after each pass, rather than assuming one error means one fix.

## Design consequence: `key_diff_check` as specified blocks legitimate fixes

The one `UNSAFE` result is instructive. Correcting the typo `imge:` → `image:`
*removes the key* `imge`, so a naive "no key may disappear" check rejects the very
fix it was meant to allow.

The plan already allows for this — *"trừ khi patch nói rõ là xoá key đó"* — and
this run turns that clause from a nicety into a hard requirement: **a patch must
declare its intended key removals, and `key_diff_check` must diff against
`observed_removals − declared_removals`.** Without that, typo correction is
impossible; with a naive check, either the feature or the safety net gets dropped.

## Limitations — read the 35% with this attached

**All 20 cases are synthetic.** No real bug-report corpus exists (Free P2 is
unbuilt), so these are failures I thought of, which is exactly the
"corpus tự sướng" bias the plan warns about. Real files are messier, larger, and
fail in combinations. Two corpus cases turned out not to be broken at all
(findings 2 and 3) — evidence that even the authoring assumptions were off.

Treat the per-category verdicts as **provisional**. Re-run this gate against real
files once Free P2 (bug reports) lands; `scripts/compose-autofix-gate.py` takes a
corpus directory argument for exactly that.

## Recommended scope for phase 1

1. `key_diff_check` with declared-removals — **write first**, per the plan.
2. Fixpoint iteration, not single-pass.
3. Apply for: undefined volume/network declaration; scalar→list coercion; tab
   de-indentation. All three preserve comments (verified: the comment probe case
   kept 3/3).
4. Suggest-only for: yaml structural errors, missing files, secrets, unset
   variables, key relocation.
5. Fix the `categorize()` ordering bug (finding 1) — it mislabels input to
   everything downstream.
6. Surface Docker's warnings, not just its errors (finding 2), or unset variables
   remain invisible.

## Unresolved

- Should the unset-variable case (finding 2) become a Free diagnostic rather than
  a Pro fix? It is a correctness problem for every user, and it is currently
  invisible in both tiers.
- Fixing `categorize()` changes the `error_signature`/Knowledge Bank grouping for
  previously-mislabelled errors. Worth checking whether any KB entries were
  recorded under `schema` that should have been `structure`.
