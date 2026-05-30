# Lift Compile-Time-Constant Values into `.rodata` (Immortal Objects)

Status: implemented in the current compiler/runtime. This file is retained as
the design history and rationale; `SCOOP_RUNTIME.md` and the language spec record
the active contract. The "current" allocation descriptions below are the pre-P5
baseline, not the current default behavior. Platform-agnostic — applies to every
codegen target. Embedded tier hints (PSRAM placement, sectioning) are deliberately
deferred.

## Motivation

At the pre-P5 baseline, several values that are **fully known at compile time**
were allocated on the GC heap on every use:

1. **String literals** — every `"hello"` in source emits a fresh
   `scoop_alloc_typed` call that materializes a `ScoopString { hdr, len, data }`.
   The byte payload itself is already in `.rodata` (good), but the wrapper
   object is heap-allocated each time the literal expression is evaluated.
2. **Type-name metadata literals** (`TypeMetadataLiteral::TypeNameString`) —
   delegate to the same string-literal path, inheriting the wrapper allocation.
3. **`scoop.core.Platform` literal** — every read of `Platform` emits
   **five** `ScoopString` allocations (one per field) plus an SSA struct insert
   sequence. The struct value is constant for the life of the binary.

Historical cost on representative samples:

- `print("hello world")` inside a hot loop allocates a `ScoopString` on every
  iteration. The mark-region GC tolerates this (pinned/scoped allocators help)
  but it is unjustified pressure on the nursery and the marker.
- `Platform.os` in any `match` chain allocates 5 strings just to read 1 field.
- A program that prints `__type_name(T)` once per element of a list pays the
  full alloc per element.

Beyond pressure, that baseline also broke the user's mental model: `"hello"`
was indistinguishable from `String("hello")` at the IR level, even though the
former is a textbook immortal compile-time constant.

## Design principle: a general decision, not three special cases

An earlier draft of this design special-cased exactly three "customers" (string
literals, type-name metadata, `Platform`). That is the wrong shape. **Whether a
value can be lifted into a compile-time constant is a property of its type, not
a hand-maintained list.** String and `Platform` are simply the first types that
happen to satisfy the property; the mechanism must admit any type that does,
including user-defined ones, with no further compiler changes.

The design rests on two **orthogonal** decisions:

1. **Constantization** — *should this value become a `.rodata` constant?* This is
   a general decision driven entirely by **transitive immutability** of the
   type. It is independent of object identity.
2. **Deduplication** — *should equal constants share one instance?* This is the
   only identity-sensitive decision. It is applied narrowly (String only, for
   now), like a string constant pool in other languages.

Separating these is what lets constantization be universal while keeping the
identity-sensitive behaviour conservative.

### Why immutability is the whole gate for constantization

A type is safe to constantize when it is **transitively immutable**: it is never
mutated after construction, so a single shared `.rodata` instance is
indistinguishable from a fresh allocation on every evaluation.

Scoop makes this property unusually clean:

- The surface language exposes only `==` / `!=`
  (`docs/spec/language_spec-part3.md:66`); there is no general `===`,
  `identityEquals`, or pointer-comparison expression. Low-level library
  primitives may still compare reference bits as part of atomic protocols, so the
  final contract makes String content pooling explicit and keeps non-String
  ref-type dedup conservative.
- Therefore, for immutable values, lifting a literal site into read-only storage
  is the implementation contract. Cross-site deduplication remains a separate,
  narrower decision, currently enabled only for String.

This is why dedup can be decoupled: even when we do *not* dedup, constantizing a
literal already collapses repeated evaluations of one site onto one instance —
and that is sound for the same reason.

### Value types vs ref types — two tiers

The runtime consequences differ sharply by whether the constant is a GC object:

- **Value-type constants** (scalars; value `struct`s like `Platform`; tuples):
  these have **no `ScoopGcObjectHeader`**. Placed in `.rodata` they are just
  plain `constant` data the GC never receives a pointer to. The runtime GC
  changes below are **irrelevant** to them. Each use copies by value.
- **Ref-type constants** (`String`, and any immutable `class`): these **carry a
  `ScoopGcObjectHeader`** and the GC *can* receive a pointer and try to trace
  them. These are the objects that need the immortal-header treatment and the
  runtime marker short-circuit.

`String` is the critical ref-type target: it is the only built-in ref type with
value semantics, and string literals are everywhere. It is a hard requirement
that this design cover it.

