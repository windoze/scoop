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

The sysroot exposes a raw pointer primitive:

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

### 4.3 Long-lived interop: stable GC handles (no raw addresses across safepoints)

For moving/compacting GCs, native code MUST NOT retain a raw address of a GC-managed object across safepoints (GC may move objects). Pinning is the mechanism for **short-lived** raw pointers into GC-managed memory (typically for the duration of a single synchronous OS call).

For **long-lived** references that must survive safepoints (e.g. storing a reference in native state, async/proactive I/O completion callbacks, global caches), the toolchain must provide **stable GC handles**: an opaque token that remains stable even if the object moves. The recommended language-level API shape is specified in `SCOOP_FULL_SPEC.md` §15.10.1.

## 5. Pointer ↔ integer casts

Casting between pointers and integers is inherently unsafe and must only be permitted in an unsafe context (`@Unsafe` function body or `@Unsafe do { ... }` block).

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

## 7. Runtime ABI surface (toolchain contract)

The repository contains a C runtime (`runtime/c/`) built via `crates/scoop_runtime`. This C runtime is a **long-term component** for Scoop 0.1:

- Platform dependencies must be isolated inside the runtime's platform/backend layer (see `PLAN.md` T14), and must not leak into the Scoop surface language (sysroot/stdlib) via OS-specific types or ad-hoc externs.
- The runtime ABI exposed to Scoop must stay **small and auditable** (see `PLAN.md` T1401).

The exact symbol allowlist is tracked in `TODO.md` and will be enforced by tooling. Until that is in place, treat any runtime symbol referenced by:

- sysroot `@Intrinsic` / `@Extern` declarations, and
- LLVM codegen mappings

as part of the toolchain contract, and update them in lockstep with fixtures when changes are required.

## 8. Sysroot Directory Structure

The **sysroot** is a directory of `.scoop` source files shipped with the toolchain. It defines the built-in API surface that the compiler recognizes (intrinsics, core types, effect declarations).

In this repository, the sysroot is located at:

```
sysroot/
  core.scoop
  delegates.scoop
  collections.scoop
  unsafe.scoop
  env.scoop
  time.scoop
  fs.scoop
  path.scoop
  process.scoop
  io.scoop
  sync.scoop
  thread.scoop
  channels.scoop
```

Conventions:

- Intrinsic declarations are marked with `@Intrinsic` and must have signatures but no bodies.
- Unsafe primitives (e.g., `Ptr<T>`, pointer/integer casts) live in `scoop.unsafe` so that `@Unsafe`-gated APIs are easy to audit.
- The compiler parses sysroot files before user code and resolves references against sysroot declarations as if they were part of the compilation universe.

The sysroot module layout is part of the toolchain contract: changes must update the compiler’s prelude/import behavior, documentation, and fixtures together.

## 9. Heap Object Model (ref) and Runtime Type Descriptors

This section records the **runtime ABI** for GC-managed reference types ("ref types") and their type descriptors.

The source of truth for the C-side layout is:

- `runtime/c/scoop_gc.h`: `ScoopGcObjectHeader` and `ScoopTypeDescriptor`
- `crates/scoop_runtime/tests/object_model_abi.rs`: Rust-side layout assertions that must stay in sync

### 9.1 GC-managed reference pointer representation

- In LLVM IR, any GC-managed reference MUST be represented as an `addrspace(1)` pointer (GC address space).
- A Scoop `ref` value points to the **start of the heap object**, i.e. the beginning of the object header:
  - `ref` runtime value == `ScoopGcObjectHeader*` (conceptually; LLVM may use `i8 addrspace(1)*` and bitcasts).
  - This is the same pointer returned by runtime allocation APIs such as `scoop_alloc(...)` / `scoop_alloc_typed(...)`.
- The **payload** (user-visible fields, array elements, boxed value, closure env, etc.) begins immediately after the header:
  - `payload_ptr = (u8*)obj + sizeof(ScoopGcObjectHeader)`

### 9.2 Object header: `ScoopGcObjectHeader`

**Contract:** every GC-managed heap object starts with the object header.

Field layout (C ABI, `runtime/c/scoop_gc.h`):

Let:

- `PTR_SIZE = sizeof(void*)`

| Field | C type | Offset | Meaning |
|---|---:|---:|---|
| `next` | `ScoopGcObjectHeader*` | `0` | Heap object list link (runtime internal). |
| `type_desc` | `const ScoopTypeDescriptor*` | `PTR_SIZE` | Type descriptor pointer; MAY be NULL in early-stage allocations. |
| `size_bytes` | `uint64_t` | `2*PTR_SIZE` | Total object size in bytes (**header + payload**). |
| `flags` | `uint32_t` | `2*PTR_SIZE + 8` | Reserved flags (runtime/GC). |
| `mark` | `uint32_t` | `2*PTR_SIZE + 8 + 4` | Mark bits / forwarding state (backend-defined). |

Additional invariants:

- `size_bytes >= sizeof(ScoopGcObjectHeader)`
- `size_bytes` SHOULD be aligned to `PTR_SIZE` (current runtime enforces pointer-aligned header size; allocators should round up).
- The runtime MUST keep these offsets stable via C `_Static_assert`, and Rust tests MUST assert the same layout.

