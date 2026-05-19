# Sysroot Reshape R2

> Created: 2026-05-19
> Status: design draft
> Scope: source-only cone model, sysroot layout, standard cone privileges, native FFI ownership, and core object initialization semantics.

## 0. Background

The previous core reshape split sysroot files into directories such as `sysroot/scoop.core/`, `sysroot/scoop.thread/`, and `sysroot/scoop.sync/`, but those directories are not real Scoop cones.

Current problems:

- Sysroot directories do not have `Cone.toml`, cone-local metadata, or cone-local source layout.
- The sysroot loader recursively scans `.scoop` files instead of loading standard cones.
- The `.cone` archive path was introduced before the source cone model was fully designed, and the current archive implementation is not a reliable dependency model.
- Runtime-specific C FFI for thread, sync, test helpers, and some string helpers still lives in `runtime/c/` and is always linked into every executable.
- `scoop.thread` and `scoop.sync` are declared as `@Intrinsic` surfaces and lowered by compiler FQN special cases to runtime C symbols.
- `SourceOrigin::Sysroot` currently grants special privileges implicitly, including intrinsic declaration privileges.

R2 replaces the directory-only split with real source-only cones and makes the privilege model explicit.

## 1. Goals

- Make every built-in library under sysroot a real source cone with `Cone.toml`, `src/`, metadata, and optional native sources.
- Revert dependency consumption to source-only cones, similar to Rust source crates, before redesigning binary distribution.
- Support cone kinds: `lib`, `bin`, and `syslib`.
- Move feature-specific native FFI out of `runtime/c/` and into the owning cone where possible.
- Keep runtime core limited to GC, allocation, root/native transition, object model, and required language substrate.
- Clarify native boundary annotations before moving sysroot FFI: `abi` describes boundary trampoline shape, while calling convention describes machine call layout.
- Replace sysroot-origin privilege checks with cone-kind privilege checks.
- Define separate auto dependency and prelude package lists.
- Make global object initialization use compiler-generated atomics instead of mutexes or runtime once helpers.
- Make top-level value initialization eager before entering `main`, with no lazy initialization behavior.
- Verify cone behavior from parsing and typechecking through native compilation and final link.

## 2. Non-Goals

- Do not redesign `.cone` binary archives in this phase.
- Do not implement `clib`, `dylib`, plugin, package registry, or binary distribution kinds in this phase.
- Do not preserve compatibility with the current `.cone` archive format.
- Do not make ordinary user cones trusted by setting `kind = "syslib"` outside the sysroot umbrella.
- Do not make all standard cones prelude imports.

## 3. Cone Kinds

`Cone.toml` must declare a cone kind.

```toml
[cone]
name = "scoop.core"
version = "0.0.0"
kind = "syslib"
```

Kinds:

| Kind | Entry Point | Privileges | Intended Use |
| --- | --- | --- | --- |
| `bin` | Required | ordinary user code | executable programs |
| `lib` | Not required | ordinary library code | reusable source libraries |
| `syslib` | Not required | trusted compiler surface | compiler-distributed standard/system libraries |

Rules:

- `bin` cones must provide an entry point.
- `lib` cones must not require `src/main.scoop`.
- `syslib` behaves like `lib` for source discovery, typechecking, native build, and linking.
- `syslib` additionally grants compiler-surface privileges that used to be granted by `SourceOrigin::Sysroot`.
- `syslib` is valid only under the sysroot umbrella path `sysroot/lib/<cone.fqn>/`.
- A user project or external dependency declaring `kind = "syslib"` outside `sysroot/lib/` is rejected.
- Future trusted SDK support must use an explicit trust configuration; it is not part of R2.

## 4. Sysroot Layout

The sysroot umbrella should be reorganized so built-in library cones live under `sysroot/lib/`.

```text
sysroot/
├── lib/
│   ├── scoop.core/
│   │   ├── Cone.toml
│   │   ├── src/
│   │   │   ├── core.scoop
│   │   │   ├── print.scoop
│   │   │   └── string.scoop
│   │   └── native/
│   │       └── ...
│   ├── scoop.lang.string/
│   │   ├── Cone.toml
│   │   ├── src/lang_string.scoop
│   │   └── native/
│   ├── scoop.thread/
│   │   ├── Cone.toml
│   │   ├── src/thread.scoop
│   │   └── native/thread.c
│   └── scoop.sync/
│       ├── Cone.toml
│       ├── src/sync.scoop
│       └── native/sync.c
├── bin/
└── docs/
```

