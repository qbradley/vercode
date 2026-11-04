# Type guards

## Verified API

The published `hashed-type-def` 0.1.2 API uses:

```toml
hashed-type-def = { version = "=0.1.2", features = ["derive"] }
```

```rust
use hashed_type_def::HashedTypeDef;

#[derive(HashedTypeDef)]
struct Child {
    x: u16,
    y: u16,
}

#[derive(HashedTypeDef)]
struct Root {
    child: Child,
    tail: u32,
}
```

The constant is `<Root as HashedTypeDef>::TYPE_HASH_NATIVE`, not `TYPE_HASH`
or a runtime `TypeId`. Import the trait into the definition's module.
The derive's generated code needs that name in scope.

The derive incorporates each field type's hash. A root hash therefore covers
embedded definitions when their hash implementations cover those definitions.
Check wrappers, containers, generic arguments, and foreign types individually.

## Container edges

In 0.1.2, standard `Vec<T>` and `Option<T>` hashes are nominal.
Their supplied definitions do not use `T` in their hashed fields.
Thus `Vec<u16>` and `Vec<i16>` have equal hashes.
An embedded definition behind these containers can change without altering
the root hash. Other standard wrappers need the same inspection.

Bridge each missing edge with an unconditional guard on the actual argument.
Infer that argument from the real field, so a later field-type change cannot
leave a stale assertion checking an unrelated type.

For a `Batch` with `entries: Vec<Child>`, use this shape:

```rust,ignore
const fn entries_hash<T: HashedTypeDef>(_: fn(&Batch) -> &Vec<T>) -> u128 {
    T::TYPE_HASH_NATIVE
}

const _: () = assert!(
    <Batch as HashedTypeDef>::TYPE_HASH_NATIVE == REVIEWED_BATCH_LITERAL
        && entries_hash(|batch| &batch.entries) == REVIEWED_CHILD_LITERAL,
    "Batch type tree changed. Ask an LLM to run custom-vercodable in update mode; \
     preserve framing, the frozen base, and global passes; run historical tests, then fix this assert."
);
```

The function pointer is a type witness. The const function never calls it.
This pattern compiles on the evaluated toolchain and adds no runtime work.
Use equivalent witnesses for `Option<T>` and map keys and values.
For nested containers, bridge every missing edge, not only the outermost one.
For `Vec<Option<Child>>`, the outer element hash alone still misses `Child`.
An accessor constraint such as `fn(&Batch) -> &Vec<Option<T>>` can reach that leaf.

Measure each reviewed literal from the real definition.
Keep an independent literal guard on each reachable owned definition when
that makes the coverage easier to audit.
An assertion on a manually named `Child` alone cannot detect replacement of
the actual field's argument with `OtherChild`.

Mutate both the argument type and the definition behind the container.
Require the intended assertion to fail, not merely an unrelated codec type error.
Record nominal standard-library internals as opaque implementation boundaries.
Structural guards cover the application's type graph, not arbitrary future
changes to standard-library source or codec method bodies.

## Literal assertion

Read the hash with a temporary executable or test. Record the emitted `u128`
literal after reviewing the implementation. Keep the assertion in normal
library code, outside `cfg(test)` and `debug_assertions`.

Use this shape with the measured literal substituted for `REVIEWED_LITERAL`:

```rust,ignore
const _: () = assert!(
    <Root as HashedTypeDef>::TYPE_HASH_NATIVE == REVIEWED_LITERAL,
    "Root type tree changed. Ask an LLM to run custom-vercodable in update mode; \
     preserve framing, the frozen base, and global passes; run historical tests, then fix this assert."
);
```

Do not define the expected value from the current hash. That equality is
tautological. Do not refresh it in a build script.
Do not hash a copied mirror definition instead of the real type.
`add_hashed_type_def!` accepts a supplied definition, so a stale copy defeats
the guard's purpose.

For generics, guard each supported concrete root and preserve generic bounds.
A concrete hash is not a guard for every possible instantiation.
If a generic associated assertion is necessary, prove when Rust evaluates it.
An unreferenced associated constant is not an unconditional tripwire.

## Coverage limits

The 0.1.2 derive does not hash attributes, explicit enum discriminant
expressions, method bodies, default implementations, or dependency codec logic.
It is a structural tripwire, not a compatibility proof.

Keep wire versions, tag maps, defaults, and representation assumptions in the
global pass plan and tests. The hash does not encode the frame grammar or length meanings,
effective subtree births, map replay order, or historical codec configuration.
Add compile-time checks for explicit codes and relevant
layout sizes where these properties drive a fast path.
Use behavioral tests for default values and codec semantics.

Some foreign hash implementations describe a nominal type, not its private
definition. A manual constant hash does not detect later private changes.
Do not claim complete graph coverage across that boundary.
Request an upstream structural hash or a suitable owned boundary.
If coverage cannot meet the request, report the blocker instead of faking it.

Recursive graphs can cause cyclic constant evaluation. Identify cycles before
deriving transitively. Use a proven existing cycle-aware fingerprint mechanism
with explicit per-definition guards, or report that full coverage is blocked.
Do not substitute a nominal hash and call it transitive.

Keep the dependency and lockfile stable. Parser or crate updates can change
hashes without changing the wire format. Review those changes like any other
tripwire failure.

Arrays hash `N.to_le_bytes()`, where `N` is `usize`. Their hashes can differ
by target pointer width even when their wire bytes do not.
Capture literals for the project's supported targets.
Do not add speculative target branches or claim unexecuted target coverage.

## Mutation proof

Make temporary copies of the final source and run ordinary `cargo check`:

1. Reorder two root fields without changing their names or types.
2. Reorder fields in a deeply embedded definition without changing the root.
3. Change a container element definition or const-generic array length.
4. Change an actual vector, option, or map argument while leaving its former
   type definition unchanged.

Require a constant-evaluation failure at the intended assertion with its
update diagnostic. An unrelated syntax or type error does not prove coverage.
Restore the copies afterward.

Run the unchanged source again. This catches a permanently broken assertion.
For updates, retain the prior literal until compatibility tests succeed.
If that prevents running the tests, disable only the assertion in a disposable
test copy. Restore the production assertion before capturing the new literal.

## Upstream sources

- [HashedTypeDef trait](https://docs.rs/hashed-type-def-core/0.1.2/hashed_type_def_core/trait.HashedTypeDef.html)
- [Derive implementation](https://docs.rs/hashed-type-def-procmacro/0.1.2/src/hashed_type_def_procmacro/lib.rs.html)
- [Standard type implementations](https://docs.rs/hashed-type-def-core/0.1.2/src/hashed_type_def_core/impl_basis.rs.html)

The derive collects field types and combines their `TYPE_HASH_NATIVE` values.
Its field and variant extraction omits attributes and discriminant expressions.
The standard implementations provide nominal definitions for Vec and Option.
The array implementation hashes its element and target-width length separately.
