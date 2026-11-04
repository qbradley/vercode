---
name: custom-vercodable
description: "Create or update handwritten Rust VerCodable implementations with global append-only passes and full-frame consumption across future versions. Use for whole-type-tree codec optimization, bincode/wincode adoption, schema evolution, or a stale hashed-type-def assertion. Favors one root length prefix and guards the real type graph."
---

# Custom VerCodable

Use global knowledge to generate a composable codec, not a serialization framework.
Each custom value owns a frozen base and root-wide extension passes.
Its root can be a field inside another codec, not only a transport message.

`read_version` must return the full encoded length of exactly one value.
That length includes unknown future passes and all framing bytes.
The input can also contain following fields or values.
Determine that length from the encoding, not from the known fields or input-buffer extent.

Prefer one leading total-length prefix for the complete custom value.
For fresh formats, keep the nested global layout unframed.
For macro-based adoption, reuse the existing root length prefix and preserve historical nested headers.
If an unframed base such as bincode must remain at byte zero, put an extension-length prefix after it.
Other framing schemes must locate the complete value without knowledge of future schemas.

## 1. Select the mode and recover the contract

- **Create:** define a base format or adopt historical field encodings.
- **Update:** preserve the existing base and every released extension pass.
  A failing hash guard selects update mode, not a hash-refresh task.

Read [wire-contract.md](references/wire-contract.md) before choosing the layout.
For existing bytes, read [adoption.md](references/adoption.md).
Read [type-guards.md](references/type-guards.md) before installing assertions.
Read [implementation.md](references/implementation.md) before optimizing.

Read the installed trait, primitive codecs, callers, and tests.
Resolve aliases, generic arguments, containers, and every enum payload.
Record the frame grammar, historical bytes, defaults, and enum policy.
Identify which root owns each graph and which traversal order it preserves.

Treat container arguments as separate hash edges. In `hashed-type-def` 0.1.2,
`Vec<T>` and `Option<T>` hashes omit `T`. Prove coverage through actual fields.

**Completion:** every reachable value has a known base encoding, traversal
order, introduction pass, and hash path. Future-unaware readers can locate the complete frame.
Report ambiguous framing or unsupported graph coverage before implementation.

## 2. Freeze the global pass plan

Document this plan beside the implementation:

| Pass | Ordered wire paths | Encoding and shape | Default when absent |
|---|---|---|---|
| Base | Every original field across the root | Fixed fields, counts, tags, payloads | Required |
| Extension 1 | Only fields introduced in this pass | Revisit existing shape | Reviewed defaults |
| Extension 2 | Only fields introduced in this pass | Revisit prior shape | Reviewed defaults |

Use pass 0 for the base when mapping to the trait's version parameter.
The user's first version is this base. The preferred format needs no wire version.
The requested version selects the highest pass to write or understand.
It does not limit the frame length that a reader returns.

Freeze the complete base traversal before adding extensions.
For each extension, finish that pass across the entire root before the next.
For vectors, write all original elements before any element's later fields.
Reuse earlier counts, option presence, and enum tags when revisiting values.

A newly introduced subtree first appears in its ancestor's introduction pass.
Assign each descendant the effective pass `max(ancestor_birth, variant_birth, field_birth)`.
Emit new container shape once at that effective birth.
Record how later passes revisit that exact shape and element order.
Keep an enum's tag in its established shape pass.
Put a new variant's entire payload in its introduction pass or later.

For updates:

- Append one new global pass beyond the released graph's maximum.
- Preserve every released pass, including its traversal order and defaults.
- Preserve old slots, widths, meanings, tags, and container encodings.
- Preserve retired slots through an explicit conversion or replacement value.
- Treat source reordering as a source edit, not permission to reorder bytes.

An incompatible edit needs an agreed migration. Stop before changing old field bytes.
Do not silently remove headers from an adopted historical base.
Freeze length widths, length meanings, and delimiter rules beside the pass plan.
Length values change to describe the actual bytes of each encoding.

**Completion:** earlier data passes remain exact prefixes of later data passes for representable old values.
Framing metadata is separate from this invariant.
The plan explains topology replay, defaults, and complete-frame consumption.

## 3. Implement three matching pass schedules

Put this warning at the top of each codec-owned source file, after its license header.
In shared files, put it immediately before each codec-owned section.
The warning must appear in the source, not only in documentation or a hash diagnostic.

```rust
// WARNING: DO NOT EDIT THIS CODEC BY HAND.
// Update or edit this codec only with the custom-vercodable skill.
// Even when the type hashes still match, use update mode.
// Manual edits can break backward and forward compatibility.
```

Scope the warning to codec methods, helpers, pass metadata, and guards, not unrelated application or model code.
Preserve the warning during updates.
If an existing codec lacks the warning, add it before editing.

