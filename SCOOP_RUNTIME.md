# Scoop Runtime Notes

**Version**: 0.1 (Draft)  
**Status**: Implementation-oriented, non-normative unless referenced by `SCOOP_FULL_SPEC.md`.

This document complements `SCOOP_FULL_SPEC.md` and records runtime/toolchain contracts that matter for code generation and safety, including:

- Target properties (word size / pointer size)
- Integer model as it relates to ABI and unsafe code
- Raw pointers (`Ptr<T>`) and their interaction with GC
- Sysroot layout and responsibilities
- GC pacing, heap growth limits, and stress/pacing test knobs
- Immortal compile-time constants and their GC invariants

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

In particular, reactor registrations / completion callbacks / wake tokens should normally store a stable GC handle token rather than a pinned reference. The handle keeps the target object alive across safepoints; pinning is only needed for short-lived raw-address borrowing.

Operational contract:

- Native state should store the stable token (`GcHandle.raw`) rather than a raw object address. The raw address returned by a handle lookup is still only a momentary view and must not be cached across safepoints.
- Copying the token bits does not clone the underlying runtime handle record. Ownership is protocol-defined: one successful `handleNew` still requires exactly one matching `handleDrop`, even if the token is round-tripped through multiple native frames or callback queues.
- Ordinary `@Extern` signatures should round-trip the word-sized token (`UIntPtr` / `GcHandle.raw`), not the `Pinned` wrapper itself. `Pinned` keeps a short-lived raw-address borrow alive on the Scoop side while some separate raw pointer / integer token is in flight.
- If native code needs a raw pointer after `handleGet`, it must separately pin for that short borrowing window. `Pinned` is not the long-lived identity object.
- The low-level runtime ABI may report stale / unknown handle lookup as `NULL` / `0`; the language-level `GC.handleGet` / `GC.handleDrop` surface may then trap. In practice this means late callbacks after cancellation must treat the old token as invalid rather than trying to resurrect it.

### 4.4 Ordinary `@Extern` boundary is effect-impermeable

The lowering/runtime contract for an ordinary `@Extern` call is deliberately narrow:

- the compiler exposes managed roots via `scoop_enter_native(root_slots, len)`,
- performs the native leaf call using the platform C ABI, and then
- restores managed mode with `scoop_leave_native()`.

This boundary does **not** install an `EffectCtx` / `EffectOutcome` propagation contract. Native code must not treat an ordinary `@Extern` return as a channel for:

- outward effect propagation,
- continuation resumption/replay, or
- longjmp-like non-local control unwinding through Scoop frames.

Returning or accepting raw tokens such as `FunPtr<F>`, `UIntPtr`, or `GcHandle.raw` does not create an effect/control bridge. The original `@Extern` call remains Pure/effect-impermeable, and any later `FunPtr<F>` call is still lowered as an ordinary native function-pointer call.

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

### 7.0 Runtime C Build Ownership

Runtime C compilation and final native linking are owned by the `scoopld` crate. The `scoop` facade may collect object paths from cone artifact manifests and dispatch `scoopc link-cone`, but it must not implement runtime C compilation or final link logic in-process.

Current contract:

- `scoopld::compile_runtime_to_obj_dir` materializes `runtime/c/*.c` into a runtime artifact directory for the current link.
- cone-local C/C++ native sources are compiled inside `scoopc build-single-cone`; their `objs/native_*.o` entries are part of that cone artifact and are linked only because the manifest lists them.
- `scoopld::link` computes an independent link fingerprint from consumer/dependency objects, runtime artifact contents, link kind, linker/version, and link flags.
- `scoopc link-cone` is only a CLI execution wrapper around `scoopld::link`; it does not own linker behavior, DAG traversal, or scheduling.
- `scoop` owns only facade scheduling: it may materialize a virtual cone, dispatch `scoopc build-single-cone`, read cone artifact manifests, and dispatch `scoopc link-cone`. It must not call compiler or linker library APIs in-process.
- Future platforms with precompiled runtime artifacts may replace the runtime compilation step with a `scoopld`-owned noop/cache provider without changing `scoop` or stage crates.

### 7.1 Explicit root frame substrate contract

The runtime now exposes an explicit root frame substrate for the managed-root path that is being migrated away from stackmap-only lookup.

Current ABI contract:

