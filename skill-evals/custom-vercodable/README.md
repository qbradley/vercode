# Composable global-pass codec evaluations

The active wire revision is `global-root-frame-v3`.
The suite evaluates root-wide append-only payloads inside one composable frame.
It does not require byte equality with the macro format.
It requires semantic equivalence, exact consumption, smaller fresh encodings, and separately measured speed goals.

**No fresh model trials or performance campaign validate this revision yet.**
The previous 89/89 scores and speedups are historical external-only evidence.
They remain in [historical-external-results.md](historical-external-results.md).
The older per-node results remain in [legacy-framed-results.md](legacy-framed-results.md).
Neither archive validates this revision.

## Layout

The installable skill lives at
[`vercode/agent-plugin/skills/custom-vercodable/`](../../vercode/agent-plugin/skills/custom-vercodable/).
This evaluation directory stays outside the plugin and published crate.
Catalog fixture paths are relative to this directory.
The runner resolves the repository, crate, and canonical skill from its own location.

Archived reports preserve their original paths and hashes from `.github/skills/custom-vercodable/`.
Those paths describe historical layouts, not current reproduction commands.
The plugin refactor does not change the codec instructions or validate new model results.

## Wire contract

`read_version` returns the exact length of ONE complete value, including unknown future bytes.
It preserves every following sibling in the input remainder, even at older requested versions.
Returning `buf.len()`, only known bytes, or `size_version(decoded)` violates this contract.
Version alone and external-only bounds do not provide generic composition.

Create and enums use one leading `u32` LE total length, including the four-byte prefix.
The payload retains the complete global-pass layout without nested struct frames.
Update uses the exact frozen bincode base, followed by a mandatory `u32` LE length of ALL extension bytes.
All extension passes follow that header.
The earliest simulated update readers are explicitly **post-bootstrap adapters**.
Even their version-zero encoding contains the zero-extension header.

Real pre-bootstrap bincode has no such header.
It requires explicit migration or a separate legacy mode.
EOF or a following sibling cannot reliably identify an absent header.
Unchanged bincode readers can accept trailing bytes without consuming them.
Exact bincode comparisons use only the base projection, excluding extension metadata.

Whole-body prefixes are append-only, not mutable header words.
For update, both the frozen base and the extension payload preserve their prefixes.
A truncated declared frame fails, even at a historical payload endpoint.
A historical endpoint defaults later fields only after its declared length is correctly reduced.

## Code cases

| Case | Coverage |
|---|---|
| `create` | Single prefix, nested structs and vectors, complete global passes, options, payload prefixes, size and speed versus macros |
| `update` | Actual bincode base, mandatory extension header, frozen post-bootstrap history, global child/root additions, four generations |
| `enums` | Deferred variant payloads, safe unknown fallback and strict rejection, new subtree shape, later descendants, unframed zerocopy |

The hidden Rust checks use independent wire models and historical codecs.
They do not infer compatibility from current-code round trips.
The grader also checks production compilation, formatting, Clippy, and structural mutations.
Deterministic malformed-input samples supplement targeted cases, not exhaustive fuzzing.
Checks cover unknown future bytes followed by a sibling, generic vectors and tuples,
and macro-derived parents with ordinary and versioned trailing fields.
They retain the evolution, enum, hash, and malformed-payload checks.

## Run a paired trial

Run these commands from the repository root:

```sh
python3 skill-evals/custom-vercodable/scripts/run_evals.py prepare create \
  target/skill-evals/custom-vercodable-composable-workspace/iteration-1/eval-create/with_skill/run-1/outputs
python3 skill-evals/custom-vercodable/scripts/run_evals.py prepare create \
  target/skill-evals/custom-vercodable-composable-workspace/iteration-1/eval-create/without_skill/run-1/outputs
```

Give each output directory to a fresh implementation agent.
Give both agents the same prompt from `evals.json`.
Give only the with-skill agent the evaluated `SKILL.md` and references.
Allow both agents to inspect the installed VerCode source.
Keep hidden checks and other candidates outside their context.

For the baseline agent, disable the `vercode` plugin and any other `custom-vercodable` copy.
The baseline must not discover the skill through a project, personal, or plugin location.

Ask each agent to save compiling source, its own tests, and `REPORT.md`.
Keep each agent inside its assigned directory.
Use isolated Cargo artifact directories.
Use the same model, tools, and validation permissions for both configurations.
Record the actual model and available telemetry.

After both agents finish, run:

```sh
python3 skill-evals/custom-vercodable/scripts/run_evals.py grade create \
  target/skill-evals/custom-vercodable-composable-workspace/iteration-1/eval-create/with_skill/run-1/outputs
python3 skill-evals/custom-vercodable/scripts/run_evals.py grade create \
  target/skill-evals/custom-vercodable-composable-workspace/iteration-1/eval-create/without_skill/run-1/outputs
```

Repeat for `update` and `enums`.
After revising the skill, use a fresh iteration and fresh agents.
Preserve scored submissions instead of repairing them silently.
Preparation refuses to overwrite an existing output directory.
The runner rejects both historical workspaces and copied runs with historical revision metadata.
Grading requires current candidate metadata.
Result aggregation rejects mixed wire revisions.
Cargo artifacts use a revision-specific directory.

## Encoded size and performance

After the create behavior checks pass and other jobs finish, run:

```sh
python3 skill-evals/custom-vercodable/scripts/run_evals.py benchmark-create \
  target/skill-evals/custom-vercodable-composable-workspace/iteration-1/eval-create/with_skill/run-1/outputs \
  --confirm-no-evaluation-jobs
```