### 9.3 Type descriptor: `ScoopTypeDescriptor`

`ScoopTypeDescriptor` is the runtime metadata record for a heap object layout.

It is used for:

- Allocation (typed alloc in later stages): `size_bytes` / `align_bytes`
- Tracing GC-managed pointers inside the object: bitmap or custom trace function
- Optional resource cleanup: `release_fn`
- RTTI + dispatch: `type_id`, `parent_type_desc`, `itable`, `vtable` (TODO T15)

#### 9.3.1 Fields and offsets

The first part of the descriptor uses fixed-width fields so that offsets are stable and easy to assert.

| Field | C type | Offset | Meaning |
|---|---:|---:|---|
| `abi_version` | `uint32_t` | `0` | ABI version (v0: `0`). |
| `flags` | `uint32_t` | `4` | Reserved flags (v0: unspecified). |
| `size_bytes` | `uint64_t` | `8` | Total object size in bytes (**header + payload**). |
| `align_bytes` | `uint64_t` | `16` | Object alignment in bytes (power-of-two; `>= sizeof(void*)`). |
| `trace_start_offset_bytes` | `uint64_t` | `24` | Byte offset from object start where tracing begins. |
| `trace_bitmap_u64_len` | `uint32_t` | `32` | Bitmap length in `u64` words. |
| `_reserved_u32` | `uint32_t` | `36` | Reserved/padding (keeps pointer fields naturally aligned). |
| `trace_bitmap` | `const uint64_t*` | `40` | Pointer to bitmap array (or NULL). |

Pointer/function-pointer fields follow, and their offsets depend on the target pointer width. Define:

- `PTR_SIZE = sizeof(void*)`

Then:

- `trace_fn` offset = `align_up(40 + PTR_SIZE, alignof(trace_fn))`
- `release_fn` offset = `trace_fn_offset + sizeof(trace_fn)`
- `type_id` offset = `align_up(release_fn_offset + sizeof(release_fn), 8)`
- `parent_type_desc` offset = `type_id_offset + 8`
- `itable` offset = `parent_type_desc_offset + PTR_SIZE`
- `vtable` offset = `itable_offset + PTR_SIZE`

The Rust-side assertions are in `crates/scoop_runtime/tests/object_model_abi.rs`.

#### 9.3.2 Trace contract (bitmap vs trace_fn)

- All callbacks use the C ABI (`extern "C"` in Rust terms).
- `trace_start_offset_bytes` MUST be a multiple of `PTR_SIZE`; otherwise v0 tracing treats it as "no refs" (defensive behavior).
- If `trace_fn != NULL`, tracing MUST call `trace_fn(object, visitor, ctx)` and ignore the bitmap.
- Otherwise, if `trace_bitmap != NULL` and `trace_bitmap_u64_len > 0`, tracing scans the object memory range:
  - `[object + trace_start_offset_bytes, object + size_bytes)`
  - in pointer-sized words (`PTR_SIZE`)
  - where bit `i` corresponds to word `i` in that scan window
- The visitor receives a writable slot (`void** slot`) so a moving GC can update pointers in place.

`trace_fn` signature (shape):

```c
typedef void (*ScoopGcTraceVisitor)(void **slot, void *ctx);
typedef uint64_t (*ScoopTypeTraceFn)(void *object, ScoopGcTraceVisitor visitor, void *ctx);
```

#### 9.3.3 Release contract

- `release_fn` MAY be NULL.
- If non-NULL, `release_fn(object)` is called once right before the runtime frees the object memory in sweep/reclaim.
- `release_fn` MUST NOT resurrect the object, and MUST NOT assume any other objects are still alive.
- `release_fn` MUST NOT call back into allocation-heavy runtime APIs (it runs in a restricted GC context).

`release_fn` signature (shape):

```c
typedef void (*ScoopTypeReleaseFn)(void *object);
```

#### 9.3.4 RTTI and dispatch (v0 ABI surface)

The following fields exist in the type descriptor as ABI-stable placeholders; their full semantics are specified by TODO T15:

- `type_id: u64` — stable type identifier (recommended: compiler stable hash of the canonical type name).
  For parameterized nominals, the canonical name should include the concrete type arguments and use-site `eff` row (for example `pkg.Box<Int>` or `pkg.Disposable<eff Raise<RuntimeError>>`), so runtime metadata and RTTI/debug tooling observe the same identity.
- `parent_type_desc` — the parent class descriptor, forming an inheritance chain.
- `itable` — interface table pointer; required for `is/as` checks against interfaces.
- `vtable` — virtual dispatch table pointer; required for class dynamic dispatch.

Runtime type tests (`is/as/as?`) are expected to follow these rules once implemented:

- Class test: check `obj.type_desc.type_id == target_id`, otherwise walk `parent_type_desc` chain.
- Interface test: check whether `itable` contains an entry for the interface id.
- `as` failure path: raise `RuntimeError.ClassCastFailed`.
- `as?` failure path: return `None` (NULL niche).