- Each activation frame object that participates in explicit root tracking begins with `ScoopRootFrameHeader` as its first field.
- `ScoopRootFrameHeader` stores:
  - `prev`: previous frame in the current thread's explicit frame chain.
  - `desc`: pointer to a `ScoopRootFrameDesc` describing the frame's managed root home slots.
- `ScoopRootFrameDesc` stores:
  - `slot_count`: number of managed root slots in the frame.
  - `slot_offsets`: offsets, relative to the frame base, for each `void*` root home slot.
- Because `hdr` is the first field, a `ScoopRootFrameHeader*` is also the frame base used with `slot_offsets`; runtime traversal must recover slots by `frame_base + offset`, not by inferring SP/FP layout.
- The current thread's chain top is exported as `__scoop_explicit_root_frame_top` and is cleared on thread teardown.
- During stop-the-world parking and `enter_native`, runtime snapshots the current thread's explicit frame top into the GC thread record. When that snapshot is non-NULL, managed-root enumeration must use the explicit frame chain directly instead of requiring stack walking / stackmap lookup for higher managed caller frames.

Defined edge cases:

- `__scoop_explicit_root_frame_top == NULL` means the current thread has no explicit managed frames and is a valid steady state.
- A frame with `slot_count == 0` is valid. It still participates in the chain shape, but visiting it yields zero root slots.
- A non-NULL frame must provide a non-NULL `desc`; if `slot_count > 0`, `slot_offsets` must also be non-NULL. Violating either condition is a runtime contract error.
- `native_roots` remains a separate buffer for native-boundary temporary roots only. It is not the mechanism for recovering higher managed caller frames when an explicit frame snapshot is available.

This substrate is intentionally narrower than a full activation-record model:

- It describes only stack-backed managed root home slots.
- It does not describe arbitrary locals, machine spill slots, heap-backed continuation/effect frame fields, or stack layout reconstruction rules.
- Later compiler work may choose not to materialize an explicit frame for functions with zero managed root slots, but that choice must remain explicit and preserve the `NULL` top / zero-slot contracts above.

## 8. Sysroot Directory Structure

The **sysroot** is a directory of `.scoop` source files shipped with the toolchain. It defines the built-in API surface that the compiler recognizes (intrinsics, core types, effect declarations).

In this repository, the sysroot is located at:

```
sysroot/
  collections.scoop
  core.scoop
  delegates.scoop
  unsafe.scoop
  print.scoop
  string.scoop
  sync.scoop
  task.scoop
  thread.scoop
```

Status note (2026-04-26): the early-stage `channels.scoop`, `env.scoop`, `fs.scoop`, `io.scoop`, `net.scoop`, `path.scoop`, `process.scoop`, `test.scoop`, and `time.scoop` surfaces have been removed from the sysroot pending redesign. Executable entry points now accept exactly four `main` forms: `fun main(): Unit / Pure!`, `fun main(): Int / Pure!`, `fun main(args: Array<String>): Unit / Pure!`, and `fun main(args: Array<String>): Int / Pure!`. For the `args` forms, argv arrives directly through the program boundary as the full native argv vector, including `argv[0]`. A `Unit`-returning `main` exits with code `0` on normal return; an `Int`-returning `main` uses that value as the process exit code. Other early-exit paths such as `panic(...)` remain separate runtime behavior rather than part of `main`'s normal return contract.

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
| `flags` | `uint32_t` | `2*PTR_SIZE + 8` | Runtime/GC flags, including `SCOOP_GC_FLAG_IMMORTAL`. |
| `mark` | `uint32_t` | `2*PTR_SIZE + 8 + 4` | Mark bits / forwarding state; immortals use `SCOOP_GC_MARK_IMMORTAL`. |

Additional invariants:

- `size_bytes >= sizeof(ScoopGcObjectHeader)`
- `size_bytes` SHOULD be aligned to `PTR_SIZE` (current runtime enforces pointer-aligned header size; allocators should round up).
- The runtime MUST keep these offsets stable via C `_Static_assert`, and Rust tests MUST assert the same layout.

#### 9.2.1 Immortal ref-object headers

Compile-time constants that are reference objects, such as immortal `String` literals, are not allocated by `scoop_alloc(...)` and are not threaded onto the runtime heap object list.

Their header contract is:

