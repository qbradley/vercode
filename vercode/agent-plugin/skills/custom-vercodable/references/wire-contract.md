# Global-pass semantics

## Shape and introduction

The base and each extension have a frozen wire-path order, independent of Rust declaration order.
For a root with two rows, adding a score to each row means:

```text
base:   row_count, row0.x, row0.y, row1.x, row1.y, tail
pass 1: row0.score, row1.score
```

Interleaving scores with old row fields breaks the old prefix.
Repeating shape wastes bytes and can make later passes disagree about which values they describe.
Maps therefore need canonical wire order or retained base-entry order; rebuilding a `HashMap` does not preserve that order.

A descendant cannot appear before the subtree or variant that contains it.
Its effective birth is `max(all ancestor births, variant birth, field birth)`.
For `Option<Vec<Point>>` introduced at pass 4, Point fields from passes 0 and 2 appear with its shape at pass 4.
A Point field from pass 6 still appears at pass 6, without another count or presence code.

Version gaps are valid.
The requested version selects the applicable passes; `MAX_VERSION` reflects the complete graph.
A delegated child's bytes and evolution contract must be frozen, or its independent evolution can break the root's prefix.

## Absence is not truncation

The base is required.
A complete historical endpoint defaults later additions.
Once a nonempty known pass starts, all its applicable fields and elements are required.
Element-local EOF would silently accept a partially transmitted pass.

An empty collection or inactive branch can make a pass zero bytes long.
That says nothing about whether a later pass contains data.
Zero-byte fields cannot encode presence, so their absent and present meanings must agree.
A side-effecting default cannot recover an event that the wire never represented.
Preserve historical default behavior, including when effects occur, or report the incompatibility.

A cut shorter than the declared frame always fails.
A shorter, correctly reframed historical payload is indistinguishable from intentionally old data.
Unknown suffix bytes inside the frame are skippable; bytes outside it belong to the caller.

## Enum boundaries

Freeze wire tags independently of source order.
For a fresh format, a new variant's tag occupies the established shape position, but its entire payload starts at its introduction pass.
Then an unknown future tag has **no payload in any known pass**, so an approved default can preserve following fields.
Keep unknown-tag provenance: constructing the fallback must not make its own payload appear on the wire.

This guarantee does not follow from Rust `Default` or from the root length.
An unknown legacy variant can have an arbitrary inline base payload whose end is unknowable.
Reject it unless an independent boundary permits skipping it.
Payload-free fixed-width code enums can default an unknown complete code, but missing bytes still fail.

Fresh writers reject variants unavailable at the requested version unless an agreed older representation exists.
Readers apply the unsupported-variant policy to known-but-newer tags.
Deferred payload fields need defaults for absent passes; known base payloads and started passes remain required.
Adopted formats retain their historical tag and version policies.

## Framing alternatives and limits

The default total-length frame makes its end available before schema interpretation.
Its length must fit its wire width and the input, including the header.
An independently composed `Vec<Custom>` needs a boundary for each element.
One outer length cannot delimit otherwise unframed elements; one global codec for the entire vector can.

The after-base fallback is:

```text
frozen base | u32_le extension_byte_length | all extension passes
```

Its consumed extent is `base_bytes + 4 + extension_byte_length`.
The base schema is frozen, but its per-value size can vary.
The length word is mandatory even at zero; [adoption.md](adoption.md) covers the bootstrap consequence.

Other framing is valid if a future-unaware reader can find exactly one complete value.
Sentinels need unambiguous termination and escaping; chunks need a known termination grammar and skippable unknown types.
Size and consumption include that framing.
A version number alone gives no unknown-payload boundary.
External-only bounds belong in a separate raw-body interface, not a generally composable `VerCodable` implementation.

Wire widths, byte order, tags, option niches, and primitive validity belong to the protocol, not Rust's memory layout.
Use checked extents and bounded allocation for hostile counts.
Fixed-width payloads permit extent checks before allocation; variable-width collections need bounded initial reservation.
`Vec<()>` represents its count without payload or an element loop.
Resource bounds also apply to recursion and expansion.
