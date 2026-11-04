# Composable-frame evaluation status

The active wire revision is `global-root-frame-v3`.
No fresh end-to-end codec-generation trials or codec-versus-macro performance measurements validate this revision yet.
Harness smoke checks are not model evaluations.

The active suite requires exact consumption of one future-inclusive frame.
Create and enums use one leading total length.
Update preserves the exact bincode base and adds a mandatory length of all extension bytes.
Even the post-bootstrap version-zero adapter writes a zero-extension header.
Bare pre-bootstrap bincode requires explicit migration or a separate legacy mode.

The previous external-only evidence remains in
[historical-external-results.md](historical-external-results.md) and
[historical-external-results.json](historical-external-results.json).
Its 89/89 scores and speedups apply only to `global-root-passes-v2`.
The older per-node results remain in
[legacy-framed-results.md](legacy-framed-results.md) and
[legacy-framed-results.json](legacy-framed-results.json).

Neither archive validates this revision.
The scored sources and reports remain historical artifacts.

## Checked typed fields, unreleased 0.7.1

This section records the earlier 0.7.1 snapshot, before macro integration in 0.8.0.
The crate adds `WireField` and `WirePrimitive` without changing the wire grammar.
Six focused integration tests cover integer encodings, malformed ranges, unaligned slices, atomic field failures, and an evolving codec.
Workspace tests, doctests, formatting, strict Clippy, and the 32 Python harness/package tests passed.
The typed-field example in the skill also compiled against the actual public interface.

The same-binary benchmark compares manual offsets with the checked public helper across the crate interface.
It covers a 24-byte run and a 1,116-byte nested encoding with 128 historical checksum frames.
Both paths use the same output buffer and indirect-call timing loop.
The declared gate permits at most 5% median overhead per workload.

The longer trial used nine alternating pairs of 20 million operations per sample.
It ran on CPU 0 of an Intel Xeon Platinum 8370C, with `rustc 1.99.1` and the workspace release profile.
These helper/manual median ratios passed the unchanged gate:

| Workload | Write | Owned read | Size |
|---|---:|---:|---:|
| Small run | 1.0310 | 0.9888 | 0.9995 |
| Nested array | 0.9973 | 0.9996 | 0.9471 |

These results establish a local non-regression result, not a helper-specific or application-wide speedup.
Targets other than `x86_64-unknown-linux-gnu` were not measured or executed.
The small readers and nested writers have matching instructions and constant targets.
Nested readers differ only in the order of two independent additions.
Both nested readers retain the same two `memcpy` calls and the same code size.
No field-helper calls remain in these measured read/write functions.

The initial one-million-iteration trial failed three gates.
After the timing-loop and buffer correction, the five-million-iteration trial failed two gates.
The longer passing trial used that same corrected binary.
All failed samples remain recorded.
Local raw timings, frozen sources, binary hashes, assembly, and hardware metadata are in `target/wire-field-evidence/`.

## Typed-field decision evaluations

Cases 14 and 15 cover the six requested adoption, evolution, validity, availability, and evidence scenarios.
Two iterations compared fresh GPT-6 Astra batches with the updated skill and the parent skill.
Each configuration had one batch per iteration, with both prompts in the same context.
The prompts are not independent model samples.

The first outputs omitted explicit offset-overflow checks and parts of the public-interface evidence policy.
The skill now names those checks and requires a separate consumer crate for the isolated helper comparison.
The first prompt also omitted the constructor's constness.
The second iteration corrected that ambiguity and made the malformed-range and evidence questions explicit.
These prompt changes prevent attribution of score changes solely to the skill.

Both final configurations scored 8/9 on the compound rubric.
Neither explicitly mentioned the target-width `usize`/`isize` warning, although every proposed field used a fixed-width integer.
The skill already states this restriction.
The remaining failure is an omitted warning, not an observed invalid encoding.
These small decision batches do not establish an advantage over the parent skill.

The outputs are sketches, not executed generated codecs.
End-to-end codec-generation and routing trials remain unrun for this revision.
Local answers, grading evidence, immutable skill snapshots, and static review reports are in
`target/skill-evals/custom-vercodable-wire-fields-workspace/`.
The final viewer is `iteration-2/review.html` in that directory.

## Macro integration, unreleased 0.8.0

Derived root lengths and enum tags now use `WireField`.
The twelve unrestricted integer codecs delegate to `WirePrimitive`.
Golden header checks cover unaligned empty, named, tuple, and enum encodings.
Workspace tests, doctests, formatting, strict Clippy, and all 32 harness/package checks passed.
Cargo rejected the new macro crate for the old 0.7 dependency range and resolved the exact `=0.8.0` pin.

The existing helper benchmark passed all six 5% non-regression gates on the rebuilt 0.8.0 binary.
It used nine alternating pairs of 20 million operations per sample on CPU 0.

| Workload | Write | Owned read | Size |
|---|---:|---:|---:|
| Small run | 0.9906 | 1.0063 | 1.0164 |
| Nested array | 0.9817 | 1.0154 | 1.0043 |

These are helper/manual median ratios, not before/after macro timings.
No macro-specific speedup is established.
The version-only skill prompt update did not trigger new model or routing trials.
The earlier 0.7.1 records remain unchanged.
Local frozen sources, binary identity, raw samples, assembly, and dependency checks are in
`target/wire-field-evidence/macro-integration-0.8.0/`.