- `next == NULL`; the object is never part of sweep traversal.
- `flags` contains `SCOOP_GC_FLAG_IMMORTAL` (`0x80000000`).
- `mark == SCOOP_GC_MARK_IMMORTAL` (`0xFFFFFFFF`).
- The storage is read-only global data; marker helpers MUST return before reading or writing `mark` or tracing payload fields.
- The invariant is: immortal objects are never written and never traced. Any object that is writable or may require tracing must remain a normal heap object or a registered global root, not an immortal.

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
- `object` is the pointer to the heap object header, not just the payload. Compiler-generated `@ReleaseHook` trampolines use the full object layout to read the configured fields.
- Language-level `@ReleaseHook` is the user-facing way to populate this slot for final non-generic classes. The compiler verifies that every released field is GC-free and that the target release function is either `@NoGC` or `@Extern(abi = "c")` before emitting a trampoline.
- Release is best-effort cleanup for unmanaged resources, not deterministic destruction. The runtime does not call `release_fn` for objects that remain live at process exit or runtime teardown.
- `release_fn` MUST NOT resurrect the object, and MUST NOT assume any other objects are still alive.
- `release_fn` MUST NOT allocate, trigger GC, or call runtime APIs that can re-enter the GC or contend on the GC lock. The callback runs in a restricted GC context, typically while the world is stopped and the GC lock is held.
- If a type also exposes an explicit `close` / `destroy` API, the underlying native release operation should be idempotent so explicit release and later best-effort GC release cannot double-free the same native resource.

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

### 9.4 GC Pacing And Heap Growth Contract

GC pacing is enabled by default. The old behavior of unbounded heap growth is retained only behind `SCOOP_GC_PACING=off` for tests that require deterministic heap counters.

The pacing model is a live-heap target:

```text
live        = max(bytes_allocated - bytes_freed, 0)
target_live = max(min_threshold, ceil(live * growth_factor))
next_gc     = bytes_freed + target_live
```

`next_gc` is stored as a cumulative `bytes_allocated` waterline because the debug counters are cumulative. This is equivalent to the target heap model above and preserves the existing counter ABI.

Trigger layers:

- Soft trigger: after an allocation is registered, the runtime compares cumulative `bytes_allocated` with `next_gc`. If the waterline is reached, it sets an idempotent `request_collect` flag. The next allocation or write-barrier safepoint consumes the flag and runs collection; on the allocation path this happens before allocating the next object. Collection is not run synchronously inside the allocation that just produced an unrooted object.
- Generational trigger: when the Immix nursery is full, the runtime attempts a minor GC and retries the nursery allocation. If the nursery is still unsuitable, the allocation falls back to old space rather than looping.
- Hard trigger: when the Immix block pool is exhausted, the runtime attempts a full GC before reserving/growing more heap memory. `SCOOP_GC_MAX_HEAP_BYTES` applies at the post-GC retry; if the allocation still requires growth beyond the cap, allocation returns `NULL` through the existing OOM path.

Environment knobs:

| Variable | Default | Contract |
|---|---:|---|
| `SCOOP_GC_PACING` | `on` | Set to `off`, `0`, `false`, or `no` to disable pacing. Any other non-empty value keeps pacing enabled. |
| `SCOOP_GC_HEAP_TARGET_GROWTH_FACTOR` | `1.5` | Multiplier in `live * growth_factor`; invalid, non-finite, or `<= 1.0` values fall back to the default. |
| `SCOOP_GC_HEAP_MIN_THRESHOLD_BYTES` | `4 * 1024 * 1024` | Minimum target-live threshold; invalid or zero values fall back to the default. |
| `SCOOP_GC_MAX_HEAP_BYTES` | `0` | Immix hard cap on reserved heap bytes; `0` means no cap. |
| `SCOOP_GC_STRESS` | off | Test-only pre-allocation collection trigger. Unset, `0`, `false`, `no`, or `off` disables it; numeric `N >= 1` collects before every Nth allocation; any other non-empty value collects before every allocation. When active, stress bypasses pacing because it collects more aggressively than pacing. |

Manual `scoop_gc_collect()` and `scoop_gc_collect_minor()` calls keep their explicit API semantics. The soft pacing state and `SCOOP_GC_PACING` / min-threshold / growth-factor behavior are backend-independent and are honored by the `immix`, `hosted`, and `minimal` backends; backend-specific collection effectiveness may still differ. The hard cap is enforced by the Immix reserved-growth paths.

### 9.5 Immortal Compile-Time Constants

The compiler may lift values that are fully known at compile time into immutable global data. This is a code generation contract, not a new surface-language allocation guarantee.

There are two tiers:

