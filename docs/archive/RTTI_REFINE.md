# RTTI_REFINE

A working document for two related implementation refinements:

1. **R1** — Replace the O(depth) parent-chain walk and the linear `itable`
   scan with O(1)/O(log n) IS-A checks driven by a precomputed
   `TypeDescriptor` layout.
2. **R2** — Put the LLVM `stackmap` / `statepoint` codegen+runtime path
   behind a Cargo feature, since the GC+stackmap pipeline is currently
   broken and Scoop's panic/Raise paths do not depend on it.

These are **implementation/ABI refinements**, not language-spec changes; they
are kept out of `SPEC_FIX.md` to avoid mixing "spec wording" items with
"runtime ABI bump" items.

Status: **pending** — neither item has been applied yet.

---

## R1. RTTI: O(1) IS-A via Cohen's display + flat `implements` table

### Current behavior (verified)

**Class IS-A class** —
`crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:380-475`
(`codegen_type_desc_chain_contains_target`):

```c
while (cur != NULL) {
    if (cur == target) return true;
    cur = cur->parent_type_desc;
}
return false;
```

O(depth), one dependent load per ancestor. Single-inheritance class chain so
correctness is fine; performance is not.

**Class IS-A interface** —
`crates/scoopc_codegen_llvm/src/llvm/codegen/main/numeric.rs:9-200`
(`codegen_itable_contains_runtime_type_id`): nested linear scan over
`itable.entries × entry.runtime_match_type_ids[]`. The `itable` exists
primarily for **method dispatch** (one entry per method group); the
`runtime_match_type_ids[]` array piggy-backs IS-A onto the dispatch table.
Worst case `O(num_method_groups × max_super_interface_count)`.

`TypeDescriptor` layout today
(`crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs:856-898`): 14 fields,
single `parent_type_desc` pointer, one `itable*`, one `vtable*`, an
`abi_version` field already present.

### Proposed layout change

Add **Cohen's display** for the class chain and **split** the IS-A path off
the method-dispatch path:

```rust
struct TypeDescriptor {
    abi_version: u16,             // bumped on layout change
    flags: u16,
    size_bytes: u32,
    align_bytes: u32,
    type_id: u32,
    depth: u16,                   // class inheritance depth; Any/Object = 0
    implements_count: u16,        // length of `implements_type_ids` tail
    parent_type_desc: *const Self,// retained for compatibility / debug
    vtable: *const InterfaceVTable, // method dispatch only
    trace_*: ...,                 // existing trace fields

    // --- variable-length tail, laid out by codegen ---
    // parent_chain: [*const TypeDescriptor; depth]
    //   parent_chain[i] = the unique class ancestor at depth i
    // implements_type_ids: [u32; implements_count]
    //   sorted ascending; full transitive closure of implemented interfaces
}
```

### Why this works under cone-separated compilation

- **Depth is locally derivable.** When emitting the descriptor for `class C
  : B`, the compiler already loads `B`'s descriptor; copy `B.parent_chain`
  and append `&B`. No global numbering required. This is the property that
  rules out Schubert numbering (which needs whole-program DFS) but admits
  Cohen's display.

- **`implements_type_ids` closure is locally derivable.** The transitive
  closure of implemented interfaces is already computed today by
  `crates/scoopc_hir/src/itable.rs:777-836` (`compute_class_interface_closure`
  + `collect_interface_and_supers`). Today it feeds the `runtime_match`
  array per method-group; we just emit it once, sorted, as a top-level
  array.

### IS-A check after the change

```c
bool is_a_class(obj_desc, target_desc) {
    if (obj_desc->depth < target_desc->depth) return false;
    if (obj_desc == target_desc) return true;
    return obj_desc->parent_chain[target_desc->depth] == target_desc;
}

bool is_a_interface(obj_desc, target_type_id) {
    // binary search; implements_count is small (typical 3-15) so a small
    // unrolled linear scan is also acceptable.
    return binary_search(obj_desc->implements_type_ids,
                         obj_desc->implements_count,
                         target_type_id);
}
```

Both paths: two loads + one or two compares for the class case; logarithmic
small-N for the interface case.

### Space cost

Per `TypeDescriptor`: `8 * depth + 4 * implements_count + 4` extra bytes.
Empirically `depth ≤ 5` and `implements_count ≤ 10` for typical Scoop
classes — about 60 bytes per descriptor. Acceptable.

### What we do *not* do

- **Schubert numbering** — needs whole-program DFS; incompatible with cone
  separate compilation.
- **Global interface bitset** — needs stable cross-cone slot assignment;
  requires either a registry or whole-program link, both of which add
  surface we don't want.
- **Inline caching at IS-A sites** — useful in JITs; AOT version (e.g. last
  hit type_id) is possible but premature.

### ABI considerations

- Bump `TypeDescriptor::abi_version`. Loader at runtime should reject
  artifacts produced against a mismatched version.
- The variable-length tail means each concrete class's descriptor is its own
  LLVM struct type; access is via base pointer + offset arithmetic. This is
  standard but has codegen-side complexity.
- All existing fixtures that snapshot `TypeDescriptor` bytes will need to be
  regenerated.

