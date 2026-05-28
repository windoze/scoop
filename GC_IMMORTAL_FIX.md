# GC Immortal Objects: Lift Compile-Time-Known GC Values into `.rodata`

Status: P0 design. Platform-agnostic — applies to every codegen target. Embedded
tier hints (PSRAM placement, sectioning) are deliberately deferred.

## Motivation

Several values that are **fully known at compile time** are currently allocated
on the GC heap on every use:

1. **String literals** — every `"hello"` in source emits a fresh
   `scoop_alloc_typed` call that materializes a `ScoopString { hdr, len, data }`.
   The byte payload itself is already in `.rodata` (good), but the wrapper
   object is heap-allocated each time the literal expression is evaluated.
2. **Type-name metadata literals** (`TypeMetadataLiteral::TypeNameString`) —
   delegate to the same string-literal path, inheriting the wrapper allocation.
3. **`scoop.core.Platform` literal** — every read of `Platform` emits
   **five** `ScoopString` allocations (one per field) plus an SSA struct insert
   sequence. The struct value is constant for the life of the binary.

Concrete cost on a representative sample:

- `print("hello world")` inside a hot loop allocates a `ScoopString` on every
  iteration. The mark-region GC tolerates this (pinned/scoped allocators help)
  but it is unjustified pressure on the nursery and the marker.
- `Platform.os` in any `match` chain allocates 5 strings just to read 1 field.
- A program that prints `__type_name(T)` once per element of a list pays the
  full alloc per element.

Beyond pressure, this also breaks the user's mental model: `"hello"` is
indistinguishable from `String("hello")` at the IR level, even though the former
is a textbook immortal compile-time constant.

## Current behavior (verified)

### String literal lowering

`crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:133-197`:

```rust
pub(in crate::llvm::codegen) fn codegen_string_literal_from_bytes(
    &mut self,
    span: crate::span::Span,
    bytes: &[u8],
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    // 1) Allocate a GC-managed `ScoopString` object via scoop_alloc_typed.
    let scoop_str_ty = self.llvm_scoop_string_type();
    let obj_size = self.target_data.get_store_size(&scoop_str_ty);
    let size_v = self.context.i64_type().const_int(obj_size, false);
    let str_desc = self.get_or_create_string_type_desc_global(span)?;
    // ... pointer cast ...
    let rt_alloc = self.declare_runtime_alloc_typed();
    let call = self.build_call_preserving_gc_local_roots(
        span, rt_alloc,
        &[str_desc_i8.into(), size_v.into()],
        "rt_alloc_string_lit",
    )?;
    // ... cast result to ScoopString* ...

    // 2) Store { len, data } into the freshly allocated object.
    let len_ptr = self.builder.build_struct_gep(scoop_str_ty, str_ptr, 1, ...)?;
    let data_ptr = self.builder.build_struct_gep(scoop_str_ty, str_ptr, 2, ...)?;
    let len = self.context.i64_type().const_int(bytes.len() as u64, false);
    let _ = self.builder.build_store(len_ptr, len)?;
    // ... stores data_gv pointer (which IS in .rodata, see below) ...
}
```

The byte payload is correctly placed in `.rodata`:

`crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs:56-72`:

```rust
pub(in crate::llvm::codegen) fn get_or_create_global_bytes(
    &self,
    span: crate::span::Span,
    bytes: &[u8],
) -> GlobalValue<'ctx> {
    let name = format!("__scoop_str_data_{}_{}", span.start, span.end);
    if let Some(existing) = self.module.get_global(&name) {
        return existing;
    }
    let arr_ty = self.context.i8_type().array_type(bytes.len() as u32);
    let gv = self.module.add_global(arr_ty, None, &name);
    let init = self.context.const_string(bytes, false);
    gv.set_initializer(&init);
    gv.set_constant(true);
    gv
}
```

Two issues with this helper independently of the wrapper bug:

- The cache key is the source span, not a content hash. The same literal `"x"`
  appearing in three places creates three separate `.rodata` arrays.
- `set_unnamed_addr(true)` is not set, so the linker cannot dedup across
  translation units.

### TypeMetadataLiteral lowering

`crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:186-201`:

```rust
pub(in crate::llvm::codegen) fn codegen_mir_type_metadata_literal(
    &mut self,
    span: crate::span::Span,
    metadata: &crate::mir::TypeMetadataLiteral,
    mir_types: &TypeStore,
) -> Result<CgValue<'ctx>, LlvmEmitError> {
    match metadata.kind {
        crate::mir::TypeMetadataLiteralKind::TypeNameString => {
            let type_name = metadata
                .source_fqn
                .clone()
                .unwrap_or_else(|| mir_types.display(metadata.source_ty).to_string());
            self.codegen_string_literal_from_text(span, &type_name)
        }
    }
}
```

