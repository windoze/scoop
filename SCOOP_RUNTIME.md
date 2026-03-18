# Scoop Runtime Notes

**Version**: 0.1 (Draft)  
**Status**: Implementation-oriented, non-normative unless referenced by `SCOOP_FULL_SPEC.md`.

This document complements `SCOOP_FULL_SPEC.md` and records runtime/toolchain contracts that matter for code generation and safety, including:

- Target properties (word size / pointer size)
- Integer model as it relates to ABI and unsafe code
- Raw pointers (`Ptr<T>`) and their interaction with GC
- Sysroot layout and responsibilities

## 1. Scope

The language specification (`SCOOP_FULL_SPEC.md`) defines the surface language and its semantics. The runtime document focuses on:

- How the compiler and runtime agree on low-level representations
- What must be enforced statically to keep unsafe primitives tractable

The implementation may evolve; keep this file aligned with `PLAN.md`.

## 2. Targets, word size, and pointer size

Scoop is a native language. The compilation target determines:

- Pointer size (typically 32-bit or 64-bit)
- ABI details (alignment, calling conventions, endianness)

Following Swift's convention:

- `Int` / `UInt` are **word-sized integers** (their bit width equals the target pointer size).
- This implies: on 64-bit targets `Int`/`UInt` are 64-bit; on 32-bit targets they are 32-bit.

When writing portable code:

- Use `Int` / `UInt` for sizes/indices/offsets.
- Use fixed-width integers for stable data formats and FFI layouts.

## 3. Integer types and standard aliases

Integer types in Scoop are **language built-ins**: their layout and core semantics are fixed by the compiler. The sysroot MUST provide their *visible* declarations (including standard aliases) so that user code and tools can reference them like normal types.

### 3.1 Word-sized integers

- `Int`: signed word-sized integer
- `UInt`: unsigned word-sized integer

`UIntPtr` is provided as a readability alias:

```kotlin
typealias UIntPtr = UInt
```

`UIntPtr` exists to make it explicit when a value is intended to represent an address or an address-sized quantity. The type itself is not unsafe.

### 3.2 Fixed-width integers

The sysroot MUST provide these fixed-width integer types:

- Signed: `Int8`, `Int16`, `Int32`, `Int64`
- Unsigned: `UInt8`, `UInt16`, `UInt32`, `UInt64`

Future extension (not in 0.1): `Int128` / `UInt128` may be supported on some targets.

### 3.3 Conventional aliases

For conventional naming, the sysroot MUST define:

```kotlin
typealias Byte   = UInt8
typealias Short  = Int16
typealias UShort = UInt16
typealias Long   = Int64
typealias ULong  = UInt64
```

Note: `Byte` is defined as an **unsigned** 8-bit integer (`UInt8`).

## 4. Unsafe pointers (`Ptr<T>`) and GC-free types

The sysroot may expose a raw pointer primitive:

```kotlin
@Intrinsic
struct Ptr<T>
```

### 4.1 GC-free pointee restriction

To avoid hiding GC-managed references inside memory the GC cannot see, `Ptr<T>` must be restricted:

- `T` must be a value type, and
- `T` must be **GC-free**: it must not contain (directly or transitively) any reference-typed fields or any field whose runtime representation contains a GC pointer.

This includes niche-optimized representations such as `Option<RefType>` where the value is still represented as a GC pointer.

This is a *static* well-formedness rule. It is not enough to say “value type” — value types can still contain reference fields, which would be invisible to the GC if stored in untracked memory.

### 4.2 Interaction with GC pinning

Even if `T` is GC-free, a pointer may still point *into* GC-managed memory. For moving/compacting GCs, such pointers are only valid while the owning object is pinned.

The language-level pinning API is described in `SCOOP_FULL_SPEC.md` §15.10.

## 5. Pointer ↔ integer casts

Casting between pointers and integers is inherently unsafe and must only be permitted in an unsafe context (`@Unsafe` function body or `@Unsafe { ... }` block).

The recommended integer type for “address as integer” is `UIntPtr` (alias of `UInt`).

Typical intrinsic APIs (shape only; exact names are sysroot-defined):

```kotlin
@Intrinsic @NoGC @Unsafe
fun <T> ptrToUIntPtr(p: Ptr<T>): UIntPtr

@Intrinsic @NoGC @Unsafe
fun <T> uintPtrToPtr(addr: UIntPtr): Ptr<T>
```

Notes:

- These conversions should not be expressed via `as` / `as?` (those are runtime type casts expressed via `Raise`).
- These conversions must not allocate and should be valid in `@NoGC` code.

## 6. Bitwise and shift operations

Integer types MUST support:

- Bitwise: `&`, `|`, `^`, `~`
- Shifts: `<<`, `>>`

The operator-to-method mapping is described in `SCOOP_FULL_SPEC.md` Appendix B.8.

## 7. Runtime ABI surface (early stage)

The repository currently contains an early C runtime (`runtime/c/`) built via `crates/scoop_runtime`.

At this stage, the runtime ABI is expected to grow to include:

- allocation entrypoints (e.g., `scoop_alloc`)
- TLS state for effects (handler stack, perform slot)
- thread registration hooks for GC

This file intentionally does not freeze symbol names yet beyond what is required by current code and fixtures.

## 8. Sysroot Directory Structure

The **sysroot** is a directory of `.scoop` source files shipped with the toolchain. It defines the built-in API surface that the compiler recognizes (intrinsics, core types, effect declarations).

In this repository, the sysroot is located at:

```
sysroot/
  core.scoop
  # Future (suggested):
  # intrinsics.scoop
  # unsafe.scoop
  # gc.scoop
  # io.scoop
```

Conventions:

- Intrinsic declarations are marked with `@Intrinsic` and must have signatures but no bodies.
- Unsafe primitives (e.g., `Ptr<T>`, pointer/integer casts) should live in a dedicated sysroot module (suggestion: `scoop.unsafe`) so that `@Unsafe`-gated APIs are easy to audit.
- The compiler parses sysroot files before user code and resolves references against sysroot declarations as if they were part of the compilation universe.

The exact module layout is not yet frozen; the main requirement is that the sysroot remains a stable, tool-readable source of truth for built-in APIs.
