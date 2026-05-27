# CALLER_LOCATION

A working document for adding call-site source location capture to Scoop.
The immediate consumer is `panic` (which today discards both its message
and its location), but the mechanism is general — `assert`, `require`,
logging, custom diagnostics, and the `Raise<RuntimeError>` fatal terminus
all benefit from the same primitive.

This is **portable** by construction: no DWARF, no stack walking, no
platform unwind tables. Everything is decided at compile time and stored
as `.rodata` constants.

Status: **pending** — not yet applied.

Document conventions:

- §-numbered references point to `SCOOP_FULL_SPEC.md` / `docs/spec/`.
- File:line citations are concrete and reflect the tree at the time of
  writing.
- Prerequisites are flagged as **prereq** because they have to land
  before the user-visible mechanism becomes correct.

---

## 1. Motivation and scope

### 1.1 Why

`scoop.core.panic(message: String): Nothing` is currently
`@Extern("scoop_panic")` and the runtime implementation
(`runtime/c/scoop_runtime.c:845-848`) is `exit(3)` — the message is
ignored and no location is printed. This is the lowest-effort viable
behavior; it is also useless once a real program panics in production.

The natural fix is to print `panic at <file>:<line>:<col>: <message>` to
stderr before exiting. The **information needed** (call-site coordinates)
is already in the HIR Span at every panic call site; what is missing is
a portable mechanism to get it from the call site into the runtime
function.

### 1.2 Non-goals

- **Stack traces.** Walking the stack via DWARF / `backtrace()` /
  platform unwind tables would require platform-specific code and would
  fail under stripped binaries. Out of scope; see `RTTI_REFINE.md` R2
  for the broader portability-first stance.
- **Runtime-introspectable function metadata** (a `Function` reflection
  type with arbitrary attributes). Future work, not blocked on this
  document.

### 1.3 In-scope future consumers

Two synthesis paths share the same `Location` rodata constants:

**Path A — stdlib functions with `at: Location = currentLocation()`**:

- `assert(cond: Boolean, message: String = "...", at: Location = currentLocation())`
- `require(cond: Boolean, message: String = "...", at: Location = currentLocation())`
- `log.warn(message: String, at: Location = currentLocation())`

These rely on the §3 prereq (default-arg substitution Span semantics).

**Path B — compiler-synthesized fatal-terminus operators**:

- `expr!!` — null-assertion operator. Lowers to a fatal abort when
  `expr` is null. The operator's source Span is directly known by the
  compiler at the lowering site.
- `expr as T` — checked downcast. Lowers to a fatal abort when the
  runtime type is not a subtype of `T`. Same: operator Span is known.

Path B does **not** need §3 — the lowering pass synthesizes the
`Location` constant directly from the operator's Span, with no default
parameter involved. It only needs §4 (intrinsic / `Location` type
machinery) and §5 (representation + runtime helpers).

`Raise<E>` is **not** in this list. It is a regular effect: propagation
is value-typed via `EffectOutcome`, handler dispatch is lexical, and a
`Raise` that escapes the program is reported by the handler-resolution
infrastructure — there is no fatal-terminus runtime call to instrument.

These are not built in this document but the mechanism is designed so
that adding them is mechanical.

---

## 2. Design

### 2.1 Surface

Two stdlib types and one intrinsic, all in `scoop.core`:

```scoop
class Location internal (
    val file: String,
    val line: Int,
    val column: Int,
    val enclosingFunction: FunctionInfo?,   // null at module init / top level
) {
    fun toString(): String =
        if (enclosingFunction != null)
            "${file}:${line}:${column} (in ${enclosingFunction.fqn})"
        else
            "${file}:${line}:${column}"
}

class FunctionInfo internal (
    val fqn: String,
    val declFile: String,
    val declLine: Int,
    val declColumn: Int,
)

@Intrinsic("current_location")
fun currentLocation(): Location
```

Both classes have `internal` primary constructors. The compiler is the
only producer of `Location` / `FunctionInfo` values, via the intrinsic.
User code can read fields freely.

### 2.2 Usage

`panic` becomes:

```scoop
@Extern(name = "scoop_panic", abi = "scoop")
fun panic(message: String, at: Location = currentLocation()): Nothing
```

User-visible call sites do not change — `panic("oh no")` continues to
work — but the runtime now receives the call-site location.

Pass-through chains are explicit:

```scoop
fun <T> Option<T>.unwrap(at: Location = currentLocation()): T = match this {
    Some(v) => v
    None    => panic("unwrap on None", at)   // pass the caller's location through
}
```

When user code at `app.scoop:42:8` writes `opt.unwrap()`:

1. Spec §4.2 substitutes the default at the call site, synthesizing
   `opt.unwrap(currentLocation())` whose `currentLocation()` AST node
   carries the call-site Span (after prereq §3 lands).
