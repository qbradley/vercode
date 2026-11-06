# Guard the reviewed type graph

The guard connects the real schema to a reviewed codec.
A changed definition must fail ordinary compilation with instructions to use `custom-vercodable` in update mode.
Only after historical compatibility succeeds does a new literal hash become justified.
An automatically refreshed hash, copied mirror type, or test-only assertion breaks that connection.

## The non-obvious container gap

In `hashed-type-def` 0.1.2, `Vec<T>` and `Option<T>` hashes are nominal: they omit `T`.
A root assertion alone can therefore miss both child changes and replacement of a container argument.
Guard each uncovered edge through the **actual field type**, not just a separately named child.

The verified dependency is:

```toml
hashed-type-def = { version = "=0.1.2", features = ["derive"] }
```

The derive requires `use hashed_type_def::HashedTypeDef` in scope.
With that derive on the real `Root` and child definitions, this pattern binds a guard to `Root.entries`:

```rust,ignore
const fn entries_hash<T: HashedTypeDef>(_: fn(&Root) -> &Vec<T>) -> u128 {
    T::TYPE_HASH_NATIVE
}

const _: () = assert!(
    <Root as HashedTypeDef>::TYPE_HASH_NATIVE == REVIEWED_ROOT_LITERAL
        && entries_hash(|root| &root.entries) == REVIEWED_CHILD_LITERAL,
    "Type tree changed. Run custom-vercodable in update mode; \
     preserve wire history, run compatibility checks, then review the hash."
);
```

The literals come from the real, reviewed definitions.
The function pointer is a type witness; the const function does not call it.
Bridge every nominal wrapper: `Vec<Option<Child>>` still needs coverage through to `Child`.
Map arguments and other wrappers need the same inspection.

## What this cannot prove

Structural hashes do not cover attributes, explicit discriminant expressions, default behavior, method bodies, or dependency codec logic.
Wire metadata and semantic compatibility still need independent checks.
A named old child guard does not detect replacement of its field by a different child type.

Opaque foreign definitions block complete structural coverage unless their provider supplies it.
Recursive graphs can cause cyclic constant evaluation and need a proven cycle-aware strategy.
Unknown downstream generic arguments cannot receive a universal, unconditional literal guard.
Use concrete supported roots or state the unsupported guarantee.
An unused associated const need not be evaluated.

Dependency/parser changes can alter hashes without changing bytes.
Array hashes include a target-width `usize` length, so supported targets can need different reviewed literals.
Keep dependency identity and actual target coverage explicit.

## Evidence of coverage

Mutation checks must reach the intended assertion with its update diagnostic, not merely cause an unrelated type or syntax error.
Cover root and embedded changes, container element definitions, and replacement of actual container arguments.
The unchanged production source must still compile.

For an update whose stale guard prevents compatibility tests, a disposable test copy can temporarily omit that assertion.
The final production guard remains unconditional and uses the newly reviewed literal.
This is not permission to refresh the hash before reviewing the codec.

## Sources

- [HashedTypeDef trait](https://docs.rs/hashed-type-def-core/0.1.2/hashed_type_def_core/trait.HashedTypeDef.html)
- [Derive implementation](https://docs.rs/hashed-type-def-procmacro/0.1.2/src/hashed_type_def_procmacro/lib.rs.html)
- [Standard type implementations](https://docs.rs/hashed-type-def-core/0.1.2/src/hashed_type_def_core/impl_basis.rs.html)