Rules:

- `sysroot/lib/<cone.fqn>/` is the only standard library source cone root in R2.
- `sysroot/bin/` and `sysroot/docs/` are reserved and ignored by the compiler in R2.
- The sysroot loader must stop recursively scanning `sysroot/**/*.scoop`.
- The sysroot loader must discover `sysroot/lib/*/Cone.toml` and load each cone using the same source-cone rules as ordinary cones.
- `SCOOP_SYSROOT_OVERLAY` should mirror the new layout, for example `overlay/lib/scoop.core/...`.
- Overlay replacement should operate at cone/file-relative paths, not by blind recursive source collection.

## 5. Source-Only Dependency Model

The `.cone` archive functionality should be removed or disabled before R2 implementation starts.

R2 dependency model:

- Dependencies are source cones.
- The build driver constructs a source cone DAG.
- The frontend receives the full source cone graph, not a current cone plus `.cone` API injections.
- Cone boundaries remain visible to resolver, typechecker, visibility checks, intrinsic privileges, native-build ownership, and diagnostics.
- Binary archive metadata such as `api.scoopir`, `PRE_SPECIALIZE.json`, and archive-time public API injection is out of scope.

Deletion/retirement targets:

- Retire `scoop package` archive output or make it fail with a clear diagnostic until redesigned.
- Remove `.cone` dependency search from the normal build path.
- Remove `ConeArchiveApi` from ordinary source build flows.
- Retire `typecheck_cone_archive` fixtures or rewrite them as source-cone dependency fixtures.
- Remove archive-only `api.scoopir` dependency injection from the active frontend path.

Dependency syntax can be redesigned after the source-only graph is in place. R2 should at minimum support sysroot auto dependencies and local source path dependencies for tests and development.

## 6. Native FFI Ownership

Cone-local C/C++ sources are the mechanism for cone-specific FFI.

Before moving substantial sysroot FFI into cones, R2 must fix the native boundary vocabulary. Two concepts are intentionally separate:

- Boundary ABI: the compiler's trampoline/edge protocol. It decides GC state transitions, root exposure, hidden arguments, effect permeability, and whether the callee is treated as native or managed. Existing examples are `abi = "c"` and `abi = "scoop"` on `@Extern`.
- Calling convention: the target-machine call layout. It decides parameter/return placement, stack cleanup, LLVM call convention, and target ABI details. Examples are `C` / `cdecl` / future platform conventions.

Rules:

- `@Extern` imports an external symbol. It owns `name`, `abi`, and optional `callingConvention`.
- `@Extern(abi = "c")` uses the native boundary trampoline. Arguments and returns must be GC-free / `@NoGC`-safe. The compiler currently wraps direct C ABI calls and native `FunPtr` calls with `enter_native` / `leave_native`.
- `@Extern(abi = "scoop")` imports a managed Scoop ABI symbol. It does not automatically `enter_native` / `leave_native`; it runs as ordinary managed-attached code and may receive/return GC refs according to the Scoop ABI contract.
- `@CallingConvention` on a Scoop function with a body exposes that function as an object-level native callable symbol. It does not import an external symbol and does not mean dynamic-library/package export.
- `@CallingConvention` owns machine calling convention and optionally an object symbol name. It does not own boundary `abi`.
- A `@CallingConvention` body function must use a C ABI / GC-free signature for now. The function body is still managed Scoop code; native callers must satisfy runtime preconditions such as the current thread being GC-attached.
- `@Extern` and `@CallingConvention` are mutually exclusive on the same function.
- Future `@Export` can have its own `name`, `abi`, and `callingConvention`, because it describes a final product/package/dylib export boundary, not an object-level symbol for cone-local C.

Example import:

```kotlin
@Extern(name = "pthread_create", abi = "c", callingConvention = "C")
fun pthreadCreate(
    thread: UIntPtr,
    attr: UIntPtr,
    startRoutine: FunPtr<(UIntPtr) -> UIntPtr>,
    arg: UIntPtr,
): Int
```

Example object-level native callable Scoop entry:

```kotlin
@CallingConvention(name = "__scoop_thread_entry", convention = "C")
private fun threadEntry(arg: UIntPtr): UIntPtr {
    // Caller has already attached the current OS thread to the Scoop GC.
    return 0.toUIntPtr()
}
```

Thread entry uses this split:

- Cone-local C creates the OS thread and runs a small entry trampoline.
- The C entry trampoline calls runtime-core `scoop_gc_thread_attach_current()`.
- It then calls the Scoop `@CallingConvention` object symbol such as `threadEntry(rawHandle)`.
- The Scoop function uses `GC.handleGet` to recover the closure from a GC-free handle token, drops the handle, casts `Any as? () -> Unit / Pure!`, and invokes it.
- The C entry trampoline detaches the thread on normal return. Fatal traps abort the process and do not guarantee detach or cleanup.

This requires one missing runtime cast capability: `Any -> closed Pure function type`, at least `Any -> () -> Unit / Pure!`. The existing opposite direction, closed pure function value erasure to `Any`, is already allowed. The reverse cast must keep the same effect boundary: any target function type with non-`Pure` effects is rejected statically.

Implementing `Any as? () -> Unit / Pure!` requires more than a typecheck gate. Closure/function values need runtime descriptors that distinguish the function signature, not just the current generic `ScoopClosure` descriptor. A cast to a function type must check the object against a signature-specific function descriptor, otherwise a closure with an incompatible parameter/return shape could be called with the wrong ABI.

Existing manifest support already includes:

```toml
[native-build]
c-sources = ["native/foo.c"]
c-flags = ["-O0"]
cxx-sources = ["native/bar.cc"]
cxx-flags = ["-std=c++20"]
link-flags = ["..."]
```

The current implementation already proves that a cone can own C/C++ FFI sources: `Cone.toml` parses `native-build.c-sources/cxx-sources/link-flags`, `scoop build` compiles the current explicit cone's C/C++ sources to objects, and `tests/fixtures/run_pass_cone/c_sources_extern_call_basic/` covers a C source linked into a cone executable. R2 must extend this existing capability from “current executable cone only” to “all loaded source cones”.

Rules:

- Every loaded cone may contribute native sources and link flags.
- Native sources are compiled relative to the owning cone root.
- Native objects are linked in deterministic dependency-topological order.
- Duplicate native symbol failures should surface as linker diagnostics, not be hidden by the build driver.
- Cone native sources should be self-contained except for stable runtime-core public headers.
- Native sources should not include `runtime/c` private headers unless the cone is a trusted `syslib` and the dependency is explicitly part of the core runtime contract.
- Runtime C sources should no longer be a catch-all bucket for standard library feature implementations.
- C sources that currently live under `runtime/c/` but semantically belong to a sysroot cone must be migrated into that cone during this reshape, not left as permanent runtime-core exports.

Likely migration targets:

| Current Runtime File | Target Owner | Notes |
| --- | --- | --- |
| `runtime/c/scoop_thread.c` | `scoop.thread` | user-level thread API should move out of runtime core |
| `runtime/c/scoop_sync.c` | `scoop.sync` | mutex, condvar, and user-visible sync once should move out of runtime core |
| `runtime/c/scoop_test.c` | `scoop.runtime.test` | test-only helper ABI should not be linked into normal programs |
| string-from-array helpers in `scoop_runtime.c` | `scoop.lang.string` or `scoop.core` by substrate boundary | decide per helper after separating core string substrate from high-level string helpers |

The migration criterion is ownership, not file history. If the symbol implements a specific standard cone API, its source should live in that cone's `native/` tree and be built through that cone's `native-build` metadata. Runtime core should only keep shared substrate that cone-local C cannot own safely.

## 7. Runtime Core Boundary

Runtime core should contain only language/runtime substrate that cannot reasonably belong to a library cone.

Keep in runtime core:

- GC heap, allocation, type descriptors, write barriers, pin/handle primitives.
- Explicit root frames and stackmap registry support.
- Runtime initialization and thread registration needed by GC.
- Core object layout substrate and global root registration.
- Minimal string substrate if codegen and GC require a canonical runtime `String` object descriptor.
- Fatal trap/panic process termination substrate, if the compiler needs it as a non-effectful internal failure path.
- GC thread lifecycle substrate for OS-created threads, for example `scoop_gc_thread_attach_current()` and `scoop_gc_thread_detach_current()`.
- Stable public runtime header declarations needed by cone-local native sources, such as GC handle APIs and GC thread attach/detach.

Move out when possible:

- User-level `Thread` APIs.
- `Mutex`, `CondVar`, and user-visible `Once` APIs.
- Test-only runtime helpers.
- Feature-specific stdlib FFI that is not required by compiler-generated core code.

Fatal trap behavior:

- A fatal trap in a thread body aborts the entire process.
- R2 does not catch OS exceptions/signals/SEH to recover thread bodies.
- Fatal trap is not a Scoop `Raise` effect and is not recoverable.
- Normal thread return detaches from the GC; fatal trap does not guarantee detach, finalizers, defers, or cleanup.

## 8. Syslib Privileges

R2 replaces origin-based privilege with cone-kind privilege.

Privileges granted only to `syslib`:

- Defining `@Intrinsic` surfaces.
- Using `@file:AllowIntrinsic` or equivalent intrinsic declaration gates.
- Defining sysroot-only sealed marker interfaces or other compiler-owned marker surfaces.
- Defining compiler-recognized built-in nominal types.

Rules:

- `SourceOrigin::Sysroot` may continue to describe physical source origin, but it must not directly grant language privileges.
- Permission checks should ask whether the owning cone is trusted `syslib`.
- Ordinary `lib` cones under `sysroot/lib/` do not get intrinsic privileges unless their `Cone.toml` kind is `syslib` and the loader validates the path.
- User cones cannot self-elevate to `syslib`.

Initial classification should be conservative:

| Cone | Likely Kind | Reason |
| --- | --- | --- |
| `scoop.core` | `syslib` | built-in scalar/object/effect/GC surface |
| `scoop.unsafe` | `syslib` | raw pointer, function pointer, and atomic intrinsic surface |
| `scoop.lang.string` | `lib` or `syslib` | `lib` if it only uses normal Scoop plus `@Extern`; `syslib` only if compiler-only intrinsic remains |
| `scoop.collections` | `lib` or `syslib` | `syslib` only if compiler-owned iterable intrinsic remains |
| `scoop.delegates` | `syslib` while delegated property lowering depends on compiler intrinsic declarations |
| `scoop.thread` | `lib` or `syslib` | `lib` if converted to `@Extern`; `syslib` while `threadSpawn` needs closure ABI intrinsic lowering |
| `scoop.sync` | `lib` or `syslib` | `lib` if converted to `@Extern`; `syslib` while `Once.run` needs closure ABI intrinsic lowering |
| `scoop.runtime.test` | `syslib` or test-only trusted cone | must not be auto-loaded into normal user builds |

## 9. Auto Dependencies and Prelude Packages

R2 defines two separate lists.

`auto dependency` means a project automatically depends on the listed cones. These cones participate in resolving, typechecking, codegen, native build, and linking. Auto dependency does not imply automatic imports.

`prelude packages` means packages automatically star-imported into each source file. Prelude packages only affect unqualified name visibility. Their owning cones must already be loaded by explicit dependency, transitive dependency, or auto dependency.

Initial implementation can hard-code both lists. Configuration can come later.

Recommended initial auto dependency set:

| Cone | Reason |
| --- | --- |
| `scoop.core` | mandatory language core |
| `scoop.lang.string` | f-string and standard string helper surface |
| `scoop.collections` | iterable/collection protocols used by language sugar and common std surface |
| `scoop.delegates` | delegated property standard surface |

Recommended exclusions from default auto dependency:

| Cone | Reason |
| --- | --- |
| `scoop.thread` | user-level thread feature should be opt-in |
| `scoop.sync` | user-level synchronization should be opt-in unless a selected language feature requires it |
| `scoop.runtime.test` | test-only helper cone must never be loaded for normal programs |

`scoop.unsafe` should preferably be a declared dependency of `scoop.core` rather than a user-facing auto dependency. If implementation order makes that difficult, it may be temporarily auto-loaded without becoming a prelude package.

Default prelude packages:

| Package | Import |
| --- | --- |
| `scoop.core` | `import scoop.core.*` |
| `scoop.lang.string` | `import scoop.lang.string.*` |

Rules:

- Auto dependency and prelude package lists are independent.
- A prelude package whose cone is not loaded is a compiler configuration error.
- Explicit user dependencies duplicate with auto dependencies by identity and are deduplicated.
- Sysroot cone-to-cone dependencies should still be expressed in each cone manifest where possible, so cones remain self-contained.

## 10. Global Object and Top-Level Initialization

There are currently two different once mechanisms.

Current `object` and top-level immutable initialization uses `runtime/c/scoop_once.c` through compiler-emitted calls to `scoop_once_begin` and `scoop_once_end`. This helper uses C atomics, but it is still a runtime ABI and depends on platform thread identity/yield helpers.

