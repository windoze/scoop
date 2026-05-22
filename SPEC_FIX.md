# SPEC_FIX

A round of spec review (see conversation 2026-05-22) identified 13 items that need
adjustment. This document is the working checklist for the spec change. Each
item gives the affected spec section, the change, the rationale, and the
implementation scope.

Status: **pending** — none of these have been applied to `SCOOP_FULL_SPEC.md`
or to the compiler yet.

Items are grouped by implementation cost (cheapest first inside each group).

---

## A. Spec wording only (no compiler change)

### A1. §2.1 — Add `Nothing` to the type hierarchy

**Current:** §2.1's type lattice does not list `Nothing`, but `raise(): Nothing`
and `panic(): Nothing` appear throughout §5 and elsewhere.

**Change:** Add `Nothing` as the **bottom type**: subtype of every type, no
inhabitants, used only as the return type of functions that never return
normally. Place it outside the reference/value split (it's neither).

**Why:** Pure spec completeness — the type is already in use.

---

### A2. §13 — Cone vs `package` wording

**Current:** §13 calls the package concept "Cone" but source still uses
`package foo.bar`. Reads like the same thing has two names.

**Change:** Make the layering explicit:
- **Cone** = distribution / build unit (analogous to Rust **crate**); `.cone`
  is the binary archive format.
- **package** = source-level namespace inside a cone (analogous to Rust
  **mod**).

Rewrite §13.1's opening so the relationship between the two is the first
thing the reader sees.

**Why:** Two-name confusion is a pure documentation problem; the layering
already exists in the implementation.

---

### A3. §B.8 — `operator` modifier required, not implementation-defined

**Current:** "Whether an explicit `operator` modifier is required is
implementation-defined unless specified elsewhere."

**Change:** Required (Kotlin style). A function named `plus` / `get` /
`compareTo` / etc. participates in operator resolution **only** when it
carries the `operator` modifier.

**Why:** Spec should not punt this to implementations. Required is safer:
prevents accidental operator capture by same-named utility functions.

**Compiler scope:** sema gains a check that operator-positioned calls
resolve only to `operator`-tagged functions.

---

## B. Spec + small compiler change

### B1. §7.2 — Delete `@Inline`

**Current:** `@Inline` is documented as a hint with no semantic effect.

**Change:** Remove the section entirely. Remove from the built-in annotation
table (§15.5). Compiler decides inlining itself.

**Why:** A pure hint with no semantic guarantee carries no value; Kotlin's
`inline` keyword only earned its place because it gave reified / non-local
return / crossinline. Scoop has none of those.

**Compiler scope:** delete `@Inline` recognition in sema; the existing
inliner already runs on its own heuristic.

---

### B2. §2.3.3 — Tuple field access `._0` → `.0`

**Current:** `t._0`, `t._1`. The leading underscore is unique to Scoop.

**Change:** Rust-style `t.0`, `t.1`.

**Lexer note:** `x.1.2` parses as `((x.0_int).1_int).2_int`. The lexer must
not greedily consume `1.2` as a float when the `1` immediately follows a `.`
that is part of a member-access token. Two equivalent fixes:

- Lexer state: after emitting `Dot`, force the next numeric run to be
  integer-only (no fractional part).
- Parser fix-up: when seeing `expr . FloatLit(N.M)`, split the float token
  back into `Int(N) Dot Int(M)`.

The lexer-state approach is simpler.

**Why:** `._0` is not used by any mainstream language; aligning with Rust
removes the surprise.

---

### B3. §4.4 / §5.7 — `!!` and `as` failure: panic, not Raise

**Current:** `!!` desugars to a match on `Option`, None-arm calls `perform
Raise.raise(RuntimeError.NullAssertionFailed)`. `as` desugars similarly with
`Raise.raise(RuntimeError.ClassCastFailed)` on type-test failure. Any
function using either gains `Raise<RuntimeError>` in its effect row.

**Change:** Both panic on failure. Removed from the effect system entirely:

- `x!!` panics if `x` is `None`; otherwise unwraps.
- `x as T` panics if the type test fails; otherwise casts.
- `as?` is unchanged (returns `None`).
- `RuntimeError` enum is no longer needed for `!!` / `as`; it can be deleted
  or repurposed as the panic-message tag (TBD when implementing).

**Why:** The current design lets a single `!!` poison a public function's
signature with `Raise<RuntimeError>`. That violates the "open row is for
intentional effects" intent — `!!` is an assertion, not a recoverable
failure.

**Compiler scope:** these are already syntactic sugar. The desugaring lives
in SHIR/SLIR lowering; only the failure-arm target changes from a `perform
Raise.raise(...)` call to a `panic(...)` call. Sema needs no separate
update — once nothing performs `Raise<RuntimeError>` from these paths, the
effect graph naturally stops adding the edge.

---

### B4. §5.3 etc. — Remove `perform` keyword

**Current:** Effect operations are invoked as `perform Effect.op(args)`. The
keyword appears in §5.3, §5.7 (all sugar-form examples), §14.7.1 (required
effects rule), Appendix A.3 (dispatch rule), and many fixture examples.

**Change:**

- The keyword is removed from the language.
- Effect operations are invoked as plain qualified calls: `Effect.op(args)`.
- The compiler distinguishes effect-op calls from ordinary function calls
  via the callee's resolution (the `Effect.op` form resolves to an effect
  declaration), not via syntax.

