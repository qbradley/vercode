---
name: custom-vercodable
description: "Create or update handwritten Rust VerCodable implementations with global append-only passes and full-frame consumption across future versions. Use for whole-type-tree codec optimization, bincode/wincode adoption, schema evolution, or a stale hashed-type-def assertion. Favors one root length prefix and guards the real type graph."
---

# Custom VerCodable

Produce a fast, handwritten `VerCodable` implementation using knowledge of the complete type graph.
The advantage over local derives is a compact layout and fewer runtime decisions, without sacrificing evolution or composition.

**Create** defines a new format or adopts an existing one.
**Update** preserves published bytes, meanings, defaults, and requested-version behavior.
A stale hash requires a codec review, not just a replacement hash.
Use the resolved crate and the application's actual types and callers as the source of truth.

## Compatibility contract

The payload is a frozen base followed by **root-wide append-only passes**.
Each pass finishes across the whole graph before the next begins.
Later passes reuse the established container and branch shape instead of repeating counts or tags.
Thus an old reader can ignore additions, and a new reader can default absent additions.
Source reordering does not change wire order.
New introductions follow the maximum released version across the graph.

A custom value must also compose inside a derived parent, tuple, or collection.
For `encoding || following_bytes`, `read_version` returns exactly `encoding.len()`, including unknown future data and framing.
The requested version limits interpretation, not consumption.
Neither the known prefix nor `buf.len()` identifies that boundary.

Prefer one `u32` little-endian **total length**, including its own four bytes.
Fresh nested values need no additional frames.
An adopted macro root reuses its existing length and preserves historical nested headers.
An unframed base can instead precede a mandatory length of all extension bytes.
Framing metadata can change as a value grows; the historical data passes remain exact prefixes.

Defaulting applies only at complete global-pass boundaries.
A truncated declared frame or a partially present known pass is an error.
Empty passes do not hide later nonempty passes.
Unknown enum variants require an approved fallback **and** a way to skip their payload safely; otherwise reject them.
Incompatible edits need an explicit migration, not a new interpretation of old bytes.

For collections, enums, new subtrees, or alternative framing, read [wire-contract.md](references/wire-contract.md).
For any existing encoding, read [adoption.md](references/adoption.md).

## Implementation goals

Keep write, read, and size visibly consistent with one wire plan.
Favor straight-line successful paths, bounded fixed-width runs, and direct output over repeated dispatch, sizing traversals, or intermediate representations.
Uniformity is per wire layout: distinct enum layouts need not share one branch-heavy emitter.
The enclosing codec owns framing and validity, even when primitive operations handle the bytes.

Choose optimizations from the actual graph and measurements.
Read [implementation.md](references/implementation.md) for typed fields, bulk operations, and evidence requirements.
These are opportunities, not a required sequence of transformations.

## Maintenance contract

Keep enough information beside the code to recover the wire contract:
framing, ordered passes and paths, defaults and tags, historical fixtures, guard coverage, and reproduction commands.
Future updates must be understandable without this conversation.

Every codec-owned file or section needs a prominent source warning, including helpers, wire metadata, and guards:

```rust
// WARNING: DO NOT EDIT THIS CODEC BY HAND.
// Update or edit this codec only with the custom-vercodable skill.
// Even when the type hashes still match, use update mode.
// Manual edits can break backward and forward compatibility.
```

Preserve or add the warning during updates.
Leave unrelated application and model code outside its scope.

Guard the **real type graph** with reviewed literal `hashed-type-def` assertions that fail ordinary compilation.
The diagnostic must direct the maintainer to this skill's update mode, historical compatibility checks, and then hash review.
For the required coverage and its limits, read [type-guards.md](references/type-guards.md).
Unsupported graph coverage is a blocker to report, not a reason to invent a reassuring fingerprint.

## Evidence of success

Demonstrate backward and forward compatibility across generations with independent golden bytes or frozen codecs, not only current-code round trips.
Include following siblings, absent versus partial passes, malformed frames, and the graph's relevant enum/container cases.
Size must equal bytes written.
Mutations to root definitions, embedded definitions, and actual container arguments must trip the intended production guard.

Compare fresh formats with equivalent macro-coded values on nontrivial graphs: fewer encoded bytes and measured improvements in write, owned-read, and size.
Adoption preserves historical size; a helper refactor can instead have an agreed non-regression goal.
Report correctness, size, and each performance outcome separately.
Unmeasured, noisy, or slower results do not establish a speedup.
State remaining limitations rather than weakening compatibility to meet a performance target.