### Implementation outline

1. Extend `TypeDescriptor` emission in `gc.rs` with the variable-length
   tail; update `abi_version`.
2. Replace the chain walk in `expr_op.rs:380-475` with the Cohen check.
3. Replace the itable scan in `numeric.rs:9-200` with the binary search
   over `implements_type_ids`.
4. Stop populating `runtime_match_type_ids` in itable entries (those
   semantics now live on `TypeDescriptor::implements_type_ids`); itable
   becomes purely method-dispatch.
5. Regenerate any `dump-rtti` / RTTI snapshot fixtures.
6. (Optional) Add a fixture for the multi-level grandparent IS-A and a
   static-typed interface variable IS-A super-interface, neither of which
   is currently covered.

### Open questions

- Whether to keep `parent_type_desc` at all once `parent_chain` exists.
  Recommend keeping it for `dump-rtti` readability and `release_callback`
  parent traversal — cost is one pointer.
- Whether to also fold class descriptors into the `implements` array (so
  IS-A class becomes a single `implements`-style query). Recommend **no**:
  class IDs and interface IDs share the type_id space, but Cohen's check is
  strictly cheaper than binary search and fits class semantics naturally.

---

## R2. Feature-gate the LLVM `stackmap` / `statepoint` path

### Why this item exists

The stackmap+GC pipeline is paused, not abandoned. The blocking design
question is **register roots**: precise GC under LLVM lowering must track
GC pointers that the register allocator may park in registers across a
safepoint, and there is no portable way to do that — it requires
arch-specific register classes, OS-specific ucontext layouts, and a
`gc.statepoint` lowering tuned to each backend's calling convention.

Until register-root handling is designed, the project's stance is:

- Keep Scoop's user-facing surface and the runtime ABI **maximally
  portable**; do not let platform-specific code creep in via the
  stackmap path's transitive dependencies (`platform/unwind*`,
  arch-specific stack-walking, `ntdll`, `libdl`).
- The default managed-roots mechanism is the **explicit frame chain**
  (TLS-linked shadow stack via `runtime/c/scoop_root_frame.h`), which is
  fully portable and adequate for current Scoop semantics.
- Preserve the stackmap codepath as **gated, not deleted**, so the
  scaffolding is available when the register-root design lands.

The gate is the freezer: stop paying maintenance cost now, keep the
artifact intact for when the design question is answered.

### Current state (verified)

**The stackmap roots path is broken in practice.** Evidence:

- `crates/scoopc_codegen_llvm/src/llvm/codegen/gc.rs:72-76` explicitly
  notes that the default mode no longer attaches `gc "statepoint-example"`
  to managed functions; only the test helper
  `__scoop_stackmap_statepoint_smoke` opts back in.
- `crates/scoopc_codegen_llvm/src/llvm/pipeline.rs:68` still runs
  `rewrite-statepoints-for-gc` in the pass pipeline, but with no functions
  carrying the GC strategy attribute the pass is effectively a no-op.
- `runtime/c/scoop_gc.c` retains both root-enumeration paths
  (`scoop_gc_managed_root_map_from_explicit_frame_top` and
  `scoop_gc_managed_root_map_from_stackmap_ctx`); production traffic uses
  the **explicit frame chain** (TLS shadow stack via
  `runtime/c/scoop_root_frame.h`).
- The whole `runtime/c/platform/unwind*.c` layer exists primarily to feed
  stackmap roots; it is never used to implement language-level unwinding.

**Panic and Raise do not flow through stackmap or unwind.** Evidence:

- `runtime/c/scoop_runtime.c:845-860`:
  ```c
  void scoop_panic(const void *message)              { (void)message; exit(3); }
  void scoop_runtime_error_fatal(const void *err)    { (void)err;     exit(3); }
  ```
  Both are `exit(3)`. There is no `_Unwind_Raise`, `__cxa_throw`, or
  equivalent.
- Raise effect is lowered as **EffectOutcome** — every effectful function
  returns a sum-type value, propagation is explicit at each call site
  (`crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/effect_outcome.rs:13-79`).
  No `invoke`/`landingpad` involvement.

So the stackmap codepath is paying a maintenance cost (parser, runtime
fallback, platform unwind backends, fixtures) for a use case that nothing
in the language semantics requires.

### Proposal

Introduce a Cargo feature **`llvm-statepoint`** (off by default) and put
all stackmap/statepoint-specific code behind it.

**Scope of the gate** (best effort — confirm during implementation):