2. `currentLocation()` expands to a `Location` literal with
   `file = "app.scoop", line = 42, column = 8`.
3. `unwrap`'s body forwards `at` to `panic`.
4. The panic message reads:
   `panic at app.scoop:42:8 (in app.unwrap-callsite-fn): unwrap on None`
   — the user's call site, not `core.scoop:1505` where `panic` is
   declared, and not `core.scoop:NNN` where `unwrap`'s body calls
   `panic`.

### 2.3 Comparison to alternatives

| Option | Surface | Hidden ABI | Pass-through |
|---|---|---|---|
| **`currentLocation()` as default expr** (this proposal) | `@Intrinsic` + class types | none | explicit (each forwarder declares its own `at` parameter) |
| `@callsite` parameter annotation | new modifier | none | requires modifier on every forwarder |
| Rust `#[track_caller]` | function-level attribute | hidden caller-frame in calling convention | implicit; transitive through annotated chain |
| DWARF stack trace | none | platform-specific | implicit |

We pick the first row because:

- It reuses three concepts already in the language (intrinsics, default
  parameters, classes).
- It does not couple ABI.
- Explicit forwarding is a small price for reading code: a function's
  signature alone tells you whether it captures caller location.
- It is fully portable.

---

## 3. **prereq** — tighten default-arg substitution Span semantics

### 3.1 The implementation gap

`SCOOP_FULL_SPEC.md` §B.5.2 and `docs/spec/language_spec-part3.md:97`
both say defaults are "evaluated at the call site (semantically)", but
the wording is loose enough that the current HIR lowering implementation
satisfies it without giving call-site Spans to the substituted AST
nodes.

`crates/scoopc_hir/src/hir/lower/expr/members.rs:1099-1320` is the
default-arg materialization path:

- `default_arg_funs` (`hir/lower/types.rs:188-202`) stores each default
  as a verbatim `Option<ast::Expr>` cloned from the function's
  declaration AST.
- At call site (`members.rs:1241-1251`):
  ```rust
  let default_value = param.default_value.as_ref()?;       // declaration-site AST
  let init = self.lower_expr_with_expected(pkg_prefix, default_value, expected);
  ```
  `init` inherits the declaration-site Span chain from the cloned AST.
- The wrapping `ValDecl::span` is set to `call_span`
  (`members.rs:1259-1264`) but the inner `init` Expr's Span is not
  rewritten.

Result: an `@Intrinsic` placed in a default expression observes the
**declaration site's** Span, not the call site's. For
`currentLocation()` this is the worst possible answer — every panic
would print the file:line of `panic`'s declaration in `core.scoop`,
which is completely useless.

### 3.2 The fix

Rewrite the substituted default expression's Span chain (at minimum the
root `Expr` node, ideally the entire subtree) to `call_span` at
substitution time.

Concrete implementation site: `members.rs:1241-1251`. Wrap the
`lower_expr_with_expected` call with a Span override, or post-process
the returned `Expr` to retag `.span` recursively. The retag must run
before any HIR pass that reads Span (intrinsic expansion, diagnostics,
etc.).

For non-intrinsic defaults this rewrite is a no-op behaviorally — they
read no Span — but it is observable in error messages: a runtime trap
produced by a default `Int` overflow would now point to the call site
instead of the declaration. That is also the better behavior.

### 3.3 Spec change

Tighten `SCOOP_FULL_SPEC.md` §B.5.2 and
`docs/spec/language_spec-part3.md` §4.2 to read (additional sentence):

> When a default value is supplied at a call site by omission, the
> substituted expression's source span is the call site's span. This
> applies to the substituted expression as a whole; nested constructs
> within it observe the call site as their lexical position for any
> compile-time intrinsic that reads source location.

This phrasing is narrow enough that it does not over-constrain
implementations on unrelated questions (e.g., where AST clones live,
diagnostic provenance for unrelated errors).

### 3.4 Test obligation

After the fix, add a fixture that exercises the spec promise *without*
relying on `currentLocation()`:

```scoop
// fixture: default expression's runtime panic points at call site
fun helper(x: Int = 1 / 0): Int = x
fun main() {
    helper()    // <- expected: panic location at this line, not at helper's signature
}
```

This protects the rewrite against future regressions.

---

## 4. Intrinsic semantics

### 4.1 `currentLocation()` expansion

`@Intrinsic("current_location") fun currentLocation(): Location`

At HIR lowering (or earlier — wherever intrinsics are expanded today),
when a `currentLocation()` call AST node is visited:

1. Read the call AST node's Span. After §3 lands, this is the call site
   for default-substituted occurrences and the literal lexical position
   for explicit occurrences.
2. Resolve Span → `(file_path, line_1based, column_1based)` via the
   source map.
