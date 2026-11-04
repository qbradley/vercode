# Adopt an existing base encoding

An existing VerCode, bincode, wincode, or other codec can define the complete base pass.
All future additions follow that entire base.
The framing contract must also expose their full extent to older readers.

## Recover the exact format

Record the original codec release, feature flags, serializer configuration,
integer encoding, byte order, count widths, enum tags, and field order.
Record custom Serde implementations or other adapters that affect bytes.
Names such as "bincode" alone do not identify a wire format.

Freeze independently generated historical bytes and an old reader.
Include nested structures, multi-element containers, options, and enums.
Record the decoder's trailing-byte behavior and returned consumption.
Acceptance of trailing bytes is not proof of composable consumption.
A legacy decoder can return its old prefix length after accepting newer bytes.
That decoder leaves extensions in place of the following sibling.
An old API that rejects trailing bytes cannot accept appended framing or passes unchanged.

For bincode 1, helper functions and `DefaultOptions` can have different defaults.
Use the actual application configuration, not a guessed equivalent.
For wincode, inspect the actual schema/configuration and custom implementations.

## Choose the envelope

### Reuse a macro-derived root frame

A `Vercode`-derived struct or enum already has a leading `u32` total-length prefix.
Reuse that slot as the complete frame length.
Expand its value to cover the appended global passes:

```text
existing root total length | historical fields, including nested headers | global extension passes
```

Add neither another leading length nor an after-base extension-length word.
Preserve all historical field bytes and nested headers.
New nested fields belong in the global suffix, not inside historical child frames.
Existing macro readers skip their unknown suffix and return the expanded root length.
They need no new framing adapter for this layout.
Without extensions, the complete encoding remains byte-identical to the historical writer.

This changes the original root length word, not the historical field encoding.
Document that framing metadata is outside the data-prefix invariant.
The new decoder must track the actual end of the historical fields.
The historical root decoder's returned consumption includes the extension suffix, so it cannot locate the start of those extensions.

`VercodeTransparent` and fixed-width integer enum derives do not necessarily provide a root length.
Inspect their actual encoding before selecting this path.

If an external requirement freezes the original root length word too, retain the entire historical encoding as the base.
That exception needs a separate envelope, such as the after-base fallback.
An unchanged old macro reader then returns only the historical length and needs a framing adapter for full consumption.

This migration does not remove existing frame overhead.
To remove it, define a distinct fresh format with an explicit transport/schema choice.
Do not silently reinterpret existing stored bytes.

### Add framing to an unframed base

Prefer one leading total length when callers can adopt that envelope.
An unchanged legacy decoder then needs an unwrapping caller or explicit format selection.
Preserve the original field bytes inside the envelope.

When an unframed base such as bincode must stay at byte zero, use the after-base fallback:

```text
legacy base | extension_byte_length | all extension passes
```

Use a frozen integer encoding for the extension length.
The default is `u32` little-endian, excluding the length word itself.
Write the word even with no extensions, using zero.
Locate it by decoding the exact frozen base.
Return `base_bytes + length_word_bytes + extension_byte_length`.
Bound known-pass reads to that extension region and skip its unknown remainder.

An unchanging base format does not imply a constant encoded size.
Counts, strings, or variants can determine its per-value extent.
Freeze the schema and decoder configuration that locate the extension-length slot.
Do not locate that slot by sizing a reconstructed or defaulted value.

#### Bootstrap limitation

Every composable historical revision must already understand this envelope.
Unframed legacy bytes have no zero-extension word.
Appending that word establishes a new framing contract, even though the base remains unchanged.
Use an explicit migration, format discriminator, or separately selected legacy decoder.
Do not guess the format from EOF or a plausible length.

An unchanged bincode reader can still recover its old projection when its trailing policy permits it.
It does not thereby return the full future frame length.
Distinguish semantic compatibility with that reader from composition through the new interface.
Retain original legacy fixtures alongside post-bootstrap framed fixtures.

## Preserve the data passes

Handwrite the original serialization/deserialization and size operations.
Require exact base-byte equality with the original codec.
Do not assume that VerCode's primitive option or length encodings match it.
Keep all original counts, tags, padding rules, and any existing frame headers.

For an original type:

```text
Order { lines: Vec<Line { price, quantity }>, total }
```

For an unframed bincode base, a later `Line.discount` belongs after the entire original Order encoding:

```text
original Order bytes || extension_byte_length || every line discount in original element order
```

The framed reader reconstructs the original lines, then patches their discounts.
The original vector length is not repeated.
The length covers all extension passes, not only those that the reader understands.

For a previously versioned format, preserve the released requested-version mapping.
Freeze each supported historical writer path and its byte fixtures.
New global passes start after the released graph maximum.
Do not rename the latest old schema to version zero and silently change old API behavior.

## Prove the migration

Compare base writes byte for byte with the actual old codec.
For reused macro root headers:

- Compare historical field bytes and check each total separately.
- Without extensions, require the original complete encoding.
- With extensions, require no additional root framing words.

Read legacy bytes through the declared migration path.
Read new writes with the actual old reader and its original trailing policy.
Compare semantic old projections rather than fields unknown to that reader.
Separately check framed older readers against at least two unknown extension passes.
Require their returned consumption to reach the next field exactly.
Include concatenated values, derived parents, and generic vectors.
For the after-base fallback, include a mandatory zero-extension word.
For both layouts, include malformed or truncated length words.

Keep original fixtures and codec configuration unchanged.
Record any required caller adaptation as a migration requirement, not a hidden optimization.