Inherits the wrapper-alloc bug end-to-end.

### Platform literal lowering

`crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:203-292`
loops over five fields (`triple`, `arch`, `vendor`, `os`, `env`) and calls
`codegen_string_literal_from_text` for each, defers them as GC-sensitive
values, then materializes them back as inserts into an SSA aggregate. Five
heap allocations per `Platform` read.

### `ScoopGcObjectHeader` mutability — the design wrinkle

`runtime/c/scoop_gc.h:210-244`:

```c
typedef struct ScoopGcObjectHeader {
  struct ScoopGcObjectHeader *next;     // mutable: heap-list link
  const ScoopTypeDescriptor *type_desc; // effectively const after alloc
  uint64_t size_bytes;                  // effectively const after alloc
  uint32_t flags;                       // mutable: GC state bits
  uint32_t mark;                        // mutable: mark color
} ScoopGcObjectHeader;
```

The marker writes to `mark` unconditionally:

`runtime/c/scoop_gc_backend_immix.c:2719-2737`:

```c
static void scoop_gc_mark_object_if_needed(ScoopGcMarkCtx *ctx,
                                           ScoopGcObjectHeader *obj) {
    if (ctx == 0 || obj == 0) return;
    if (obj->mark == ctx->mark_value) return;
    obj->mark = ctx->mark_value;            // <-- write
    ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
    if (block != 0) {
        // ... region mark bitmap update ...
    }
    scoop_gc_mark_stack_push(ctx->stack, obj);
}
```

**A naive `.rodata` copy of a heap-allocated `ScoopString` would fault on the
first GC cycle that traces it**, because the `mark` write hits a read-only
page. So `.rodata` placement alone is not enough — the GC must also be taught
to recognise these objects and skip the mark write.

Fortunately there is already a natural choke point. Slot scanning in the mark
visitor already filters out non-heap pointers via the heap membership index:

`runtime/c/scoop_gc_backend_immix.c:2739-2760`:

```c
static void scoop_gc_mark_visitor(void **slot, void *raw_ctx) {
    // ...
    ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw;
    if (!scoop_gc_heap_membership_index_contains(ctx->membership,
                                                 ctx->heap, obj)) {
        return;                              // <-- silently skipped
    }
    scoop_gc_mark_object_if_needed(ctx, obj);
}
```

Any pointer that does not fall inside an immix block (the magic-checked region)
is already silently skipped. The membership check is the keystone we will lean
on: as long as immortal objects live entirely outside immix blocks, the
existing marker treats them as transparent — no header writes, no traversal,
no scan.

The remaining question is whether the **payload** of an immortal object
contains any GC-pointer fields that themselves need tracing. For the three
customers in this fix:

- `ScoopString` payload is `{ uint64_t len; const uint8_t *data; }` — no
  managed references. The `data` pointer targets a `.rodata` byte array,
  which is itself off-heap and would also be skipped by the membership check.
- `Platform` is a struct of five `ScoopString` references. Each reference
  would point at another immortal `ScoopString` — also skipped by the same
  rule. Recursive transparency.

So for this fix, no bespoke immortal-trace path is required. We only need
**all immortal references to remain off-heap**. Future immortal types that
embed managed references back into the heap will need a more careful trace
strategy; see "Out of scope".

### Other globals already correctly placed

For reference, these are already `set_constant(true)` and need no change:

- Type descriptors and trace tables: `scoopc_codegen_llvm/src/llvm/codegen/gc.rs`
  lines 1018, 1092, 1330, 1373, 1406, 1442, 1482, 1637.
- Composite/value-erasure descriptors:
  `crates/scoopc_codegen_llvm/src/llvm/codegen/main/composite_transport.rs:609,637`.
- Frame/effect ABI tables:
  `crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:251,267`,
  `crates/scoopc_codegen_llvm/src/llvm/codegen/main/effect_lowered/layout/abi.rs:508,519`.
- String byte arrays themselves: `main/alloca.rs:70`.

These are not affected by this design.

## Proposed design

### Naming

A GC value is **immortal** when it lives outside any immix block, has the
same lifetime as the binary, and has a payload whose every managed reference
is itself immortal. Immortal values may be placed by codegen into `.rodata`
via `set_constant(true)` and are transparent to the GC.

### LLVM emission shape for an immortal `ScoopString`

A single module-level constant aggregate, replacing the per-use heap path:

```llvm
@__scoop_str_lit_<hash> = private constant
  %ScoopString {
    %ScoopGcObjectHeader {
      ; next:        const-null     -- never threaded onto the heap list
      ptr null,
      ; type_desc:   string desc    -- already a constant global
      ptr @__scoop_string_type_desc,
      ; size_bytes:  sizeof(ScoopString)
      i64 <store_size>,
      ; flags:       SCOOP_GC_FLAG_IMMORTAL  (new bit; see runtime change)
      i32 <IMMORTAL_FLAG>,
      ; mark:        SCOOP_GC_MARK_IMMORTAL  (new sentinel; see runtime change)
      i32 <IMMORTAL_MARK>
    },
    i64 <byte_count>,
    ptr @__scoop_str_data_<hash>     ; existing byte array, also constant
  }, unnamed_addr
```

The header fields `flags` and `mark` are populated with **sentinel values
recognized by the runtime**. They are still readable mutable fields in C terms,
but the runtime never writes to them on immortal objects, so `.rodata`
placement is sound (see "Runtime change" below).

Codegen invariants:
- Cache key is `(type_desc, content_hash)`, not span.
- `set_constant(true)` and `set_unnamed_addr(true)` so the linker can
  fold equivalent literals across translation units.
- Internal linkage; let the linker promote/dedup as needed.

### Runtime change — recognize immortal headers

Two minimal additions to `runtime/c/scoop_gc.h`:

```c
#define SCOOP_GC_FLAG_IMMORTAL 0x80000000u
#define SCOOP_GC_MARK_IMMORTAL 0xFFFFFFFFu
```

Plumbing — exactly **one** call-site needs to be defensive: the mark stack
entry point. The existing slot-scan path
(`scoop_gc_mark_visitor`) already filters via heap membership and is correct
for immortals as written. The remaining hardening is `scoop_gc_mark_object_if_needed`,
which is also reachable from pinned-object scanning (lines 5177/5185):

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

This is the **only required runtime change**. Rationale:

- Slot scanning in `scoop_gc_mark_visitor` already drops off-heap pointers
  via the membership index — the immortal global is off-heap by construction,
  so this path needs no change. We could add the flag check there too as a
  belt-and-braces measure, but it is redundant.
- Pinning APIs (`scoop_gc_pin_object`) only accept previously allocated heap
  objects; user code cannot pin an immortal. But the marker iterates pinned
  records directly without going through the membership index, so the flag
  check in `mark_object_if_needed` covers any future surface that pushes an
  object onto the mark stack without a membership pre-check.
- Sweep iterates `heap->objects` (the linked list rooted at heap state).
  Immortals are never threaded onto that list (`next = null` from codegen),
  so sweep cannot see them. No change needed.

### Customer 1 — String literals

Replace the per-use `scoop_alloc_typed` path in
`codegen_string_literal_from_bytes` with `get_or_create_immortal_string_global(bytes)`,
which:

1. Hashes `bytes`, looks up `__scoop_str_lit_<hash>` in the module; returns
   if it exists.
2. Otherwise emits the constant aggregate above, referencing the existing
   `__scoop_str_data_<hash>` global (which we re-key to content hash too,
   so the same payload across spans collapses).
3. Returns the global as a `CgValue { ty: CgTy::String, value: <ptr> }`.

The wrapper is now a compile-time constant. No allocation, no GC pressure,
no GC-local-root tracking, no deferred-spill dance. The IR shrinks
significantly at every literal use.

### Customer 2 — `TypeMetadataLiteral::TypeNameString`

No bespoke change. It already routes through `codegen_string_literal_from_text`,
which inherits the new immortal path automatically.