3. Read the enclosing function being lowered from `LowerCtx`:
   - `fqn`: the function's stable FQN (already used elsewhere in
     codegen).
   - `declFile`, `declLine`, `declColumn`: from the function decl's own
     Span. For `LowerCtx` operating outside any function (top-level
     init, module body), set `enclosingFunction` to `null`.
4. Get-or-create a per-cone deduplicated rodata global of static type
   matching `Location`'s representation (see §5). Dedup key:
   `(file_path, line, column, fqn, declFile, declLine, declColumn)` —
   collapsing repeated calls at the same site.
5. Replace the call node with a constant reference to that global.

The intrinsic never has a real callable body. Calling it via reflection
or function pointer is rejected (consistent with other intrinsics).

### 4.2 Single-cone vs cross-cone

Each cone bakes its own rodata. Two cones that happen to have a panic
at the same `(file, line, column)` produce two distinct globals. This
is fine because the strings (file paths) are also per-cone allocated;
there is no cross-cone interning to coordinate.

### 4.3 Monomorphization

For generic functions, each monomorphization performed in a downstream
cone re-runs intrinsic expansion at user call sites. The
`enclosingFunction` field reflects the user's enclosing function, not
the generic body's, because — after §3 — the substituted default's Span
points at the user code and the lowering context is the user's
function. This is the desired behavior.

### 4.4 Inlining

If a function carrying `at: Location = currentLocation()` is inlined
into its caller (`@Inline`), the `Location` value is already a baked
constant by the time inline runs. Inline must not re-expand it; it
treats the `Location` reference as opaque, like any other constant.
Verify when implementing `@Inline` that this invariant holds.

---

## 5. Representation and ABI

### 5.1 `Location` layout

`Location` is a regular Scoop class with an `internal` primary
constructor. Its codegen layout is the standard nominal class:

```
+-----------------------+
| ScoopGcObjectHeader   |   (standard managed-object header)
+-----------------------+
| file: ScoopString*    |
| line: i64             |
| column: i64           |
| enclosingFunction:    |
|   FunctionInfo*       |   (nullable)
+-----------------------+
```

For the **first cut**, this is acceptable: each `currentLocation()`
expansion produces a **single rodata global of type `Location`**, with
the GC header marked as a non-collectable static object. The
deduplication in §4.1 step 4 keeps the rodata footprint bounded by the
number of distinct call sites.

`FunctionInfo` has the analogous layout, also stored in rodata, also
deduplicated (one per emitted function).

### 5.2 `ScoopString` for file paths

Each unique file path becomes a static `ScoopString` in rodata. These
are interned per-cone (same path → same global). The existing
`scoop_string_*` runtime helpers already accept `ScoopString*`, so no
ABI change on that side.

### 5.3 No GC allocation on the panic / assert path

Because `Location` and `FunctionInfo` instances live in rodata with
non-collectable headers, `panic` (and any future `assert` / `require`)
does not allocate when the default is taken. This matters in
hot-path-asserting code — even if the assertion is never tripped, no
GC pressure is added.

### 5.4 ABI for `scoop_panic` (C side)

```c
// Updated ABI; bumps SCOOP_RUNTIME_ABI_VERSION.
void scoop_panic(const ScoopString *message,
                 const ScoopLocation *location);
```

`ScoopLocation` is an opaque struct from C's perspective; the runtime
reaches into it via accessor helpers that match the layout in §5.1.
Implementation:

```c
void scoop_panic(const ScoopString *m, const ScoopLocation *loc) {
    if (loc) {
        const ScoopString *file = scoop_location_file(loc);
        int line = (int)scoop_location_line(loc);
        int col  = (int)scoop_location_column(loc);
        const ScoopFunctionInfo *fn = scoop_location_enclosing_function(loc);
        if (fn) {
            const ScoopString *fqn = scoop_function_info_fqn(fn);
            fprintf(stderr, "panic at ");
            scoop_string_write_stderr(file);
            fprintf(stderr, ":%d:%d (in ", line, col);
            scoop_string_write_stderr(fqn);
            fprintf(stderr, "): ");
        } else {
            fprintf(stderr, "panic at ");
            scoop_string_write_stderr(file);
            fprintf(stderr, ":%d:%d: ", line, col);
        }
    } else {
        fprintf(stderr, "panic: ");
    }
    if (m) scoop_string_write_stderr(m);
    fputc('\n', stderr);
    exit(3);
}
```

For the `!!` and `as` operators (Path B in §1.3), the compiler emits a
direct call to a dedicated runtime helper with a synthesized `Location`
constant:

```c
void scoop_null_assertion_failed(const ScoopLocation *location);
void scoop_cast_failed(const ScoopLocation *location,
                       const ScoopTypeDescriptor *expected,
                       const ScoopTypeDescriptor *actual);
```

Both end with `exit(3)` after stderr formatting. The `scoop_cast_failed`
variant also prints the type names (via the existing
`scoop_type_descriptor_name_*` helpers) so the diagnostic can read e.g.