## The constantization predicate `is_immutable(T)`

A structural, recursive, memoizable predicate computed from type/field metadata
available at MIR/codegen time (`FieldDecl.mutable` survives into
`crates/scoopc_hir/src/hir/mod.rs:205`):

```
is_immutable(T):
    has @InteriorMutable     -> false   // see "Interior mutability" below
    value scalar             -> true    // Int, Bool, Float64/32, Char, Unit
    value struct / tuple     -> every field type is_immutable
                                  (struct fields are always `val`; see note)
    ref class                -> every field is `val`
                                  AND every field type is_immutable
```

Notes:

- The recursion into **field types** is essential: `val x: RefCell<T>` is an
  immutable *binding* to a *mutable* object, so the field type fails and the
  enclosing type fails.
- For a value `struct` the "all fields `val`" check is **vacuous**: Scoop
  structs may only declare `val` fields (deliberately — allowing `var` struct
  fields invites Swift-style `mutable self` and `Self`-type complications, which
  the language does not intend to take on). The `var`-field check therefore does
  real work **only** in the `ref class` arm, which is exactly where the atomics
  and `RefCell` are caught.

### Interior mutability — the `@InteriorMutable` marker

Some types are mutated **in place behind the language's back** by unsafe
intrinsics that take a storage slot by address, with no `var` field exposing it.
For a value `struct` (every field necessarily `val`), structural inspection can
*never* detect this. Such a type must carry an explicit `@InteriorMutable`
marker; `is_immutable` returns `false` for it before any structural reasoning.

The marker is metadata only — **no codegen**. It keeps the decision
characteristic-driven (the predicate keys on a *property*, not on a type name),
and it is reusable by any future "mutated-via-unsafe-behind-`val`" type.

#### `scoop.unsafe.__AtomicInt`: typealias → marked struct

Today `__AtomicInt` is a transparent alias
(`sysroot/lib/scoop.unsafe/src/unsafe.scoop:163`):

```scoop
public typealias __AtomicInt = Int
```