- Value tier: scalars, tuples, and value `struct`s such as `Platform` are emitted as plain constant data without a GC header. Uses load or copy the value; the GC never receives a pointer to the aggregate itself.
- Ref tier: immutable reference objects such as `String` are emitted as off-heap globals with the immortal header contract from §9.2.1. They are transparent to the collector and must remain read-only.

The compiler computes a structural `is_immutable(T)` predicate for ref-tier lifting and related safety checks:

- Any nominal type carrying compiler-recognized `@InteriorMutable` is not immutable.
- Built-in scalar value types are immutable.
- Value structs, tuples, and `Option<T>` are immutable when their field/element types are immutable.
- `String` is immutable.
- Ref classes are immutable only when the class and its superclasses expose no `var` fields and every field type is immutable.
- Interfaces, function values, unions, type parameters, star projections, unknown layouts, or missing metadata are conservative `false`.

`@InteriorMutable` is metadata-only and has no codegen payload. It exists for types that mutate storage behind `val` fields through unsafe/compiler intrinsics. The sysroot `scoop.unsafe.__AtomicInt` is a distinct `@InteriorMutable` one-word struct with `Int` layout; aliases resolve back to the marked nominal so the marker is not erased.

Deduplication is separate from constantization:

- `String` and type-name strings use a content pool keyed by the UTF-8 bytes. Equal content reuses the same `__scoop_str_data_<hash>` byte global and `__scoop_str_lit_<hash>` wrapper global.
- `Platform` is a value-tier `struct` constant whose five fields point at immortal `String` globals.
- Other liftable ref-tier constants are site-keyed rather than content-pooled. This keeps cross-site identity-sensitive behavior conservative while still avoiding per-use heap allocation at that literal site.

Code must not rely on string literal allocation freshness. Identity-sensitive low-level APIs may observe the content-pool result for strings; `String` equality remains content equality.

## 10. Effect / Continuation Lowering Contract

This section records the continuation/effect contract after the `T4016` alignment work and the `T4017a` documentation alignment. The public language surface is unchanged, but the implementation-facing model is now described in terms of explicit dynamic context plus explicit eager outcome rather than TLS side channels as the semantic source of truth.

- Language-level continuation model: `Continuation<Resume, Answer, eff E = Pure>`.
  - `Resume` is the handled operation's result type.
  - `Answer` is the nearest `handle` delimiter's result type.
  - `E` is the required effect row of the resumed computation.
- Internal semantic model:
  - `EffectCtx` represents the dynamic handler/delimiter environment visible to the current computation. It is distinct from a state-machine frame or continuation object; the frame holds local continuation state, while `EffectCtx` describes the surrounding effect environment.
  - `EffectOutcome<R>` is the eager step result of an effectful computation: either `Complete(R)` or `Propagate(EffectSignal)`.
  - `EffectSignal` is the outward-propagated control/effect event carrying the operation/effect-instance identity, payload transport, and any resume token needed to continue the suspended computation.
- `k.resume(payload...): Answer / (E + Raise<RuntimeError>)`.
  - Payload surface continues to follow the resume payload shape:
    - `Continuation<Unit, Answer>.resume()` is accepted.
    - `Continuation<(A0, A1, ...), Answer>.resume(v0, v1, ...)` is accepted, and named args use `a0`, `a1`, ...
    - The legacy single-payload form `k.resume(value)` remains accepted for compatibility, including tuple payloads passed as one tuple value.
  - If the resumed computation normally reaches the delimiter, `k.resume(...)` returns that delimiter answer to the call site and local code after the call continues.
- User-facing resuming handler syntax is only `Effect.op(args), k -> expr`.
  - Legacy `-> resume` syntax is removed from the language surface.
  - If the compiler recognizes tail-position `k.resume(...)` and lowers it more directly, that is an internal optimization class rather than a distinct surface form.
- Resuming continuations remain one-shot and deep:
  - The continuation captures both the suspended computation's local continuation state and the dynamic effect context active at the suspension point.
  - Resuming from another OS thread runs under that captured effect context rather than the resuming thread's ambient effect state.
  - If the resumed computation suspends again, that suspension exposes a fresh continuation; `k.resume(...)` itself still exposes only the eventual delimiter answer or a raised error.
- Resume payload transport still uses the erased `(resume_word, resume_gc_ref)` carrier, but the owner of the resume algorithm is now compiler-generated code rather than runtime C helpers.
- `Continuation.resume(...)` lowers to a generated continuation driver that:
  - writes the payload into the codegen-owned continuation object;
  - restores the captured `EffectCtx` / resume-token inputs;
  - calls the owner step/core through the explicit hidden ABI `current_effect_ctx_ref + incoming_resume_token_ref + ScoopEffectOutcome *outcome`;
  - rebuilds the published `Step_F` / delimiter-answer result from explicit `EffectOutcome`.