Current `scoop.sync.Once` is implemented in `runtime/c/scoop_sync.c` with `ScoopPlatformMutex` and `ScoopPlatformCondVar`. It is a user-visible synchronization primitive and should not be part of core object initialization.

R2 target:

- Core object initialization must be generated by codegen using LLVM atomics.
- Object initialization must not call `scoop_once_begin`, `scoop_once_end`, or `scoop_sync_once_*` runtime helpers.
- Object initialization must not depend on `Mutex`, `CondVar`, `scoop.thread`, or `scoop.sync`.
- Object initialization failure is a fatal compiler/runtime trap, not a user-visible effectful exception.
- Object access must not become effectful just because initialization can fail internally.
- Top-level `val` initialization is not lazy and does not use the object once protocol.
- Top-level `val` initializers from every linked source cone must run before entering the user `main` body, in a deterministic compiler-defined order.
- Top-level `var` initializers follow the same eager-before-`main` rule, but each top-level `var` must be explicitly annotated as `@Global` or `@ThreadLocal`.
- `@Global var` storage is initialized in global storage before `main` body begins.
- `@ThreadLocal var` storage is initialized in the current thread's TLS before that thread observes the top-level binding; for the entry thread, this also happens before `main` body begins.
- The eager top-level initialization phase covers the consumer `bin` cone, all explicitly linked dependency cones, all auto dependency cones, and any transitive source cones that contribute code or native objects to the final program.
- Cone-level top-level initialization roots are ordered by dependency topology first, then by the cone's deterministic source/item order. A cone must not observe uninitialized top-level bindings from a dependency that appears earlier in the topology.
- Each linked source cone should emit its own cone init routine. The final system entry point calls these cone init routines in source cone DAG order, then enters the user `main` body.
- A cone init routine is responsible only for that cone's eager top-level `val`, `@Global var`, and entry-thread `@ThreadLocal var` initialization. It must not recursively initialize dependency cones; dependency ordering is owned by the final system entry point.

Guard state:

| State | Meaning |
| --- | --- |
| `0` | uninitialized |
| `1` | initializing |
| `2` | initialized |

Object access semantics:

- If state is `initialized`, acquire-load the guard and read the initialized storage.
- If state is `uninitialized`, one thread wins atomic compare-exchange to `initializing` and runs the initializer.
- The initializing thread publishes storage first, then release-stores `initialized`.
- Other threads observing `initializing` busy-loop with atomic acquire loads until state becomes `initialized`.
- Other threads must never read object/global storage while state is `initializing`.
- Same-thread recursive access to the same object initializer is a fatal trap.
- The trap is not modeled as `Raise` or any other user-visible effect.

The same-thread recursive check is implemented with a compiler-generated TLS init stack, not with a platform thread id or runtime helper.

Conceptual shape:

```c
thread_local ScoopObjectInitFrame *__scoop_current_object_init_frame;

struct ScoopObjectInitFrame {
  ScoopObjectInitFrame *prev;
  uint64_t *guard;
};
```

When a thread wins `cmpxchg(uninitialized -> initializing)` for an object, codegen creates a stack frame for that object init, links it into `__scoop_current_object_init_frame`, executes the object initializer, then unlinks the frame after the object is published as initialized. The frame must remain linked while the initializer body runs.

When object access observes `initializing`, codegen scans the current thread's TLS init-frame chain. If any frame guard equals the accessed object's guard, the access is same-thread recursive initialization and must fatal trap. Otherwise the access is from another thread, or from the same thread while it is initializing a different object, and it busy-waits until the guard becomes `initialized`.

This stack-shaped TLS check catches direct recursion and indirect cycles such as `A -> B -> A` on the same thread. It does not require `scoop.thread`, `scoop.sync`, `Mutex`, `CondVar`, OS thread ids, or runtime once helpers.

Busy wait can be a pure codegen loop. It may use LLVM atomics and simple branch backoff. It should not require a runtime helper. A later optimization may add target-specific pause instructions, but that is not part of the semantic contract.

Property initialization rule:

- Object initialization may read properties of the same object that have already been initialized earlier in declaration/init order.
- Object initialization must not read same-object properties that have not yet been initialized.
- Object initialization must not access the same object singleton through the ordinary object access path while the object is still initializing.
- A first implementation may enforce this statically by allowing references only to earlier same-object properties in initializer context.
- A later implementation may use per-property initialized bits if dynamic tracking becomes necessary.