```scoop
// before
fun fetchData(): String / Raise<IOError> {
    if (resp.status != 200) {
        perform Raise.raise(IOError("bad status"))
    }
    resp.body
}

// after
fun fetchData(): String / Raise<IOError> {
    if (resp.status != 200) {
        Raise.raise(IOError("bad status"))
    }
    resp.body
}
```

**Why:**

- `Effect.op(...)` is already unambiguous: ordinary functions don't use the
  `EffectName.opName` qualified form, and the resolver knows whether the
  callee is an effect operation.
- `perform` adds no information at the call site — it duplicates what the
  callee's declared effect row already says.
- Crucially, `perform e` is a **prefix statement-keyword** the way C#
  `await` is. It defeats chaining:
  ```
  perform Yield.next() + 1     // ambiguous: is + part of the perform?
  val x = perform foo()        // forced temporary in many shapes
  ```
  C# learned this lesson the hard way; no reason to repeat it.
- Per the current implementation, `perform` is already optional, so dropping
  it is a removal, not a redesign.

**Compiler scope:** parser stops accepting `perform` (or accepts it for one
release as a soft-deprecated alias, then removes — TBD). All sysroot and
fixture examples mechanically rewritten to drop the keyword. Sema, SHIR,
SLIR, and effect propagation are unaffected (they already work on callee
resolution).

---

### B5. §7.6.1 — Closure `var` capture: forbid

**Current:** Captured `var` is reloaded from a snapshot on every closure
call; rebinding inside the closure does not persist across calls and does
not write back to the outer binding. `makeCounter` silently breaks.

**Change:** Closures **may not capture `var` bindings**. Compile error,
with a diagnostic that suggests the right tool:

- `RefCell<T>` for genuinely shared cross-call mutable state
- `fold` / other higher-order operators for accumulation patterns
- explicit `val snapshot = currentValue` for read-only borrows

`val` capture is unchanged.

**Why:**

