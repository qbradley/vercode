# Efficient implementations

## The target

Minimize work beyond the necessary reads, writes, validation, and allocation.
Whole-graph knowledge makes field order, fixed extents, and relevant branches available at generation time.
Avoid replacing that knowledge with a runtime schema interpreter or intermediate value tree.

Write, read, and size should expose the same pass structure for each layout.
This makes future updates auditable; it does not require distinct variants to share a branch-heavy emitter.
Keep direct code when an abstraction only hides straightforward repetition.
Owned decoding remains owned: `read_version` has no input lifetime for a borrowed result.

Useful opportunities depend on the layout:

- Fixed runs can share a bounds check across former struct boundaries.
- Fixed-width collections and size calculations can use checked arithmetic instead of repeated dispatch or traversal.
- Direct writes can backpatch actual lengths without a sizing preflight or a complete staging buffer.
- Distinct variant layouts can specialize constant offsets instead of carrying artificial options and mode flags.
- Adjacent frozen headers and payloads can use wider packed stores without deleting any historical bytes.
- Owned collections can receive later fields in place rather than through a second object graph.

Preserve variable-region bounds, pass boundaries, primitive validity, and historical error behavior.
Larger specialized code or extra packing instructions can cost more than they save.
Use measurements to select among these opportunities.

## Typed fixed-offset fields

Prefer named `WireField<integer, OFFSET>` constants shared by read and write when the resolved crate provides them.
Offsets are relative to the supplied run slice; the enclosing codec still owns its extent, framing, and domain meaning.
Keep variable fields and later passes in their own wire positions.
Packed historical pairs can remain packed.

`WirePrimitive` is sealed to unrestricted integers.
Bool, char, nonzero values, options, enums, and domain wrappers still need their protocol's conversions and validation.
`usize` and `isize` use target width; portable protocols need explicit widths.
Checked fields handle short ranges and offset overflow without panics or partial field writes.
Shared offsets prevent read/write drift, not a consistently wrong offset: independent fixtures remain necessary.

Plugin version does not prove API availability.
For an older dependency without the helper, keep explicit fixed offsets rather than copying a helper framework or silently upgrading.
Inspect the installed interface instead of assuming an unreleased checkout is available from the registry.

## Bulk operations and zerocopy

Bulk access needs a wire-layout proof: extent, byte order, padding, alignment, and permitted bit patterns.
`Copy`, `repr(C)`, and `size_of` alone provide no such proof.
Fields from different passes cannot become one memory copy.

Use the installed zerocopy API's actual guarantees.
For example, `FromBytes` accepts every bit pattern, whereas `TryFromBytes` requires validation.
Padding-free output and unaligned input are separate requirements.
Safe byte copies or endian-aware wire types often express these constraints directly.

## Evidence, not instruction counts

Compare equal values, versions, ownership costs, and framing.
Fresh nested formats must be smaller than macro encodings.
Unchanged adopted formats retain their original size.
For helper cost, compare the checked public interface from a separate consumer crate with equivalent manual fixed offsets.
An infallible private prototype or an old cursor reader answers a different question.

Declare performance acceptance before measurement.
Measure release-mode write, owned-read, and size separately on representative nontrivial graphs.
Paired, order-balanced samples, reusable buffers, and sufficient iterations reduce setup and timer bias.
Keep raw samples and the exact binary/source, compiler, flags, target, and workload identity.
Preserve failed trials; changed code is a new trial.

Use profiles to attribute costs and matching assembly to explain them.
Without a profile, attribution remains unverified even when paired timings compare candidates.
Fewer stores, possible vectorization, or a faster reader do not prove a faster writer.
A neutral-cost helper refactor can meet a non-regression goal without meeting a separate speedup goal.
Report these distinctions and shared-host uncertainty honestly.