Add an audit pass over MIR `TypeMetadataLiteral` consumers: confirm no
caller mutates the result. (The MIR contract already states these are
immutable but it's worth a check.)

### Customer 3 — `Platform` literal

Convert to a single module-level constant:

```llvm
@__scoop_platform_literal = private constant
  %ScoopCorePlatform {
    ptr @__scoop_str_lit_<hash_of_triple>,
    ptr @__scoop_str_lit_<hash_of_arch>,
    ptr @__scoop_str_lit_<hash_of_vendor>,
    ptr @__scoop_str_lit_<hash_of_os>,
    ptr @__scoop_str_lit_<hash_of_env>
  }, unnamed_addr
```

Each field is a pointer to an immortal `ScoopString` from Customer 1.
`codegen_platform_literal` becomes:

```rust
fn codegen_platform_literal(&mut self, span, target_cg) -> Result<CgValue, _> {
    let global = self.get_or_create_immortal_platform_global(span)?;
    Ok(CgValue { ty: target_cg, value: Some(load_struct_value(global)) })
}
```

The struct value is loaded once from a constant global. Every prior
`build_insert_value` / `defer_gc_sensitive_cg_value` /
`build_call(rt_alloc_typed, ...)` per field disappears. Five allocations
per use → zero.

### Cache and dedup

Replace `__scoop_str_data_{span.start}_{span.end}` with
`__scoop_str_data_<base16(SHA-256(bytes)[..16])>`. Keep the helper internal
to `alloca.rs`; callers are unaffected. Add `set_unnamed_addr(true)` to
both the byte array global and the new wrapper global so the linker can
fold across crates.

This is independently valuable — it reduces `.rodata` for any program with
repeated literals — and it gives Customer 1 / Customer 3 stable lookup keys.

## Phasing

1. **Runtime: introduce `SCOOP_GC_FLAG_IMMORTAL`** and the marker
   short-circuit. Land standalone, with a unit test that constructs a
   stack-allocated `ScoopGcObjectHeader` with the flag set, pushes it
   onto a mark stack indirectly, and asserts `mark` is not written.

2. **Codegen: switch `get_or_create_global_bytes` to content-hash keying**
   and add `set_unnamed_addr(true)`. Land standalone with golden-file
   diffs only — no behaviour change. Confirms the renaming/dedup story
   works before any wrapper changes.

3. **Codegen: emit immortal `ScoopString` wrappers** and route
   `codegen_string_literal_from_bytes` through them. This is the largest
   change. Existing tests should still pass; new tests assert that no
   `scoop_alloc_typed` call appears in the IR for a function whose only
   string activity is literals.

4. **Codegen: collapse `Platform` literal** to a single immortal global.

5. **Audit pass over `TypeMetadataLiteral` consumers** — confirm immutability,
   add explicit test that two `__type_name(T)` reads return pointer-equal
   `ScoopString` (now possible because of the dedup).

6. **Annotation metadata audit** — annotations are compile-time markers,
   not runtime nominal types (confirmed in
   `sysroot/lib/scoop.core/src/core.scoop:420-467`). If any runtime
   annotation surface is added later, it should land directly on the
   immortal path; flag this in the design review.

Phases 1–2 are independent. Phase 3 unblocks 4 and 5. Phase 6 is forward-looking.

## Test plan

**Unit (runtime, C):**
- Mark a synthetic immortal header — assert `mark` and `flags` are
  byte-identical before and after. Run under ASan to catch any stray
  writes.
- Mark a heap header in the same test — assert `mark` is updated. Confirms
  the short-circuit is gated on the flag, not blanket.

**Unit (codegen, Rust):**
- Assert `codegen_string_literal_from_bytes("hello")` emits **zero**
  `scoop_alloc_typed` calls in the resulting IR.
- Assert two `"hello"` literals in the same function reference the same
  global (post content-hash keying).
- Assert `Platform` access emits **zero** `scoop_alloc_typed` calls.

**Integration:**
- A program that prints a literal in a tight 10M-iteration loop should
  show zero growth in `bytes_allocated` after the first GC cycle (only
  whatever the print path itself allocates, which is unrelated).
- A program that reads `Platform.os` 10M times should not trip the GC.

**Existing tests:**
- The full LLVM codegen suite must pass unchanged. String semantics are
  identical from the user's perspective — the wrapper is still a
  `String`, still has `.length`, still compares correctly. Pointer
  equality is now stronger than before (literals dedup), which only
  affects tests that explicitly assert non-equality of pointers from
  distinct spans — none should exist.

## Out of scope (future work)

These come up naturally during the discussion but are deliberately
deferred to keep this fix focused and platform-agnostic.

- **Immortal aggregates with managed-reference fields whose targets are
  themselves heap-allocated.** Today no such case exists. If a future
  type wants this, it needs either a custom trace path or a static
  guarantee that the target is also immortal.
- **`val const-eval` lifting.** A `val PI = 3.14` at module scope is
  conceptually immortal but is not handled here because it requires a
  general-purpose const-evaluator on the right-hand side.
- **Annotation runtime reflection.** If we ever expose annotation
  classes to user code at runtime, those metadata structures should be
  emitted on the immortal path from day one.
- **Embedded tier hints.** On targets with multiple memory tiers
  (ESP32-class SRAM/PSRAM, MCU flash/RAM split), immortal globals are
  the natural candidates for an explicit `.rodata.fast` /
  `.rodata.psram` section split. That decision is target-specific and
  is left to the embedded-port design doc.
- **Cross-archive (`.cone`) literal dedup.** The `unnamed_addr`
  attribute lets the linker dedup within a final image. Sharing across
  separately compiled `.cone` archives would require either weak
  linkage or a name-mangling scheme keyed on content hash; either is a
  separate design.
