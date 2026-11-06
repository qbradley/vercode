# Adoption preserves a protocol, not a library name

Recover the actual codec version, configuration, custom adapters, wire layout, and supported requested-version behavior.
Keep independent historical bytes and an unchanged reader.
For bincode 1, helper functions and `DefaultOptions` can use different integer and trailing-byte settings.
For wincode, the application's schema and configuration are authoritative.

Trailing-byte acceptance is not full-frame consumption.
A decoder can accept a suffix yet return only its known prefix, leaving extensions in place of the next field.
One that rejects trailing bytes cannot accept appended framing unchanged.

## Existing macro root

A `Vercode`-derived struct or enum already supplies the preferred total-length slot:

```text
existing root length | historical fields and nested headers | new global passes
```

Reuse that slot; another leading or after-base length is unnecessary.
New nested fields belong in the global suffix, outside historical child frames.
Only the root length changes to describe the added bytes.
Without additions, the complete encoding remains byte-identical.

Unchanged old macro readers return the expanded root length and need no framing adapter.
For that same reason, their returned consumption cannot locate the start of the new passes.
The new decoder needs the historical-field endpoint separately from the whole-frame endpoint.

Transparent wrappers and fixed-width integer enum derives may have no root length.
Preserve their option hooks (`write_option`, `read_option`, `size_option_version`), niches, and malformed-input behavior, not only standalone bytes.

If even the old root length word must remain unchanged, a separate envelope is necessary.
The old reader then needs an adapter to consume that envelope.
Removing historical nested headers is a different format, not a byte-preserving optimization.

## Unframed legacy base

A leading total length is simplest when callers can adopt an envelope.
If the base must remain at byte zero, use the [after-base extension length](wire-contract.md#framing-alternatives-and-limits).
The unchanged base decoder locates this word; sizing a reconstructed projection is not a substitute.

Every composable revision must understand the envelope, including its zero-extension form.
Bare pre-bootstrap bytes therefore need explicit migration or a separately selected legacy decoder.
Neither EOF nor a plausible length can distinguish them from framed data followed by siblings.
Keep original legacy fixtures separate from post-bootstrap fixtures.

## Migration evidence

Preserve released wire slots, counts, tags, defaults, and version mappings.
New global passes start beyond the released graph's maximum.
Renaming the latest old schema to version zero changes the API contract.

Compare bytes with the actual old writer and semantics with the actual old reader under its original trailing policy.
For macro adoption, distinguish mutable root framing from frozen field bytes and retain every historical header.
Separately demonstrate exact future-inclusive consumption with concatenated values, derived parents, and generic collections.
Any required caller adaptation is a migration requirement, not an optimization detail.