```
cast failed at app.scoop:42:8 (in app.parse): expected Some<Int>, got None
```

`scoop_runtime_error_fatal` keeps its current `exit(3)` behavior — there
is no `Location` to add because `Raise<E>` is a regular effect, not a
fatal terminus (see §1.3).

---

## 6. Implementation outline

Ordering is enforced by the dependency graph in §3 → §4 → §5.

1. **prereq §3** — tighten default-arg substitution Span:
   - Update `members.rs:1241-1251` to retag the substituted Expr's Span
     to `call_span`.
   - Add the `1 / 0` default-expr fixture from §3.4.
   - Update spec §B.5.2 / §4.2 wording.

2. **§4 + §5.1-5.3** — intrinsic + types:
   - Add `Location` and `FunctionInfo` to `scoop.core`.
   - Register `current_location` intrinsic.
   - Implement expansion at HIR lowering, including rodata dedup.
   - Confirm `@Inline` does not re-expand (per §4.4).

3. **§5.4** — runtime ABI:
   - Bump `SCOOP_RUNTIME_ABI_VERSION`.
   - Update `scoop_panic` C signature; add `scoop_location_*` /
     `scoop_function_info_*` accessor helpers in `runtime/c/`.
   - Update `lower_panic_call` (`effect_lowered/value.rs:2442-2472`)
     and the panic dispatch site (`call/lowering.rs:1506`) to pass the
     `Location` ref.
   - Add `scoop_null_assertion_failed` and `scoop_cast_failed`. Wire
     `!!` and `as` lowering sites to synthesize a `Location` constant
     from the operator's Span and call these helpers. (No §3
     dependency — Span is read directly from the operator's AST node,
     not from a substituted default expression.)

4. **Fixtures**:
   - panic with default location: assert stderr matches
     `panic at <fixture-file>:<expected-line>:<col>: <msg>`.
   - panic with explicit pass-through (synthetic `unwrap`-style helper)
     to verify the location is the user's call site, not the helper's
     body.
   - `currentLocation()` called at top level → `enclosingFunction` is
     null in stderr formatting.
   - `currentLocation()` inside a `@Inline` callee — should observe the
     callee body's location for explicit calls (no default
     substitution), to confirm inline does not corrupt baked constants.

5. **Stdlib follow-ups (separate work)**:
   - Add `assert(cond, message?, at = currentLocation())`.
   - Add `require(cond, message?, at = currentLocation())`.

---

## 7. Open questions

1. **Span retag depth** (§3). Retagging the root Expr alone is enough
   for `currentLocation()`, since the intrinsic call AST node is itself
   the root of the substituted default. But if the default expression
   wraps the intrinsic in something nontrivial — e.g., `at: Location = if
   (debugMode) currentLocation() else cachedLocation` — the inner
   intrinsic call must also be retagged. Recommendation: retag
   recursively. This is `O(default-expr-size)` and only runs at call
   sites with omitted args. Cost is bounded.

2. **`enclosingFunction` for closures.** A `currentLocation()` inside a
   lambda body — what is the enclosing function? Two candidates:
   the synthesized closure callable, or the surrounding named
   function. Recommendation: the **named function** containing the
   lambda. The closure's synthesized name is an internal detail and
   exposing it is leaky. Easy to thread through `LowerCtx` since named
   ancestry is already tracked for diagnostic purposes.

3. **`enclosingFunction` for top-level expression contexts.** Module
   init, primary-constructor argument lists, top-level vals with
   default initializers. Recommendation: `null`, with the formatting
   in `scoop_panic` already handling that branch (§5.4).

4. **Cross-cone deduplication of file paths.** Each cone interns its
   own paths. If two cones share the same project source root and the
   same file is referenced from both, two distinct rodata strings
   exist. Recommendation: accept the duplication. The savings from
   cross-cone interning are not worth the linker coordination.

5. **`Location` toString format stability.** Once printed by
   `scoop_panic` to stderr, the format becomes a de facto contract for
   downstream tooling that grep-parses panic output. Recommendation:
   make the format deliberate from day one (the format in §5.4 is
   close to `clang`'s). Fix it in a fixture and document it under
   `SCOOP_RUNTIME.md`.

---

## Cross-references

- `SPEC_FIX.md` — no overlap. The default-arg-Span tightening (§3) is
  worth flagging there as well, since it is technically a spec wording
  change. **Recommendation:** add a small SPEC_FIX item once this doc
  is approved, so the spec change has a tracking entry.
- `RTTI_REFINE.md` — independent. Both documents share the
  portability-first stance but touch different runtime layers.
- `SCOOP_RUNTIME.md` — needs a follow-up entry once §5.4 lands,
  documenting the `panic at ...` stderr format as a stable surface.
