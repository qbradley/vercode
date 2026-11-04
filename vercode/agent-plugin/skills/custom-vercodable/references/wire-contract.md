# Root-wide append-only wire contract

## Full-frame consumption

The deserializer is `VerCodable::read_version`, which returns `(value, consumed)`.
The convenience `deserialize` function discards that count.
The count must cover exactly one complete encoded value, including unknown future bytes.
The input can contain following fields or values.

For `input = encoding || following_bytes`, every successful reader returns `encoding.len()`.
This includes framing bytes and all passes that the writer actually serialized.
The requested version limits field interpretation, not frame consumption.
`write_version` and `size_version` include the same framing bytes for their requested version.
Sizing a decoded old projection cannot recover the actual future encoding length.

The format must expose its end to readers that do not know future schemas.
Returning the known prefix leaves extensions in place of the next field.
Returning `buf.len()` consumes that field.
A version number alone cannot locate arbitrary unknown payloads.

## Preferred frame: one leading total length

Use this default layout:

```text
u32_le total_length | frozen base | pass 1 | pass 2 | ...
```

`total_length` includes its own four bytes and every serialized pass.
This convention matches macro-derived VerCode frames.
For macro-based adoption, reuse the existing root length slot instead of adding framing.
Read [adoption.md](adoption.md#reuse-a-macro-derived-root-frame) for the rules that preserve historical field bytes and nested headers.
Freeze another width only when the format requires it.
Check the wire-width limit and arithmetic overflow.

Read the length before the payload.
Require `4 <= total_length <= buf.len()`.
Decode the payload from `buf[4..total_length]`.
Return `total_length`, even after decoding only older passes.
Skip unknown suffixes inside that slice, not outside it.

The writer reserves four bytes, writes the passes, and backpatches the actual total.
The size method includes those four bytes.
One custom frame can contain many nested structs without their own frame headers.
An independently composed custom child has its own frame.

This layout supports derived parents, tuples, options, and generic collections.
`Vec<Custom>` needs a frame for each element.
A custom global codec for the entire collection can use one frame instead.
Putting an outer length around a generic vector of unframed roots does not delimit its elements.

## After-base fallback: frozen base, then extension length

When an unframed base such as bincode must stay at byte zero, prefer:

```text
frozen base | u32_le extension_length | extension passes
```

The frozen base decoder must locate its own end from its unchanged schema.
Its encoded size can vary by value.
The extension length counts all extension bytes, excluding the length word.
It is mandatory even when its value is zero.
The total consumption is `base_bytes + 4 + extension_length`.
Check that this sum fits the input and `usize`.

Readers decode their known passes inside the declared extension region.
They skip the rest and return the complete total.
Every revision claiming this contract must understand the extension-length slot.
Pre-bootstrap bytes without that slot need explicit migration or a separate legacy decoder.
EOF or following bytes cannot reliably identify an omitted length word.

This fallback is not necessary for a macro root with a reusable total-length prefix.
Read [adoption.md](adoption.md) for historical reader policies and explicit framing exceptions.

## Other framing schemes

Other schemes are acceptable when old readers can find and consume the full future encoding.
Document the framing grammar independently of the schema.
Prove concatenation, future-version skipping, and malformed-frame behavior.

A sentinel scheme needs collision-free termination.
Escape payload occurrences or use a structured grammar that distinguishes data from delimiters.
Include the terminator and escaped bytes in size and consumption.
A missing terminator is an error, not an implicit end of value.
Length-delimited chunks need a known termination rule and skippable unknown chunk types.
These schemes can cost more branches and bytes than a single length.

An external transport frame is useful but does not repair ordinary `VerCodable` composition.
Expose raw externally bounded encoding through a separate helper or interface.
The generic trait implementation must retain its own full-frame consumption contract.

## Base and extension passes

Let `B(x)` be the frozen base encoding and `Pj(x)` extension pass `j`.
For a reused historical root header, treat that word as framing, not as part of `B(x)`.
The data passes for requested version `k` are:

```text
D0(x) = B(x)
D1(x) = B(x) || P1(x)
D2(x) = B(x) || P1(x) || P2(x)
```

For values representable by the older schema, earlier data passes are exact prefixes of later data passes.
Length words change to describe the actual frame.
Whole framed encodings therefore need not be literal prefixes.
Old readers decode their known passes and discard the remaining frame payload.
New readers decode the old prefix and default absent later fields.
This guarantee does not make arbitrary type changes or unknown variants compatible.

Fresh base encodings need no nested struct lengths or enum payload lengths.
Primitive values use their specified bytes.
Vectors and strings retain counts. Options retain presence or niche codes.
Enums retain tags. These bytes describe values, not schema versions.
An adopted base retains its published field encodings.
Any reuse of an existing root length needs the migration rules in [adoption.md](adoption.md).

Consider this tree:

```text
Root { rows: Vec<Row>, tail: u32 }
Row  { x: u16, y: u16, score: u8 added in pass 1 }
Root.note: String added in pass 2
```

For two rows, the preferred wire order is:

```text
frame:  total_length
base:   row_count, row0.x, row0.y, row1.x, row1.y, tail
pass 1: row0.score, row1.score
pass 2: note_byte_count, note_utf8
```

There is one length around Root, but none around Row or either extension pass.
Pass 1 does not repeat `row_count`.
In particular, `row0.score` must not appear between the original rows.

## Topology replay

Each pass follows the frozen wire-path order, not current Rust declaration order.
The base establishes existing container counts, option presence, and enum selection.
Later passes revisit the same values through that established shape.
They emit only fields assigned to their pass.

For a subtree introduced in pass `b`, each descendant has effective introduction
`max(b, variant_introduction, descendant_introduction)`. Apply this rule through every ancestor.
Its required fields and shape appear in pass `b`, even if their type definitions
predate that field. Later descendants still belong to later passes.

Example:

```text
pass 1: introduce extra: Vec<Item>
        emit count, then every Item's required key
pass 2: introduce Item.flag
        emit flags for existing extra elements, without a count
```

The reader reconstructs `extra` before pass 2 and revisits its stored element order.
The same rule applies to nested vectors, arrays, tuples, options, and known variants.
A pass can create shape that a subsequent pass revisits.
Value mutation between serialization passes is outside the contract.

Map traversal needs an explicit order.
Independent `HashMap` iteration orders can attach later values to the wrong keys.
Use a frozen canonical order, or retain the base wire order as decode traversal state.
Preserve that order across every pass.
Changes to key equality, sorting, or comparator behavior need a compatibility review.
Recursive values also need depth/resource bounds and a supported hash strategy.

## Absence, empty passes, and malformed bytes

Decode the complete base or fail.
Initialize absent additions from their documented defaults.
Prefer pure defaults. Preserve observable default behavior when adopting history.
Delay effectful defaults until absence is established.

Within the declared frame payload, at a global pass boundary:

1. If no bytes remain, return the value with defaults for all later additions.
2. Otherwise, decode the complete known pass.
3. If any required bytes are missing, return `InvalidEncoding`.
4. Continue to the next pass, even if this pass consumed zero bytes.

The EOF decision belongs to the whole pass, not each element or field.
One missing score at the end of a started multi-element pass is an error.
A decoder must not default the remaining elements.

An empty collection or inactive branch can make a pass consume no bytes.
A later pass can still contain data.
Do not stop after a zero-byte pass or require one byte per pass.
Use the finite known pass schedule, not a loop that requires cursor progress.

Zero-byte values cannot convey whether their pass was present.
Permit that representation only when absence and presence are semantically
equivalent for those fields, or preserve a separately agreed historical policy.
Do not claim to recover an unencoded event or side effect.

A cut shorter than the declared frame is an error.
A shorter, correctly reframed payload can end at a complete historical endpoint.
Without a schema declaration, that payload is indistinguishable from an intentional old encoding.
Cuts inside a started pass remain errors.
Unknown bytes beyond the reader's last supported pass are intentionally ignored.
Bytes beyond the declared frame belong to the caller.

## Enum evolution

Freeze explicit wire tags independently of source order.
Variants present in the base schema emit their required payload in the base traversal.
Their added fields belong to later root-wide passes.
New variants emit their tag in the established enum shape pass.
Their entire payload belongs to their introduction pass or later.
Existing tags and option presence are not repeated.

For example, a variant introduced in pass 2 has this schedule:

```text
base:   new_tag, then the root's remaining original fields
pass 1: the root's existing pass-1 fields; nothing for this variant
pass 2: new_variant_payload, at its frozen position among pass-2 fields
```

By this protocol, an unknown future tag has no payload in any known pass.
An approved fallback can therefore preserve following fields without a nested enum size prefix.
Keep enough decode provenance to skip that unknown branch in every known pass.
Constructing the fallback must not make its own payload appear present.
The unit default variant is often the simplest choice.
Without an approved default, reject unknown tags.

This reasoning depends on the global introduction rule, not on `Default` alone.
The root frame permits skipping the root, not an arbitrary unframed nested enum.
An unknown legacy variant can have an arbitrary payload in the historical base.
Without a boundary or the no-known-pass-payload guarantee, its end is unknowable.
Reject that case rather than guessing where the next field begins.

A declared payload-free fixed-width code enum is different.
All possible codes have the same known width and no payload in any pass.
An approved default can handle an unknown complete code without losing alignment.
Empty input still fails.
An adopted, independently bounded historical enum can retain its original fallback.

Define new-variant behavior explicitly.
For a fresh format, reject writing a variant unavailable at the requested version
unless an agreed old representation exists.
For reading, a known-but-newer tag follows the unsupported-variant policy.
Deferred payload fields need explicit defaults at an absent global pass.
For example, an absent `[u8; 3]` extension can default to `[0; 3]`.
Adopted formats retain their historical tag and requested-version behavior.
Known base payloads must be complete.
Once an extension pass starts, its known variant payloads must also be complete.

## Primitive, allocation, and size rules

Freeze byte order, integer widths, length widths, and option encodings.
Fresh formats can reuse the installed VerCode primitive encodings:

- Integers use little-endian bytes. `usize` and `isize` use target width.
- `bool` accepts 0/1. `char` requires a Unicode scalar value.
- Nonzero options use zero for `None`.
- `Option<bool>` uses 0/1/2 for false/true/none.
- Ordinary options use a 0/1 presence byte.
- VerCode strings/vectors use `u32` byte/element counts respectively.

These are not universal bincode or wincode settings.
Read [adoption.md](adoption.md) for historical configuration requirements.

Use checked arithmetic and checked slices.
Validate fixed-width payload extent before allocating a vector.
For variable-width elements, bound initial reservation independently of untrusted counts.
Apply resource limits to nesting, expansion, and total allocations.
`Vec<()>` needs its count but no payload or element loop.

Each count must fit its declared wire width.
The preferred `u32` total length limits the complete frame to `u32::MAX` bytes.
Other length fields impose their own limits.
Include framing bytes and escaped bytes in all size calculations.
A failed write can leave partial output, as the trait permits.