- The runtime no longer carries handler-stack snapshots, perform-slot TLS, `callee_suspend_state`, pending-continuation replay state, or other continuation/effect policy scratch state. Those semantics now live in the backend-owned continuation object model plus explicit `EffectCtx` / `EffectOutcome` contract.

## 11. Task Stepping Layer Contract

`Task.step()` is defined entirely by ordinary Scoop code (`sysroot/task.scoop`)
rather than by any task-specific runtime polling ABI. The authoritative task
driver lives at the Scoop layer:

- `__task_create(...)` creates a lazy task object with ordinary Scoop state
  `Created(start)`.
- `__task_step_ready(...)` and `__task_step_pending(...)` build the private
  `__TaskStepResult<T>` carrier in ordinary Scoop code.
- `Task.step()` reads and updates the ordinary Scoop `__TaskState<T>` object
  model.
- `__task_join(...)` is an internal Scoop helper that repeatedly calls
  `step()` and `yield()` until the task completes; it is still compiler /
  runtime / test-harness helper surface, not a public structured-concurrency
  contract.

The runtime layer only provides generic substrate used by that Scoop-level
driver:

- GC-managed class / enum allocation and tracing;
- generic sync / atomic primitives used to implement exclusive drive ownership
  and state publication;
- generic thread primitives (`yield`) used by the internal join helper;
- the compiler-generated continuation resume driver plus the standardized
  state-machine frame transport (`resume_state_tag`, `resume_word`,
  `resume_gc_ref`) used when a waiting task resumes a captured continuation.

Stable synchronization contract:

- each task may have at most one active public driver at a time;
- different threads may drive the same task sequentially, but only after the
  previous drive attempt has published the next state and released ownership;
- `Pending` means genuine suspension / not-ready state, never drive contention;
- if `step()` races another active `step()` call, reenters the same task, or
  observes the task already in `Running`, the task layer traps immediately
  rather than returning `Pending` or raising `RuntimeError`;
- no synchronization primitive or ownership claim is held while running user
  code or while calling `Continuation.resume(...)`;
- publishing the next `Completed(...)` / `Waiting(...)` state establishes the
  visibility required for later cross-thread sequential handoff.

Implementation note:

- `sysroot/task.scoop` now uses a per-task `__claim: scoop.unsafe.__AtomicInt`
  field as the only ownership bookkeeping for public drive attempts.
- The current claim protocol is a SeqCst `cmpxchg(__claim, 0, 1)` on acquire and
  a SeqCst `store(__claim, 0)` on release. That is sufficient for the intended
  contract: one active public driver at a time plus synchronized state
  publication for sequential cross-thread handoff.
- These fatal trap paths are expressed in the core surface as
  `scoop.core.panic(message: String): Nothing`; the current runtime still maps
  that intrinsic to immediate process termination (`exit(3)` / equivalent), but
  the `exit(3)` detail now stays behind the runtime boundary rather than leaking
  through task/sysroot user code.
- Claim failure is fatal driver misuse, not a recoverable result: the task layer
  does not spin, yield, block, or translate contention into `Pending`.
- The claim is intentionally an implementation detail rather than a user-visible
  synchronization API; it exists only to protect short state inspection /
  publication windows.
- The private `__TaskStepResult<T>` carrier is not part of the public language
  surface.
- A suspended task is just an ordinary task object plus a private raw
  continuation whose delimiter answer is `__TaskStepResult<T>`; the compiler may
  erase the continuation payload type internally, but it does not erase the
  answer carrier.
- When a waiting task resumes that captured continuation, it goes through the
  generated continuation resume driver and the standardized frame transport
  (`resume_state_tag`, `resume_word`, `resume_gc_ref`).
- There is no separate task-only C ABI for `Task.step()`, `__task_create(...)`,
  `__task_step_ready(...)`, `__task_step_pending(...)`, `__task_from_result(...)`,
  or `__task_join(...)`.
- The only remaining task-specific compiler/runtime special handling is the
  transport helper pair `__task_transport_pack(...)` /
  `__task_transport_unpack(...)`, which exists solely to preserve `(word, gc_ref)`
  movement for erased payloads crossing the generic continuation path.
