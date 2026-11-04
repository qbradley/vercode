# HISTORICAL external-only evaluation results

This report records `global-root-passes-v2`, not the current composable revision.
The 89/89 scores and speedups do not validate `global-root-frame-v3`.
The original results follow. The scored candidates remain unchanged.

The revised skill uses a frozen base followed by root-wide extension passes.
Fresh formats have no embedded struct sizes, enum payload sizes, or version words.
The caller supplies the complete root boundary.
Adopted formats retain their exact historical prefix.

Both configurations passed all 89 executable assertions.
The skill-assisted create output also passed the declared size and speed goals
against macro-based VerCode on all three current-version workloads.

## Coverage and trial design

GPT-6 Astra produced six isolated implementations: three cases with this skill
and three without it.
Both configurations received the same detailed task prompts and real library APIs.
Only the with-skill agents received the frozen skill snapshot.
Candidates did not receive hidden checks or other candidates.
Candidate library source remained unchanged during grading and measurement.

The baseline omitted this custom skill, not all general guidance.
Several baseline agents loaded generic `paw-essence` or `simple-english`.
These trials cannot establish a statistical effect from the skill alone.

| Case | With skill | Without this skill | Independent behavior tests |
|---|---:|---:|---:|
| Fresh nested/vector codec | 29/29 | 29/29 | 16 |
| Bincode adoption and update | 25/25 | 25/25 | 17 |
| Enums and new subtree birth | 35/35 | 35/35 | 23 |
| Automated total | 89/89 | 89/89 | 56 |
| Manual decision judgments | 27/27 | 25/27 | Not executable |

Automated assertions include compilation, formatting, Clippy, child-codec checks,
eighteen structural mutations per configuration, and immutable history.
The behavior checks cover exact prefixes, old/new readers, partial passes,
zero-byte passes, malformed values, external boundaries, and actual bincode bytes.

Nine decision prompts shared one context per configuration.
They are two batch trials, not eighteen independent samples.
Baseline omissions concerned partial later entries/defaults and count-only unit-vector construction.
These omissions are not observed runtime failures.
Manual grading was not blind.

Two rubric items were aligned with their unchanged prompts before scoring.
They previously demanded an unstated earlier-write policy and explicit test instructions.
The executable checks and coding prompts were not relaxed.

## Encoded size and speed

The create workload contains a root Point, 64 primitive samples, and many
Entry values with embedded Points.
The current version adds a global confidence pass and a label pass.
The macro reference represents the same values through per-node framing.
Both round trips were checked before timing.
External framing is excluded equally from both totals.

| Entries | Custom bytes | Macro bytes | Reduction | Write speedup | Owned-read speedup | Size speedup |
|---|---:|---:|---:|---:|---:|---:|
| 32 | 465 | 729 | 36.21% | 2.32x | 1.20x | 1.13x |
| 512 | 4,785 | 8,889 | 46.17% | 2.47x | 1.28x | 1.13x |
| 4,096 | 37,041 | 69,817 | 46.95% | 2.19x | 1.18x | 1.13x |

Speedup is the macro median divided by the custom median.
Byte savings equal `8 + 8*N` for `N` entries.
They include the removed root, Point, and Entry frame headers.

Each method used nine paired release-mode samples with alternating order.
The predeclared current-version policy required smaller output, lower medians,
and at least seven custom wins per nine pairs for every method and workload.
Both configurations passed.
With-skill write and size won all nine pairs.
Owned-read won nine, eight, and eight pairs respectively.

Base-version workloads were measured separately.
Their write/read paths improved, but with-skill sizing was approximately 13-15% slower.
Base-version sizing was outside the declared current-version acceptance scope.
The baseline's owned reads were faster than the skill-assisted candidate's reads.
The experiment does not show that the skill always produces the fastest implementation.

These are shared-host measurements, not universal speedup claims.
Allocation counts and per-implementation code size were not measured.
Compiler, target, raw samples, medians, and ranges are in [historical-external-results.json](historical-external-results.json).

## Measurement correction and tuning

The first benchmark failed the size-speed acceptance condition.
It scaled all methods' iteration counts with collection size.
The largest workload therefore measured only 300 constant-time size calls,
which produced sub-microsecond timing blocks.

The corrected harness uses one million calls for each size sample.
Write/read iteration counts and the acceptance threshold remain unchanged.
Both candidates were remeasured without library edits.
Initial failed results remain under `superseded_performance` in the machine-readable report.
The skill now tells agents to choose method-specific timing durations.

The generation trials used `custom-vercodable-append-v1`.
Final generation instructions and wire rules match that snapshot.
Only two measurement-duration sentences were added to `references/implementation.md` afterward.
That final documentation change did not receive another generation trial.

## Manual implementation observations

Paths below start at
`target/skill-evals/custom-vercodable-append-workspace/iteration-1/`.

| Property | Evidence |
|---|---|
| Explicit matching passes | `eval-create/with_skill/run-1/outputs/src/lib.rs:87-232` writes the complete base, all confidences, then the label. Size uses the same schedule without element traversal. |
| Complete-pass bounds | Create lines 143-203 validate the entire fixed-width vector and confidence extents before consumption. |
| Historical adoption | `eval-update/with_skill/run-1/outputs/src/lib.rs:82-236` appends enabled, stamp, and note after the complete bincode base. |
| Deferred variants | `eval-enums/with_skill/run-1/outputs/src/lib.rs:173-276` separates existing shape from newly introduced subtree payloads. |
| Safe unknown fallback | Enum lines 220-265 map unknown tags to a unit fallback with no payload in known passes. Lines 306-538 replay that same schedule. |
| Zerocopy | Enum lines 326-335 and 413-431 use safe Block byte APIs with explicit endian conversion and unaligned reads. |
| Hash coverage | Create lines 41-76 and enum lines 117-162 tie nominal container arguments to real fields and literal assertions. |
| Maintenance documentation | Create's README, update's source wire plan, and enum's source/REPORT.md document history, external framing, and update order. |
| Baseline design difference | `eval-create/without_skill/run-1/outputs/src/lib.rs:97-313` uses four serialization macros and cursor helpers. The skill-assisted output uses explicit methods. |

Unknown future variants are safe only under the frozen introduction rule:
their tags occupy established shape slots, while every payload byte belongs
to a later global pass.
Unknown legacy payloads beside tags remain unskippable without another boundary.
The enum checks exercise this distinction, not guessed payload lengths.

The update candidate uses an audited raw copy for its little-endian `Vec<u16>` path.
Its extent and allocation checks precede the copy.
Only native execution was performed.

## Reproducibility and limits

[README.md](README.md) gives preparation, grading, and benchmark commands.
[historical-external-results.json](historical-external-results.json) preserves assertions, source identities, snapshot hashes,
and both final and superseded timing data.
The frozen bincode-history fixture has SHA-256
`6d12211244a1bc4b5b2a0b39856cfad375e41cc22dbe348de6e9e8d59625b270`.

Twenty routing prompts remain unrun.
Bincode adoption has executable evidence; wincode adoption has guidance only.
There is no executed 32-bit/big-endian proof, exhaustive fuzzing, or general reliability estimate.
Exact executor timing and token counts are unavailable.

The earlier framed-format scores remain in
[legacy-framed-results.md](legacy-framed-results.md).
They do not apply to this format.
Generated candidates, logs, and the static viewer remain in the local workspace.
The viewer is `iteration-1/review.html`.

A Clippy exception applies only to the grader's import of immutable historical code.
The historical source itself remains unchanged.
All final skill-assisted crates pass all-target Clippy after grader injection.