| Layer | File(s) | Action |
|---|---|---|
| codegen pipeline | `crates/scoopc_codegen_llvm/src/llvm/pipeline.rs:68` | drop `rewrite-statepoints-for-gc` from the default pipeline |
| codegen — stackmap parser | `crates/scoopc_codegen_llvm/src/stackmap.rs` & `llvm/stackmap.rs` | gate behind `llvm-statepoint` |
| codegen — smoke helper | `gc.rs:62-91` (`__scoop_stackmap_statepoint_smoke`) | gate, plus its declaration in `runtime_abi.rs` |
| CLI surface | `crates/scoop/src/cli.rs:108-123` (`DumpStackmaps`, `--verify-roots`, `--dump-records`) | gate the subcommand |
| CLI dispatch | wherever `Command::DumpStackmaps` is routed | gate |
| runtime — parser | `runtime/c/scoop_stackmap.c` / `.h` | wrap in `#ifdef SCOOP_LLVM_STATEPOINT` (or new feature flag passed by build script) |
| runtime — gc fallback | `runtime/c/scoop_gc.c:60-90, 268-282, 1690-2030` (stackmap fallback branches) | gate; the explicit frame chain becomes the single managed-roots source |
| runtime — gc backends | same in `scoop_gc_backend_immix.c`, `scoop_gc_backend_minimal.c`, `scoop_gc_backend_hosted.c` | gate |
| runtime — platform unwind | `runtime/c/platform/unwind*.{h,c}`, `build.rs` linkage to `ntdll`/`libdl` | gate; explicit frames don't need it |
| tests | `crates/scoop_runtime/tests/{stackmap_*,gc_stackmap_*,gc_stack_walking_unwind,unwind_capture,gc_verify_roots}.rs` | gate behind `#[cfg(feature = "llvm-statepoint")]` or move to a separate test crate |

### Broader changes flagged during the audit

These came up while tracing the gate boundary; documenting so the gate PR
doesn't have to rediscover them:

1. **`abi_exports_allowlist.rs` may shrink.** If
   `scoop_stackmap_registry_*` symbols leave the runtime ABI, the
   allowlist (T1401) needs to drop them — otherwise the gating won't pass
   the symbol-export regression. Confirm during implementation.

2. **`runtime/c/platform/unwind*` becomes unreachable** under the gated
   build. The `Cargo.toml` build script's `cargo:rustc-link-lib=ntdll`
   (Windows) and the Linux `libdl` link become conditional. The unwind
   layer itself is internal-linkage `static` so removing the include site
   is sufficient.

3. **Mention in `SCOOP_RUNTIME.md`** of stackmap/statepoint as the v0
   target (T1503/T1505/T1506 references) needs to be revisited — the
   document should reflect that explicit frames are the **default and
   sole** managed-roots mechanism, with statepoint as an
   experimental/future option.

4. **MIR-side traces of `unwind` terminology.** A grep over `crates/`
   shows ~20 files mentioning "unwind"; most are unrelated (effect
   lowering's "unwind" of a `with` chain), but the LIR/MIR emission paths
   (`scoopc_lir/src/effect_lowered/segment.rs`,
   `scoopc_mir/src/mir/lower/fn_lowering_basic.rs`) should be checked for
   any **actual** dependency on LLVM unwind tables. Initial read: no such
   dependency — the term is overloaded.

5. **No language-spec impact.** `SCOOP_FULL_SPEC.md` does not promise
   stack-walking-based exception semantics. The current spec's only
   non-local control transfer is Raise effect (handled lexically) and
   `panic` (abort). Gating stackmap is a pure implementation cleanup.

6. **Moving GC interaction.** `scoop_gc.c:2023-2030` notes that "moving GC
   under stackmap mode requires statepoint stackmaps". With the gate
   off, moving GC must rely solely on explicit-frame slot updates.
   Confirm `gc-immix` (the default backend) under explicit-frames-only
   still passes the compaction stress fixtures, or document that moving
   compaction also moves behind `llvm-statepoint`.

### Why a feature flag rather than deletion

- The decision to revive this path is gated on the register-root design
  (see "Why this item exists" above). Until that design lands the work
  cannot continue, but discarding the scaffolding would force a
  ground-up rebuild later — including the parser, the smoke helper that
  proves the LLVM toolchain still emits valid stackmaps, and the
  platform unwind plumbing.
- Removing code is a one-way door; gating is reversible.

### Implementation outline

1. Add the Cargo feature in `scoopc_codegen_llvm` and `scoop_runtime`.
2. Thread a `cfg!(feature = "llvm-statepoint")` predicate through
   `pipeline.rs` and `gc.rs:62-91`.
3. In `runtime/c`, introduce a build-time `-DSCOOP_LLVM_STATEPOINT`
   macro driven by the runtime crate's feature; wrap the stackmap-only
   includes/branches.
4. Adjust the test files (`#[cfg(feature = "llvm-statepoint")]` or move
   them to a feature-gated submodule).
5. Drop or feature-gate the `dump-stackmaps` subcommand; if dropped,
   update `tests/p8_docs_cleanup.rs` and any spec_doctest fixtures that
   reference it.
6. Re-run the `gc-immix` compaction/parallel-mark stress suites to
   confirm explicit frames cover all moving-GC paths.

---

## Cross-references

- `SPEC_FIX.md` C4 (delete `sealed` marker → ref/value bound) interacts
  with the sealed-interface filter in `itable.rs:504,541`. R1 effectively
  removes the per-itable-entry filter site, but the sealed-interface
  semantics question remains and belongs in `SPEC_FIX.md`.
- `SPEC_FIX.md` does not currently track an item for either R1 or R2 and
  should not — both are ABI/implementation refinements, not spec wording.