Examples:

```kotlin
object O {
    val a = 1
    val b = a + 1 // allowed: a is earlier and already initialized
}
```

```kotlin
object O {
    val a = b + 1 // rejected or fatal: b is not initialized yet
    val b = 1
}
```

```kotlin
object O {
    val a = O.a // rejected or fatal: recursive ordinary access to O while initializing
}
```

## 11. Compiler Pipeline Changes

Required pipeline changes:

- Load source cones as graph nodes rather than flattening sysroot files first.
- Preserve cone identity on every source file.
- Preserve cone kind and trust status in resolver/typecheck/codegen inputs.
- Preserve top-level initializer roots per cone and expose them to entry generation in dependency-topological order.
- Generate one init routine per linked source cone and record the routine in codegen/link metadata.
- Generate compiler-owned TLS object-init frame stack helpers for same-thread recursive object initialization detection.
- Replace sysroot-origin privilege checks with owner-cone-kind checks.
- Select `bin` entry point only from the consumer `bin` cone.
- Allow `lib` and `syslib` cones without `src/main.scoop`.
- Compile native sources for all loaded cones.
- Link native objects from dependencies, sysroot auto dependencies, and the consumer bin cone.
- Emit a final system entry point that calls every linked cone init routine in source cone DAG order before the user `main` body.
- Remove `.cone` archive public API injection from normal build.

## 12. Verification Requirements

R2 must be verified end-to-end, not just by manifest parsing.

Required fixtures/tests:

- `lib` cone without `src/main.scoop` loads and typechecks.
- `bin` cone without an entry point fails with a stable diagnostic.
- `bin` cone depends on a source `lib` cone and can call its public Scoop function.
- `bin` cone depends on a source `lib` cone with native C FFI and links/runs successfully.
- A sysroot `syslib` can declare `@Intrinsic`; an ordinary `lib` cannot.
- A user cone declaring `kind = "syslib"` outside `sysroot/lib` is rejected.
- Auto dependency makes a cone available without writing it in the consumer `Cone.toml`.
- Prelude packages make `scoop.core.*` and `scoop.lang.string.*` visible without file-level imports.
- A non-prelude auto dependency still requires explicit import for short names.
- `scoop.thread` and `scoop.sync` are not linked into a program that does not depend on them after migration.
- Top-level `val` and annotated top-level `var` initializers run before `main` body and are not lazily triggered by first access.
- Top-level eager initialization covers all linked source cones, not only the consumer `bin` cone.
- Unannotated top-level `var` is rejected.
- Object initialization allows earlier-property references and rejects or traps recursive/later-property references.
- Cross-thread object initialization waits for initialization completion and never observes half-initialized storage.

## 13. Suggested Implementation Order

R2 should be implemented in small verified steps.

| Step | Focus | Outcome |
| --- | --- | --- |
| R2-0 | Archive/remove stale plans | root planning docs no longer conflict with R2 |
| R2-1 | Disable/remove `.cone` archive flow | no active build path depends on archive API injection |
| R2-2 | Add cone kind parsing and validation | `lib`, `bin`, `syslib` available with path trust checks |
| R2-3 | Restructure `sysroot/lib` | standard cones have `Cone.toml` and `src/` layout |
| R2-4 | Source cone graph loader | sysroot and dependencies load as cones, not flat files |
| R2-5 | Auto dependency and prelude split | loaded cones and imported packages are separate concepts |
| R2-6 | Native build for all source cones | dependency cone C/C++ sources compile and link |
| R2-7 | Runtime FFI migration | thread/sync/test helpers move out of runtime core |
| R2-8 | Codegen object once + eager top-level init | `scoop_once_*` runtime ABI no longer needed for object init; top-level values initialize before `main` |
| R2-9 | Full verification sweep | cone compile/typecheck/link/run coverage is complete |

## 14. Open Decisions

- Exact manifest syntax for source path dependencies.
- Whether `scoop.collections` and `scoop.delegates` are initial auto dependencies or explicit dependencies of compiler-lowered language features.
- Whether `scoop.lang.string` can be ordinary `lib` after native helper migration.
- Whether `scoop.thread.threadSpawn` can become `@Extern(abi = "scoop")` or still needs a `syslib` intrinsic adapter for closure ABI.
- Whether `scoop.sync.Once` remains as a user-visible API, and whether it should use mutex/condvar or a pure atomic implementation.
- Which runtime core C headers become stable public headers for cone-local native sources.