- Snapshot semantics is technically the *consistent* choice (copy a value;
  copy a ref pointer; copy a var's value) but the cost in user-facing
  surprise (the `makeCounter` antipattern) outweighs the formal cleanliness.
- The Kotlin alternative — implicit boxing of captured vars to a heap cell
  — violates Scoop's design principle that **implicit boxing must be
  source-visible**. `val x: Any = 42` shows the box (the type widens). A
  captured `var` becoming a heap cell does not.
- Forbidding is the only third option that preserves both: no silent
  surprise AND no source-invisible allocation. Users have to be explicit
  about intent.
- Future-compatible: the rule can be relaxed later (to escape-aware
  boxing) if real demand surfaces. The reverse path is much harder.

**Compiler scope:** sema gains a single diagnostic when a closure body
references a captured `var`. No SLIR/codegen change. §7.6.2 migration note
deleted entirely.

---

### B6. §8.2 — f-string interpolation `{ ... }` → `${ ... }`

**Current:** `f"hello {name}!"`; literal braces require doubling
(`f"{{...}}"`).

**Change:**

- `f"hello ${name}!"` is the only interpolation form.
- `{` and `}` inside f-strings are literal characters — JSON-friendly, no
  escaping needed.
- No `$x` identifier shorthand. (Cost: complexity for no real win, plus
  visual ambiguity in cases like `f"$x+y"`.)
- `f` prefix is **kept**: f-strings desugar to a `StringBuilder` chain (not
  trivially zero-cost), so the prefix is a deliberate cost marker.

**Why:** JSON-heavy code (and any other text format using `{}`) becomes
readable without escape soup.

**Compiler scope:** lexer recognizes `${` as the interpolation opener inside
f-strings; f-string desugaring otherwise unchanged.

---

## C. Spec + medium compiler change

### C1. §2.6 — Enum `with` mismatched variant: silent ignore → panic

**Current:** "Updates targeting other variants are ignored and the original
value is kept."

**Change:** When the runtime variant does not match the variant named in the
`with` path, **panic**. Same failure mode as `!!` and `as` (item C4).

**Why:** Silent no-op on a path-typoed update is a real footgun. Panic
matches the language's other "you asserted X, X is false" failure semantics.

**Compiler scope:** SLIR lowering already builds the variant-check branch;
the "preserve original" arm is replaced with a panic call.

---

### C2. §5.4 / §5.7 / Appendix A — Handler keyword `with` → `on`

**Current:**
```
handle { body } with { Effect.op(args) -> ... } finally { ... }
```

**Change:**
```
handle { body } on { Effect.op(args) -> ... } finally { ... }
```

**Why:** Frees `with` from its double role (effect handler vs. value-type
update). Value-type `with` (§2.6) **stays** — it aligns with the Java
Valhalla `with`-expression proposal, and keeping the same surface eases
adoption for Java-track users.

The replacement keyword `on` is short, unambiguous in this position, and
reads naturally ("handle this **on** these effects").

**Try/catch desugaring:** the lowering target keyword changes correspondingly:

```
try { B } catch (e: T) { H }
≡ handle { B } on { Raise<T>.raise(e) -> H }
```

**Compiler scope:** parser accepts `on` in handler position; reject `with`
there with a clear diagnostic. All examples in §5 / Appendix A get
mechanically updated.

---

### C3. §4.2 — `val Some(x) = e` allowed; mismatch panics

**Current:** §4.2 forbids refutable patterns in `val` bindings; only
irrefutable patterns (tuple, struct) are accepted.

**Change:** Allow refutable patterns. On mismatch: **panic** (not Raise, not
silent). Equivalent desugaring:

```
val Some(x) = e
≡ val x = when (e) { Some(v) -> v; else -> panic("pattern mismatch") }
```

Update §4.2's prose: the panic-on-mismatch path is the same control-flow
shape as `!!`, so it is no longer "hidden match-failure control flow."

**Why:** Common, useful, and unambiguous once panic is the spec'd outcome.

**Compiler scope:** sema/SHIR lowering emits the panic fallback for
refutable val patterns.

---

### C4. §2.2.3 — Delete sealed marker, replace with `<T: ref>` / `<T: value>`

**Current:** `AnyRef` / `AnyValue` are `sealed interface` markers — a
"compile-time-only type" concept used only as generic bounds.

**Change:**

- Delete §2.2.3 entirely.
- Introduce two **bound keywords** (not types): `ref` and `value`. They
  appear only in generic bound position:

  ```
  class Atomic<T: ref>(initial: T) { ... }
  fun <T: value> hash(x: T): Int = ...
  ```

- Bound keywords are not types: they cannot appear as parameter type, return
  type, type argument, in `is` / `as`, or in supertype lists. The compiler
  enforces "T is a reference type" / "T is a value type" as an internal
  constraint kind, with no runtime metadata footprint.

**Why:** Same expressive power, no need for a third "kind of type" concept.
Real motivating use case (per author): generic types like `Atomic<T>` whose
implementation strategy differs across primitive / composite-value /
reference, which is cleanly expressed by overloading on the bound.

**Compiler scope:** add the two bound keywords to the parser; replace the
existing `AnyRef`/`AnyValue` constraint logic with the bound-kind check;
remove the sysroot sealed-marker declarations.

---

### C5. §13.6 — Default visibility: `public` → `internal`

**Current:** §13.6 documents the three visibility modifiers but does not
state a default. The implicit Kotlin-style default is `public`.

**Change:**

- A declaration with **no visibility modifier** is `internal`.
- `public` becomes a deliberate opt-in, marking the declaration as part of
  the cone's exported API.
- `internal` keeps Kotlin's meaning: **visible within the same cone**
  (cone = distribution unit, item A2).
- `private` keeps file-scope meaning (unchanged).

**Why:** Kotlin's `public`-by-default exists because the JVM model needs
inline / reified / crossinline declarations to be visible at every call site
that compiles against them. Scoop monomorphizes generics, so this
constraint does not apply, and the safer / more conservative default is
better:

- API surface is explicit (no accidental exports).
- Cones can refactor non-`public` declarations without breaking downstream.
- Removing `internal` declarations from `.cone` API exports becomes the rule
  rather than the exception.

**Compiler scope:**

- Sema: when no modifier is present, set visibility to `internal`.
- `.cone` exporter: only `public` declarations participate in `api.scoopir`.
- Spec §13.6: rewrite to lead with the default and the rationale.

---

## D. Decision records (no spec change beyond what's already listed)

### D1. §2.6 — Path A confirmed: keep value-type immutability + `with`

**Decision (no spec change beyond C1):** value types remain fully immutable.
No `var` fields in struct. The `with` expression (with the C1 fix) remains
the only update mechanism.

**Why this is recorded here:** During review we considered deleting `with`
in favor of Swift-style `var`-field mutation. That path requires either
introducing `mutating` methods (which collide badly with interfaces /
generics — the Self-type problem that has chased Rust and Swift for years)
or accepting verbose full-reconstruction syntax for nested updates. Path A
keeps the current spec.

**Future option:** if a real motivating case appears (e.g., a high-perf
vector-math library where copy elision matters), reopen the discussion
under a fresh proposal. Until then, the door stays closed.

---

## Summary table

| # | Section | Change | Scope |
|---|---|---|---|
| A1 | §2.1 | Add `Nothing` | spec |
| A2 | §13 | Cone vs package wording | spec |
| A3 | §B.8 | `operator` required | spec + small sema |
| B1 | §7.2 | Delete `@Inline` | spec + sema strip |
| B2 | §2.3.3 | Tuple `.0`/`.1` | spec + lexer |
| B3 | §4.4 / §5.7 | `!!` / `as` panic, leave effect system | spec + change desugaring target |
| B4 | §5.3 etc. | Remove `perform` keyword | spec + parser strip |
| B5 | §7.6.1 | closure var capture forbidden | spec + sema diagnostic |
| B6 | §8.2 | f-string `${...}` | spec + lexer |
| C1 | §2.6 | enum `with` mismatch → panic | spec + SLIR |
| C2 | §5.4 etc. | handler keyword `with` → `on` | spec + parser |
| C3 | §4.2 | `val Some(x) = e` allowed, panic | spec + sema/SHIR |
| C4 | §2.2.3 | sealed marker → `<T: ref/value>` bound | spec + sema |
| C5 | §13.6 | default visibility `public` → `internal` | spec + sema + cone exporter |
| D1 | §2.6 | (no change) Path A: keep `with`, no struct `var` | record-only |
