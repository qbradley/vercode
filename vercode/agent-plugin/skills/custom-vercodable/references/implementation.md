# Fast, matching global passes

## Whole-graph specialization

Generate explicit passes from the frozen wire plan.
Do not materialize a runtime plan, action enum, or intermediate value tree.
Skip branches that have no fields in the current pass.
Repeated traversal is necessary only where that pass contributes data.

The methods have this shape:

```text
write: reserve length -> base -> pass 1 -> pass 2 -> backpatch actual frame length
read:  bound frame -> base and defaults -> known present passes -> return full frame length
size:  framing bytes + base size + pass 1 size + pass 2 size
```

This outline uses the preferred leading length.
For macro-based adoption, reserve the existing root-length slot and count it once in the size.
For the after-base fallback, put the reserved extension-length slot after the frozen base.
Its value excludes the base and length word.
Write, read, and size still return the complete base, header, and extension extent.
For other framing, preserve its frozen grammar around the same data passes.

For each wire layout, keep the same visible pass order, field names, constants, and conditional branches across methods.
Group a fixed-width run into one bounds check.
Without nested struct prefixes, that run can cross multiple nested struct boundaries.
Split it at variable data, conditional payloads, and global pass boundaries.
Restricted primitive values still need validity checks.

A fixed-width base vector can use one checked `count * element_width` extent.
Decode each element from its corresponding chunk.
A later fixed-width extension can use another whole-pass extent.
Avoid repeated size calls, per-field generic cursor methods, and nested codec dispatch.
Keep explicit duplication when a helper only hides straightforward repetition.

The size method can often use counts rather than element traversal.
For example, a later `u8` per existing row contributes exactly `rows.len()`.
Keep that contribution in its pass position rather than gathering all versions
into an unrelated size formula. Uniform code is easier to update correctly.

Writers do not call size methods or stage the complete output.
Backpatch the length from the actual cursor after all requested passes succeed.
Check representability before storing the length.
Readers retain the frame end separately from the known-pass cursor.
Return the frame extent even when the requested version stops interpretation earlier.
Do not use `buf.len()` or the decoded value's size as a substitute.
Readers decode owned collections directly and patch later fields in place.
Initialize fields with inexpensive pure defaults where that preserves semantics.
Avoid cloning the graph or allocating a list of per-field actions.
Map replay can require traversal state. Explain and measure that specific cost.

An adopted opaque codec is safe to delegate only if its byte consumption is known.
It must leave the following base fields and extension passes accessible.
An independently evolving child codec can invalidate the frozen prefix.
Freeze its schema/configuration or include its evolution in the global plan.

## Typed fixed-offset fields

This checkout provides `vercode::WireField` and `WirePrimitive` in version 0.8.0, which is not yet released.
Inspect the resolved dependency before using these symbols.
For older crates without this interface, retain explicit fixed-offset code instead of changing dependencies.
After publication, record the actual minimum released version rather than assuming this checkout is available from the registry.

1. Freeze the historical wire layout before selecting helpers.
2. Declare named `WireField<integer, offset>` constants once per fixed run.
   Group the run length and related fields together.
   Offsets are relative to that run's slice, not necessarily the root frame.
3. Use those same declarations in the read and write schedules.
   The crate owns little-endian conversion and primitive width.
   Keep domain conversion, boolean validation, and option/niche handling at the call sites.
   `usize` and `isize` retain target width. Portable protocols need explicit-width integers.
4. Bound the complete run before accessing its fields.
   Call the named fields directly, without a runtime descriptor list or interpreter.
   Keep variable regions, conditional shapes, and later passes in their explicit schedules.
5. Preserve proven specializations, including packed historical header/value pairs.
   A packed `u64` field can retain both words without treating a domain wrapper as an unrestricted integer.
6. Keep root framing, backpatching, nested historical headers, and full future-inclusive consumption in the enclosing codec.
   A raw field helper is not a composable `VerCodable` implementation.
7. Repeat compatibility, malformed-input, offset-overflow, structural guard, and performance checks after refactoring.
   Shared declarations do not detect wrong offsets or replace independent historical fixtures and reviewed guard literals.

This example handles one fixed run, not a complete frame:

```rust
use vercode::{InvalidEncoding, WireField};

const RUN_SIZE: usize = 24;
const OFFSET: WireField<u64, 0> = WireField::new();
const FILE_ID: WireField<u128, 8> = WireField::new();

fn write_run(buf: &mut [u8], offset: u64, file_id: u128) -> Result<(), InvalidEncoding> {
    let run = buf.get_mut(..RUN_SIZE).ok_or(InvalidEncoding)?;
    OFFSET.write(run, offset)?;
    FILE_ID.write(run, file_id)
}

fn read_run(buf: &[u8]) -> Result<(u64, u128), InvalidEncoding> {
    let run = buf.get(..RUN_SIZE).ok_or(InvalidEncoding)?;
    Ok((OFFSET.read(run)?, FILE_ID.read(run)?))
}
```

The checked field interface rejects truncated ranges and offset overflow without panicking.
A failed field write leaves the buffer unchanged.
`WirePrimitive::read_le` and `write_le` operate on the first `WIDTH` bytes and leave the remainder untouched.
The sealed trait supports only unrestricted integers, not arbitrary `VerCodable` types.

