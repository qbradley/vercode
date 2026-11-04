# Historical framed-format evaluation results

These results apply to the earlier per-node framed skill at commit `a8f03e6`.
They do not evaluate the current root-wide append-only format.
The old fixtures and harness are available at that commit.

The tuned skill passed all 60 automated assertions across three fresh coding cases.
The paired baseline passed 57.
Both configurations passed all 36 independent Rust behavior tests.
The difference came from compile-time guards, not observed wire failures.

## Trial design

GPT-6 Astra produced one implementation per case and configuration in each iteration.
The two iterations contain twelve implementation runs.
Each run used an isolated crate and Cargo artifact directory.
Agents received the same task and real library APIs.
Only the with-skill agents received the skill and its references.
Agents did not receive the independent grader.

Nine decision prompts ran in one shared-context batch per configuration during iteration 1.
These two batches are not eighteen independent samples.
Twenty trigger prompts exist, but actual routing trials did not run.

## Scores

| Evaluation | Initial skill | Initial baseline | Tuned skill | Fresh baseline |
|---|---:|---:|---:|---:|
| Create a nested codec | 19/21 | 19/21 | 21/21 | 19/21 |
| Update a released codec | 20/20 | 19/20 | 20/20 | 19/20 |
| Enums and zerocopy | 19/19 | 19/19 | 19/19 | 19/19 |
| Automated total | 58/60 | 57/60 | 60/60 | 57/60 |
| Decision judgments | 27/27 | 25/27 | Not repeated | Not repeated |

Each automated total includes 36 behavior tests, compilation, formatting, Clippy, child-codec checks, and structural mutations.
The update case also compares the historical implementation byte for byte.
Any failed compatibility or guard assertion blocks adoption, regardless of the total score.

The decision baseline omitted metadata-assertion detail and version-gap test detail.
These judgment failures are not observed runtime defects.
The behavior tests did not distinguish the skill from the baseline.

## What changed after evaluation

`hashed-type-def` 0.1.2 gives `Vec<T>` and `Option<T>` nominal hashes.
Those hashes omit the argument `T`.
A derived root hash can therefore miss an embedded definition or argument change.

Both initial create outputs missed this gap.
The initial skill-assisted update independently added a field-inferred witness.
The baseline update did not.

The revised skill names the gap and requires witnesses tied to actual field types.
It requires coverage across every missing container edge, including nested containers.
The grader gained `Vec<u16>` to `Vec<i16>` and `Option<bool>` to `Option<u8>` mutations.
Both initial configurations were regraded against these stronger checks.

Fresh skill-assisted implementations passed these mutations.
Fresh baselines repeated the same three guard failures.
The skill also gained guidance about target-width-dependent array hashes.
No expected hash literals or fixture-specific bytes were added to its instructions.

## Grader corrections

Shared Cargo artifacts initially mixed identically named candidate crates.
The grader now isolates artifacts per run.
A lexical child-codec check also matched a same-named independent test oracle.
The replacement compiles a trait requirement against the actual public child type.

The final cleanup moved shared test helpers outside Cargo's integration-test discovery.
Both iterations and configurations were regraded without edits to candidate library source.
Seven Python tests cover mutation behavior and report export.
The three final skill-assisted crates also passed Clippy across all targets after grader injection.

## Manual review

Paths in this table start at
`target/skill-evals/custom-vercodable-workspace/iteration-2/`.
They identify the evaluated source, not a repaired submission.

| Property | Evidence and result |
|---|---|
| Matching traversals | `eval-create/with_skill/run-1/outputs/src/lib.rs:106-198` uses corresponding fixed runs and version boundaries. |
| Uniformity limitation | `eval-update/with_skill/run-1/outputs/src/lib.rs:185-203` adds the stamp size after values and enabled. Write/read place stamp inside Child first. Totals are correct, but traversal order is less uniform. |
| Inlined frames | Create lines 141-159 and update lines 124-157 bound the child frame and advance by its declared length. |
| No writer sizing preflight | Create lines 106-138, update lines 94-122, and enum lines 159-197 and 309-339 write directly into the caller's buffer. |
| Container guards | Create lines 215-234 and update lines 79-89 infer arguments from actual fields. |
| Safe zerocopy | `eval-enums/with_skill/run-1/outputs/src/lib.rs:309-369` retains Block framing and uses safe byte APIs with explicit endian conversion. |
| Update instructions | Create lines 42-67, update lines 32-53, and enum lines 36-49 document history, guards, and future updates. |
| Performance | The create harness provides independent measurements. Agent reports include workload limits and slower update paths. No general speedup is established. |

The manual review was not blind.
Its observations are separate from the automated score.
In particular, 60/60 does not mean that every output has ideal traversal uniformity.

## Exploratory performance

The independent create benchmark used release mode on native x86_64 Linux.
It compared equivalent current-version values against a derive oracle.
Each workload used seven timing blocks.
The 0/8-element workloads used 200,000 operations per block.
The 4096-element workload used 10,000.
The two configurations ran sequentially.

Final skill-assisted medians, in nanoseconds per operation:

| Vector elements | Write custom / derive | Owned read custom / derive | Size custom / derive |
|---|---:|---:|---:|
| 0 | 7.565 / 9.566 | 27.642 / 37.763 | 1.719 / 1.719 |
| 8 | 7.930 / 9.150 | 40.293 / 49.479 | 1.722 / 1.719 |
| 4096 | 124.541 / 129.151 | 160.522 / 174.612 | 1.730 / 1.730 |

These are shared-host measurements, not general speedup claims.
Crate boundaries and inlining can affect the comparison.
The benchmark does not measure allocation counts or per-implementation code size.
The machine-readable report includes both configurations and all timing ranges.

## Evidence and limits

[legacy-framed-results.json](legacy-framed-results.json) preserves the historical assertions, source hashes, and independent performance data.
The frozen historical source has SHA-256
`7360df4f90cef60877f544b8a9b9f177584060efb002b31b335e1ca1df00c115`.
The evaluated skill snapshot matches `SKILL.md` and references at commit `a8f03e6`.

Generated implementations, compiler logs, reports, and static review pages remain under
`target/skill-evals/custom-vercodable-workspace/`.
The final viewer is `iteration-2/review.html`.
These local artifacts are not tracked repository files.
The evaluation README at commit `a8f03e6` gives the historical reproduction commands.

Only native little-endian execution occurred.
There is no executed 32-bit or big-endian proof, exhaustive fuzzing, or general reliability estimate.
Recursive, generic, and foreign-type cases received decision-level evaluation, not executable implementations.
Exact executor durations and token counts are unavailable.
The report makes no token-cost or task-duration claim.