It is erased to plain `Int` at the type level
(`crates/scoopc_hir/src/typecheck/lower.rs:2662` / `:3522`,
`crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs:436`,
`crates/scoopc_hir/src/hir/lower/util/generic_layouts.rs:89`,
`crates/scoopc_hir/src/hir/lower/main/impl_lowering.rs:1724`). Its atomic nature
lives entirely in the intrinsics (`__atomicIntStore` requires a mutable lvalue
slot and emits an atomic write on that slot's storage), not in the type.

That is a latent trap for this design: a value `struct` holding a
`val raw: __AtomicInt` field would, after erasure, look like
`{ val raw: Int }` — fully immutable — and wrongly constantize into `.rodata`,
where the next atomic store faults on a read-only page. A marker on a *typealias*
cannot save us, because the alias collapses to `Int` and any re-alias would lose
it too.

Fix — promote it to a distinct, marked struct:

```scoop
@InteriorMutable
public struct __AtomicInt { val raw: Int }
```

- A normal one-field struct: the compiler derives its word layout and its
  `__AtomicInt(initial)` constructor for free — **no bespoke codegen**, unlike a
  body-less `@Intrinsic struct`.
- A distinct nominal type: aliases to it resolve back to the marked nominal, so
  the marker cannot be stripped by re-aliasing.
- `val raw` is the only legal form (structs are `val`-only), so `@InteriorMutable`
  is load-bearing rather than redundant, and it also forbids non-atomic
  `this.raw = x` reassignment — all mutation must go through the atomic
  intrinsics.

Ripple (small, contained):

- The five erasure sites above change from "type = `Int`" to "type =
  `__AtomicInt` nominal, layout/ABI = `Int` word". The
  `__atomicIntLoad/Store/CompareExchange` signatures are unchanged (they take
  the struct by lvalue and operate on its storage as an `i64`).
- `core.scoop`'s atomics construct explicitly — there is **no implicit Int ↔
  `__AtomicInt` coercion** (the language has no implicit coercion, and forcing
  load/store/construct to be written out is the correct discipline):

  ```scoop
  // AtomicInt
  var raw: __AtomicInt = __AtomicInt(initial)
  // AtomicBool
  var raw: __AtomicInt = __AtomicInt(__atomicBoolToInt(initial))
  ```

  The struct constructor *is* the explicit construction surface; no separate
  `__atomicIntInit` intrinsic is needed. (`AtomicInt` / `AtomicBool` /
  `Atomic<T>` remain `class`es with `var raw`, so they are independently caught
  by the `ref class` arm; the marker is defense-in-depth for the value-struct
  case and for re-aliasing.)

## The generic immortal folder

A single recursive, content-hash-keyed entry point on codegen replaces the
per-type lifting helpers:

```
try_emit_immortal(value) -> Option<GlobalValue>
```

Dispatch:

- `ConstValue::Bool / Int / SynthInt / Float64 / Float32 / Char / Unit`
  → LLVM scalar constant (value-type tier; trivially "immortal" — never a GC
  object). These already avoid allocation; folding only unifies the entry point.
- `ConstValue::String / SynthString` → immortal `ScoopString` global
  (ref-type tier).
- `TypeMetadataLiteral::TypeNameString` → routes through the string path,
  inheriting the immortal global automatically.
- `Rvalue::StructLit / MakeTuple` → if the **lift gate** passes, emit a constant
  aggregate global (value-type tier if the aggregate type is a value type;
  ref-type tier with an immortal header if it is a ref type); otherwise return
  `None` and let the caller fall back to the existing dynamic path.

### The lift gate

A `StructLit` / `MakeTuple` is liftable iff **all** of:

1. Every field is `Operand::Const` (this version does not chase
   `Local` → defining statement, so nested aggregates referenced via a `Local`
   fail and fall back — consistent with the "pure data, no EnumVariant" scope).
2. Every field's `transport.boxing.is_none()` — i.e. no `AnyErasure` /
   `RefErasure` / `ClosureCapture` / etc. boxing intent
   (`crates/scoopc_mir/src/mir/transport.rs`: `AggregateTransportMetadata.fields[i]
   .transport.boxing: Option<MirBoxingIntent>`). Any boxing means the in-memory
   layout is not a flat `memcpy` and must use the dynamic path.
3. `transport.kind` is `Tuple` or `Struct` (not `EnumPayload` / `ClosureEnv`).
4. For a **ref-type** aggregate, additionally `is_immutable(aggregate_ty)`.
   (Value-type aggregates do not need this: a value copy has no shared pointer
   and no observable identity. Mutability of a value type after construction
   hits a *copy*, never the `.rodata` original.)

A field whose `transport.requirements.trace == true` does **not** block lifting:
a traced field is a managed reference, but as long as the referent is itself an
immortal off-heap global (which recursion guarantees), the GC's heap-membership
check skips it. `Platform`'s five string fields are exactly this case.

## Emission shapes

### Ref-type immortal `ScoopString`

```llvm
@__scoop_str_lit_<hash> = private unnamed_addr constant
  %ScoopString {
    %ScoopGcObjectHeader {
      ptr null,                       ; next: never threaded onto the heap list
      ptr @__scoop_string_type_desc,  ; type_desc: already a constant global
      i64 <store_size>,               ; size_bytes
      i32 <SCOOP_GC_FLAG_IMMORTAL>,   ; flags: new bit (see runtime change)
      i32 <SCOOP_GC_MARK_IMMORTAL>    ; mark: new sentinel
    },
    i64 <byte_count>,
    ptr @__scoop_str_data_<hash>      ; existing byte array, also constant
  }
```

`set_constant(true)` + `set_unnamed_addr(true)` so the linker can fold equivalent
literals across translation units.

### Value-type immortal aggregate (`Platform`)

No GC header — a plain constant value struct of field pointers:

```llvm
@__scoop_platform_literal = private unnamed_addr constant
  %ScoopCorePlatform {
    ptr @__scoop_str_lit_<hash_of_triple>,
    ptr @__scoop_str_lit_<hash_of_arch>,
    ptr @__scoop_str_lit_<hash_of_vendor>,
    ptr @__scoop_str_lit_<hash_of_os>,
    ptr @__scoop_str_lit_<hash_of_env>
  }
```

Each field references an immortal `ScoopString` from the ref-type tier. The
aggregate itself is GC-transparent because no pointer to it is ever handed to the
collector (it is a value, loaded/copied at use).

## Deduplication (String only, for now)

Dedup is the **only** identity-sensitive behaviour and is kept separate:

- **String**: content-pool. Key `__scoop_str_lit_<hash>` and
  `__scoop_str_data_<hash>` by `base16(SHA-256(bytes)[..16])` instead of source
  span, so equal literals across all sites collapse to one global. This is now an
  explicit runtime/spec contract: code must not rely on string literal allocation
  freshness, and low-level identity-sensitive APIs may observe the pooled object.
  Independently valuable: shrinks `.rodata` for any repeated-literal program.
- **Other constantizable ref types**: emit **one global per literal site** (no
  cross-site pooling). This keeps per-site behaviour identical to today even if
  some future identity channel (e.g. an identity-based default `hash()`) were
  ever added, so constantization stays unconditionally safe while dedup remains a
  String-scoped optimization.

## Runtime change — recognize immortal headers (ref-type tier only)

The byte payload is already correctly placed in `.rodata`
(`crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs:56-72`,
`set_constant(true)`). The wrapper is the problem.

### The `ScoopGcObjectHeader` write hazard

`runtime/c/scoop_gc.h:210-244` — the header has mutable `flags` / `mark`, and
the marker writes `mark` unconditionally. Immix has both a serial marker
(`runtime/c/scoop_gc_backend_immix.c:2719-2737`) and a parallel marker
(`:2950-2975`) that updates `obj->mark` with an atomic CAS. The baseline,
minimal, and hosted backends have their own marker helpers
(`runtime/c/scoop_gc.c:1313-1323`,
`runtime/c/scoop_gc_backend_minimal.c:503-513`,
`runtime/c/scoop_gc_backend_hosted.c:513-523`). A naive `.rodata` copy would
fault on the first GC cycle that traces it, because the `mark` write hits a
read-only page.

### The keystone: heap-membership already filters off-heap pointers

Slot scanning already drops non-heap pointers via the membership index
(`runtime/c/scoop_gc_backend_immix.c:2739-2760`). Immortal globals live entirely
outside immix blocks, so the existing `scoop_gc_mark_visitor` treats them as
transparent with **no change**. Recursive transparency holds: an immortal
`ScoopString`'s `data` points at an off-heap `.rodata` byte array (skipped), and
a value-type aggregate's fields point at immortal `ScoopString`s (skipped).

### The one required change

Add two sentinels to `runtime/c/scoop_gc.h`:

```c
#define SCOOP_GC_FLAG_IMMORTAL 0x80000000u
#define SCOOP_GC_MARK_IMMORTAL 0xFFFFFFFFu
```

and one defensive short-circuit in each marker helper before any `mark` read or
write. This includes Immix serial/parallel markers and the baseline,
minimal, and hosted marker helpers. Immix pinned-object and stable-handle
scanning can reach the marker without a visitor membership pre-check in both
parallel (`:5043-5060`) and serial (`:5169-5186`) paths.

```c
static void scoop_gc_mark_object_if_needed(ScoopGcMarkCtx *ctx,
                                           ScoopGcObjectHeader *obj) {
    if (ctx == 0 || obj == 0) return;
    if ((obj->flags & SCOOP_GC_FLAG_IMMORTAL) != 0) return;  // NEW
    if (obj->mark == ctx->mark_value) return;
    obj->mark = ctx->mark_value;
    // ... unchanged ...
}
```

This is the only required runtime behavior change. Immix slot scanning already
filters immortals via membership; pinning APIs only accept previously
heap-allocated objects (user code cannot pin an immortal), but the marker
iterates pinned records directly, so the flag check covers any path that pushes
an object onto the mark stack without a membership pre-check. Sweep iterates
`heap->objects`; immortals are never threaded onto that list (`next = null`), so
sweep cannot see them.

This change is inert for the value-type tier — those objects never reach the
marker at all.

## Consumers (recast)

The three original "customers" are now consumers of the one mechanism, not
bespoke paths:

- **String literals** — `codegen_string_literal_from_bytes`
  (`crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:133-197`) routes
  through `try_emit_immortal` instead of `scoop_alloc_typed`. No allocation, no
  GC-local-root tracking, no deferred-spill dance.
- **`TypeMetadataLiteral::TypeNameString`**
  (`crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:186-201`)
  — no bespoke change; already routes through the string-literal path. Audit
  that no consumer mutates the result (the MIR contract says it is immutable).
- **`Platform`** — lower the `TypedIntrinsicKind::Platform` intrinsic in
  HIR→MIR to a plain `Rvalue::StructLit` of five `SynthString` fields. The
  generic folder then picks it up automatically. The bespoke
  `codegen_platform_literal`
  (`crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:203-292`)
  and any `get_or_create_immortal_platform_global` helper are **deleted** — there
  is no Platform-specific code left.

### Cache and dedup keys

Replace `__scoop_str_data_{span.start}_{span.end}` with a content hash
(`crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs:74-89`). Wrapper
globals key on `(type_desc / aggregate_ty, content_hash)`. Add
`set_unnamed_addr(true)` to both the byte-array and wrapper globals.

### Globals already correctly placed (no change)

Type descriptors / trace tables (`codegen/gc.rs:1018,1092,1330,1373,1406,1442,
1482,1637`), composite/value-erasure descriptors
(`main/composite_transport.rs:609,637`), frame/effect ABI tables
(`main/frame.rs:251,267`, `main/effect_lowered/layout/abi.rs:508,519`), and the
string byte arrays (`main/alloca.rs:70`) are already `set_constant(true)`.

## Phasing

1. **`__AtomicInt` → `@InteriorMutable struct`.** Introduce the `@InteriorMutable`
   attribute, promote the typealias, update the five erasure sites and the two
   atomic constructors. Standalone; existing atomic tests must pass unchanged.
2. **Runtime: `SCOOP_GC_FLAG_IMMORTAL` + marker short-circuit.** Land standalone
   with a C unit test (synthetic immortal header is not written; a heap header in
   the same test still is). Run under ASan.
3. **Codegen: content-hash keying for `get_or_create_global_bytes`** +
   `set_unnamed_addr(true)`. Golden-file diffs only, no behaviour change.
4. **Codegen: `is_immutable(T)` predicate + `try_emit_immortal` folder**, routing
   `codegen_string_literal_from_bytes` and the `StructLit` / `MakeTuple` arms
   through it. Largest change. New tests assert zero `scoop_alloc_typed` for a
   literal-only function, and that two `"hello"` literals share one global.
5. **`Platform` → MIR `StructLit`**; delete `codegen_platform_literal`. Assert
   `Platform` access emits zero `scoop_alloc_typed`.
6. **`TypeMetadataLiteral` audit**: confirm immutability; assert two
   `__type_name(T)` reads are pointer-equal (now possible via dedup).

Phases 1–3 are independent. Phase 4 unblocks 5 and 6.

## Test plan

**Unit (runtime, C):**
- Mark a synthetic immortal header — `mark` / `flags` byte-identical before and
  after (under ASan). Mark a heap header in the same test — `mark` updated.
  Confirms the short-circuit is flag-gated, not blanket.

**Unit (codegen, Rust):**
- `is_immutable`: `String` and a synthetic all-`val` class are immutable;
  `RefCell` / `AtomicInt` (`var` field) and `__AtomicInt` (`@InteriorMutable`)
  are not.
- `codegen_string_literal_from_bytes("hello")` emits zero `scoop_alloc_typed`.
- Two `"hello"` in one function reference the same global.
- `Platform` access emits zero `scoop_alloc_typed`.

**Integration:**
- A literal printed in a 10M-iteration loop shows zero growth in
  `bytes_allocated` after the first GC cycle (modulo the print path itself).
- `Platform.os` read 10M times does not trip the GC.

**Existing tests:** the full LLVM codegen suite passes unchanged. String
semantics are identical from the user's perspective; pointer equality is now
stronger (literals dedup), which only affects tests asserting non-equality of
pointers from distinct spans — none should exist.

## Out of scope (future work)

- **Liftable aggregates referenced via a `Local`** (nested constant aggregates).
  This version requires direct `Operand::Const` fields; chasing `Local` →
  defining `StructLit` is a follow-up.
- **`EnumVariant` constants** and any aggregate needing non-trivial transport
  (boxing / value-erasure). These fall back to the dynamic path by gate
  construction.
- **General `val const-eval` lifting** (`val PI = 3.14`, `1 + 2`). This is the
  *same* mechanism with an extra front-end entry (feed a const-evaluated module
  value into `try_emit_immortal`); only the const-evaluator is missing, not the
  lifting machinery.
- **Cross-type dedup beyond String.** Pooling other immutable ref types is held
  back pending a full identity-sensitive API review; see the dedup section.
- **Embedded tier hints** (`.rodata.fast` / `.rodata.psram` section splits) —
  target-specific, left to the embedded-port design doc.
- **Cross-archive (`.cone`) literal dedup** — `unnamed_addr` folds within a final
  image; sharing across separately compiled archives needs weak linkage or a
  content-hash mangling scheme. Separate design.