A helper refactor can meet a declared non-regression gate without establishing a speedup.
Preserve any separate codec improvement goal.
Measure the checked public helper from a separate consumer crate against manual fixed offsets with identical ownership and schedules.
Private or infallible prototypes do not establish that interface's cost.
Apply the [size and speed evidence checklist](#size-and-speed-evaluation) to this isolated comparison, including assembly and raw paired timings.
Keep primitive implementations in the crate rather than copying them into generated codecs.

## Specialize distinct write layouts

Uniformity means matching write/read/size schedules for each layout, not a shared emitter for every enum variant.
Artificial `Option` values, mode flags, and variable cursors can retain branches and indexed addressing after compilation.
If those costs remain in a hot path, compare separate fixed-layout emitters.
Dispatch on the variant once.
Bound each selected fixed base or fixed run once, then write at constant offsets within that slice.
Bound variable tails separately without a subtree sizing preflight.
Preserve short-buffer errors, overflow handling, and every requested-version layout.

Shared fixed blocks remain useful when inlining removes their extra control flow.
Measure each variant and a representative mixture before retaining duplicated paths.
Larger code can increase instruction-cache costs.

## Pack historical framed runs

Preserving bytes does not require preserving the original sequence of stores.
Adjacent constant headers and fixed-width payloads can form wider stores within one pass.
Keep each header's exact width, value, and byte order.

For a frozen `[length: u32 LE = 8, checksum: u32 LE]` frame, an eight-byte `chunk` can receive:

```rust
let word = 8_u64 | (u64::from(checksum) << 32);
chunk.copy_from_slice(&word.to_le_bytes());
```

Here, `checksum` is a `u32`, and `chunk` comes from an already bounded output run.
The low four bytes remain the historical length, including its own word.
The high four bytes remain the checksum.
Repeat over the original element order without removing headers or crossing pass boundaries.
Emit directly into the caller's buffer without a staging array or an alignment-dependent pointer cast.

Fewer scalar stores do not prove a speedup.
Packing adds shifts and combines, and the compiler can already combine separate stores.
Vectorization is a possibility, not a guarantee.
Use [Size and speed evaluation](#size-and-speed-evaluation) to decide whether to retain this candidate.

## Zerocopy and primitive bulk operations

First prove that the copied run matches the pass's bytes:

| Opportunity | Required proof |
|---|---|
| Byte arrays and byte payloads | Exact extent and valid bounds |
| Integer runs | Defined byte order on every supported target |
| Integer vectors | Checked count multiplication and payload extent |
| Zerocopy wire struct | Fixed layout, no padding, alignment, byte order, validity |
| Restricted primitives | Explicit validation or checked conversion |

`Copy`, `repr(C)`, and `size_of` alone do not prove wire compatibility.
Rust padding can expose uninitialized bytes.
Native integers can change byte order. Rust enum layout is not a tag table.
A type's newly added fields can occupy different passes and cannot join a base memory copy.

For zerocopy 0.8, inspect the installed API:

- `IntoBytes` permits byte output without padding.
- `FromBytes` permits every bit pattern. `TryFromBytes` needs validity checks.
- `KnownLayout`, `Immutable`, and `Unaligned` describe separate requirements.
- Byte-order-aware wrappers can express endian-independent wire integers.

Use safe APIs where possible.
Use unaligned reads or copies when input alignment is not guaranteed.
Inlined struct runs need no separate frame around their zerocopy bytes.
Keep the enclosing custom frame and any adopted historical headers.

`read_version` returns owned `Self` without an input lifetime.
Bulk decoding of owned strings/vectors is not borrowed zero-copy decoding.

## Size and speed evaluation

Compare equal semantic values, requested versions, and ownership costs.
Fresh global-pass codecs intentionally differ from macro encodings.
Check each decoder's result against the same value before measurement.
For adoption, separately require exact historical field bytes and correct framing metadata.
For an unchanged adopted schema, equal encoded size is expected.

Use nontrivial nested graphs and collections with many structured elements.
Report serialized bytes for each format, including counts, tags, and complete codec framing.
Include the custom or reused root length, or the after-base extension-length word.
Report external framing separately, or include identical framing in both totals.
Require a smaller fresh encoding than the macro format.
Do not remove historical bytes merely to pass this size check.

Measure release-mode write, owned-read, and size separately.
Keep setup outside timing. Reuse equal-purpose write buffers.
Use optimizer barriers and alternate measurement order within paired samples.
Choose iteration counts per method, not only per collection size.
Constant-time sizing needs enough calls to make timer overhead negligible.
Include a base-version workload and a latest-version workload.
Report sample counts, distributions, compiler, target, and workload sizes.

Declare acceptance before reading timings.
For example, require lower custom medians and at least seven wins in nine paired samples.
Apply this to each method and declared representative current workload.
Keep encoded-size and performance outcomes separate from correctness scores.
A noisy or slower method fails the declared performance goal.
It does not justify relaxing compatibility or hiding that workload.
A read-speed gain does not satisfy the write-speed goal.

Use failed measurements to choose the next optimization.
Record the benchmark binary, source revision, target, and compiler flags before inspecting generated code.
Profile that binary before attributing costs to its assembly.
If profiling is unavailable, label those costs unverified and use paired timings to evaluate candidates.
Inspect optimized assembly for remaining branches, indexed addressing, bounds checks, and scalar stores.
Measure layout specialization and packed emission separately, then together, against the same baseline.
Require historical-byte equality and short-buffer behavior before timing each candidate.
Instruction counts and source-level operation counts alone do not establish a speedup.
Check repeated traversals, bounds checks, allocations, and missing inlining.
Run a new candidate or explicitly labeled optimization trial after a code change.
Do not repair a scored submission and retain its old identity.

Shared-host timings do not establish universal speedups.
Record unavailable allocation and code-size measurements honestly.
The format removes per-node evolution overhead, not all data or runtime costs.