Run the same command for the baseline afterward.
Keep benchmark runs sequential.

The release harness compares equal values against independent macro-derived types.
It checks each round trip before timing.
The byte layouts intentionally differ.
Encoded-size checks require fewer custom bytes for nontrivial nested values.
Both totals include every byte required for composition.
Create savings are exactly `4 + 8*N` bytes for `N` entries.
The benchmark asserts both this formula and the encoded total-length word.

Measurements separate buffer writes, owned reads, and size calculations.
They use paired samples with alternating measurement order and reusable write buffers.
Size samples use one million calls because constant-time sizing needs longer timing blocks.
The report includes encoded sizes, raw timings, medians, and per-method outcomes.
The acceptance policy requires lower custom medians and at least seven wins
in nine paired samples on the declared current-version workloads.

A slower or noisy method is a failed performance goal.
Report it independently of correctness and byte savings.
Shared-host measurements do not prove universal speedups.
Record code changes or repeat trials explicitly rather than selecting favorable samples.

## Correctness and manual review

`grading.json` contains assertions and evidence paths.
`logs/` contains compiler and test output.
The grader injects namespaced checks under `tests/skill_eval_contract.rs`
and `tests/skill_eval_support/`.
Hash mutations use temporary copies, not candidate library edits.

A guard passes only if ordinary `cargo check --lib` reports `E0080` with
an update diagnostic. An unrelated compilation error does not pass.
The actual-child-codec check uses a trait requirement on the public child type.
Same-named test oracles do not count as child implementations.

Review these properties separately:

- The three methods share the same root-wide pass schedule.
- Existing counts and tags are not repeated by extension passes.
- Newly introduced containers emit shape once, before their later descendants.
- Readers distinguish global boundaries from partial fields or elements.
- The reader consumes exactly one full declared frame and preserves all following values.
- Older readers include unknown future extensions in their consumed length.
- After-base adoption writes the extension header even when its declared byte length is zero.
- Updates preserve history and actionable hash instructions.
- Zerocopy operations have valid layout, endian, and alignment proofs.
- Performance claims describe the measured workload and failures.

Record source locations for each manual judgment.
Treat compatibility and guard failures as adoption blockers.
Do not turn an aggregate score into a universal guarantee.

## Decision and routing cases

The remaining catalog prompts address graph and migration decisions.
They cover unknown enum payloads, topology replay, empty passes, generic and
foreign hash coverage, and historical decoder settings.
The write-specialization case covers distinct variant layouts and packed historical checksum frames.
It requires byte preservation and a measurement plan, not an inference from read speed or store counts.
The typed-field cases cover fresh runs, macro adoption, later variable passes, restricted values,
older dependencies, and performance-neutral refactors.
The crate's `wire_field_tests` and `wire_fields` benchmark exercise the helper independently of model trials.
Their results do not establish that an agent used the skill correctly.

Use `prepare-decisions OUTPUT_DIRECTORY` for a prompts-only batch.
Save responses as an `answers.json` array of `{id, answer}` objects.
Grade each answer against the catalog expectations.
If prompts share one agent context, report one batch trial per configuration.
Do not count prompts as independent model samples.

`trigger-evals.json` contains twenty routing prompts.
Routing requires observing actual skill invocation, not matching answer keywords.
Without that observation, report these cases as unrun.

## Tuning and reporting

Classify each failure as a skill defect, model error, or grader defect.
For grader defects, correct the check and regrade both configurations.
For skill defects, generalize the instruction and use fresh agents.
Preserve old results and source hashes.

After skill-creator aggregates the iteration, record the evaluated snapshot:

```sh
python3 skill-evals/custom-vercodable/scripts/run_evals.py review-metadata \
  target/skill-evals/custom-vercodable-composable-workspace/iteration-1 \
  --model ACTUAL_MODEL --skill-path PATH_TO_EVALUATED_SNAPSHOT
python3 skill-evals/custom-vercodable/scripts/run_evals.py record-results \
  target/skill-evals/custom-vercodable-composable-workspace \
  skill-evals/custom-vercodable/results.json
```

The report preserves assertion evidence, source hashes, and performance data.
Unavailable telemetry is omitted, not reported as zero.
[results.md](results.md) records current status and links to historical evidence.

## Harness-only validation

These commands do not start model trials or benchmarks:

```sh
python3 -m unittest discover -s skill-evals/custom-vercodable/scripts -p 'test_*.py'
rustfmt --edition 2024 --config skip_children=true --check skill-evals/custom-vercodable/fixtures/*.rs \
  skill-evals/custom-vercodable/checks/*.rs
```

The Python suite requires Cargo and checks the standalone bundle and crate package contents.
It also checks the evaluation runner without compiling candidate codecs.
CI runs this suite from the same location.

For smoke checks, use new project-local directories and clearly labeled adapted candidates.
Never edit or regrade the scored historical sources.
Wrapping an archived raw-body implementation checks the harness, not the skill.
Compile the benchmark source without running it when no performance campaign is requested.

When the archived candidates are available, this command runs the labeled smoke checks:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 skill-evals/custom-vercodable/scripts/smoke_evals.py \
  target/skill-evals/custom-vercodable-append-workspace \
  target/skill-evals/custom-vercodable-frame-v3-smoke
```

The script refuses to overwrite its destination.
It wraps copied raw bodies, grades the copies, and compiles the benchmark without measurement.
It also requires behavior checks to reject `buf.len()`, decoded-size, and known-size consumption shortcuts.
The result aggregator rejects smoke grades as model evidence.