Write `write_version`, `read_version`, and `size_version` from the same plan.
Use explicit code, the same pass order, and the same fixed-run constants.
For fixed-width runs, read [Typed fixed-offset fields](references/implementation.md#typed-fixed-offset-fields) and prefer the supported crate helper.
Keep corresponding methods uniform within each wire layout, rather than forcing distinct variants through one shared runtime path.
Avoid a new macro, runtime schema interpreter, or intermediate wire-value tree.

For length framing, the writer reserves the length word and emits the complete base and requested global passes.
It writes directly into the caller's buffer without a sizing preflight.
It backpatches the length from the actual written extent.
For macro-based adoption, backpatch the existing root slot without adding a second length.
Only the after-base fallback places an extension-length slot after the frozen base.
For other framing, emit its frozen delimiter or chunk grammar around the same data passes.

The reader locates its frame using the frozen framing rules.
For the after-base fallback, it decodes the frozen base to locate the extension-length slot.
It bounds extension reads to that frame and prepares defaults for later fields.
At each global pass boundary, an exhausted payload means all later passes are absent.
If bytes remain, decode the complete known pass or return `InvalidEncoding`.
Never treat EOF inside a field, element, or started pass as a default.
Skip zero-byte passes without terminating the pass schedule.
After the requested known passes, discard the unknown suffix.
Return the full frame length, including that suffix and all framing bytes.
Leave the following value untouched.

The size method includes framing and calculates the same passes without serializing.
Collapse fixed-width collections into checked multiplication where possible.
Do not scan a subtree to calculate a size that its counts already determine.
Calculate `MAX_VERSION` from the complete effective pass plan.
Do not calculate read consumption by sizing the decoded value.

Coalesce checked fixed-width runs across former struct boundaries.
For adopted frames, consider packing frozen headers and adjacent payloads into wider stores without changing their bytes.
Use existing primitive bulk operations only when their bytes match this plan.
Delegate a nested codec only if its consumption and evolution contract is frozen.
A raw bounded-body helper is not a generally composable `VerCodable` implementation.

For unknown tags, require an approved default and proof of safe payload skipping.
The protocol must exclude payloads from known passes or expose the enum end.
Fresh variants satisfy the first rule through their introduction passes.
Preserve unknown-tag provenance so later passes do not decode a fallback's payload.
Without that proof or an approved default, return `InvalidEncoding`.
Known base payloads and started extension payloads must be complete.

**Completion:** all three methods agree on each pass's order, bytes, and size.
Known data is bounded. Old readers skip complete future frames without consuming siblings.
Every codec-owned section carries the maintenance warning.

## 4. Install the update tripwire

Derive `hashed_type_def::HashedTypeDef` on the real reachable definitions.
Use unconditional compile-time assertions against reviewed literal hashes.
Bridge nominal container edges through field-inferred argument witnesses.
Use the procedures in [type-guards.md](references/type-guards.md).

Include an actionable diagnostic:

> Type tree changed. Ask an LLM to run custom-vercodable in update mode.
> Preserve the framing contract, frozen base, and global passes. Update all three
> methods, prove historical compatibility, then update the reviewed hash.

Keep these instructions beside the code:

- The frame grammar, length meanings, and full-consumption requirement.
- The frozen base, ordered global passes, tags, defaults, and traversal order.
- New-subtree birth rules, inlined paths, and fast-path assumptions.
- Hash coverage and separate checks for wire metadata.
- Exact commands and immutable historical fixtures.
- Update order: recover history, revise the pass plan, then update all three methods.
  Run compatibility checks before capturing hashes. Repeat guard mutations.

**Completion:** ordinary compilation fails after root, embedded-definition,
and container-argument mutations. The diagnostic gives the update procedure.

## 5. Prove compatibility, size, and speed

Use frozen old implementations or independent golden bytes.
For adoption, include the actual original codec and its exact configuration.
Do not use current-code round trips as the only compatibility evidence.

Cover these cases:

- Exact data-pass prefix equality across at least three generations.
- Correct framing metadata for every generation, including empty extension regions.
- Old writers to new readers with defaults, and new writers to old readers.
- Nested siblings, multiple collection elements, and conditional branches.
- New containers, new subtrees, and descendants added in later passes.
- Zero-byte passes before nonempty passes and gaps in requested versions.
- Each input/output truncation and correctly framed historical endpoints.
- Partial passes, invalid primitive values, UTF-8, and hostile counts.
- Unknown-variant fallback, strict rejection, and deferred new-variant payloads.
- Exact size/write agreement, including all framing bytes.
- Old readers that skip unknown passes and return the actual future frame length.
- Derived parents with following fields, tuples, vectors, and consecutive values.
- Reused macro root lengths without extra headers, preserved nested headers, and unchanged old macro readers.
- After-base adoption with zero extensions, missing length words, and explicit pre-bootstrap migration.
- Production guard mutations, formatting, and relevant lints.

Compare semantic equivalents against macro-based VerCode on nontrivial graphs.
Fresh formats intentionally differ, so compare decoded values rather than bytes.
Record encoded sizes and require strictly fewer bytes for fresh nested formats.
Measure release-mode write, owned-read, and size paths separately.
Require measured speed improvements on the declared representative workloads.
Treat slower or inconclusive results as failed performance goals, not successes.
Tune the implementation or report the unmet goal without hiding regressions.

**Completion:** compatibility and guard checks pass. Size and speed evidence
meet the declared evaluation goals, or the remaining failure is explicit.
Report the mode, format lineage, evidence, and unsupported graph or target.
