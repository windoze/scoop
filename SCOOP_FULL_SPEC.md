# Scoop Language Specification

**Version**: 0.1 (Draft)

## 1. Overview

Scoop is a statically-typed, garbage-collected programming language. It aims to provide a Kotlin-like experience without JVM restrictions, featuring true value types, rich enums, an algebraic effect system, monomorphized generics, and compile-time reflection.

### 1.1 Spec Doctest Fixtures

This spec may contain executable Scoop snippets. Code blocks that include a `// FIXTURE:` directive are treated as doctest fixtures:

- Extracted into `tests/fixtures/spec_doctest/` (see `tools/scoop_tools`)
- Verified in CI via `cargo run -p scoop_tools -- spec-fixtures check`
- Executed by `cargo run -p scoop -- test`

```scoop
// FIXTURE: spec_doctest/overview_minimal_main.scoop
// EXPECT: pass

fun main() {}
```

## 2. Type System

### 2.1 Type Hierarchy

```
Types
├── Reference types (GC managed, heap allocated)
│   ├── class (open / abstract / sealed)
│   ├── interface
│   └── boxed value type
│
└── Value types (inline, copy semantics, immutable)
    ├── built-in scalar types (e.g., Int/UInt, Int32/UInt32, ...)
    ├── struct
    ├── enum (rich tagged union / value-only "C enum")
    ├── tuple
    └── Unit (0-arity tuple)
```

All types fall into one of two categories: **reference types** and **value types**.

- **Reference types** are allocated on the GC-managed heap and accessed by reference. Assignment copies the reference, not the value.
- **Value types** are stored inline (on the stack or embedded in containing types). Assignment copies the entire value. All value types are **immutable** — their fields cannot be modified after construction.

### 2.2 Reference Types

#### 2.2.1 Class

Classes follow Kotlin semantics:

```kotlin
open class Animal(val name: String)
class Dog(name: String, val breed: String) : Animal(name)
abstract class Shape { abstract fun area(): Double }
sealed class Expr { ... }
```

- Classes support single inheritance and multiple interface implementation.
- Classes are `final` by default; use `open` to allow inheritance.
- `sealed` classes restrict subclassing to the same compilation unit.
- `abstract` classes cannot be instantiated directly.

#### 2.2.2 Interface

```kotlin
interface Hashable {
    fun hash(): Int
}

interface Printable {
    fun print() { println(toString()) }  // default implementation
}
```

- Interfaces can declare abstract members and provide default implementations.
- Both reference types and value types can implement interfaces.

### 2.3 Value Types

All value types share these properties:

- **Immutable**: Fields cannot be modified after construction.
- **No inheritance**: Value types cannot extend other value types.
- **Interface implementation**: Value types can implement interfaces.
- **No identity**: Two value type instances with the same field values are indistinguishable. Identity comparison (`===`) is not available for value types.
- **Copy semantics**: Assignment, parameter passing, and return all produce copies.

#### 2.3.1 Struct

Structs are named value types with named fields.

```kotlin
struct Point(val x: Int, val y: Int)

struct Color(val r: Byte, val g: Byte, val b: Byte, val a: Byte) : Hashable {
    fun hash(): Int = ...
}
```

- Structs can implement interfaces.
- Structs cannot extend other structs or classes.
- Structs can contain both value type and reference type fields.
- Any reference-typed fields stored inside value types must remain **precisely traceable** by the GC:
  - On the stack, the compiler provides precise stack maps (GC stack maps) for all live GC pointers, including those nested inside spilled value-typed aggregates.
  - Inside GC heap objects, the runtime traces references via the containing object's type descriptor (trace bitmap / trace function).

**Struct literal syntax**:

```kotlin
val p = Point { x: 1, y: 2 }
// equivalent to constructor call:
val p = Point(1, 2)
```

#### 2.3.2 Enum (Rich Enum)

Enums are tagged unions. Each variant can optionally carry data.

```kotlin
enum Color {
    Red,
    Green,
    Blue,
    Custom(val r: Byte, val g: Byte, val b: Byte)
}

enum Option<T> {
    Some(val value: T),
    None
}

enum Result<T, E> {
    Ok(val value: T),
    Err(val error: E)
}
```

- Enums are value types (immutable, copy semantics).
- Variants can carry zero or more fields.
- Enums can implement interfaces.
- Enums can have methods and computed properties.

**Memory layout**:

- Layout is `tag + union` where the union size equals the largest variant.
- Tag size is determined by variant count: ≤256 variants → `UInt8`, ≤65536 → `UInt16`.
- The compiler automatically boxes oversized variants and emits a lint warning when size disparity is significant.
- **GC trace safety (normative)**:
  - An enum is a value type, but its variants may carry GC-managed references (directly or nested inside value-typed aggregates).
  - The GC requires that **GC pointer slots are statically enumerable**:
    - On the stack, by compiler-provided GC stack maps.
    - In GC heap objects, by the containing object's type descriptor (trace bitmap / trace function).
  - Therefore, for any enum whose payload may contain GC-managed references, the in-memory layout must satisfy all of:
    - The set of GC pointer slots for the enum value is fixed for that enum type (for a given monomorphized instantiation) and **must not depend on the runtime tag**.
    - Each GC pointer slot must contain **only** `null` or a valid GC object pointer at all times.
    - Non-reference data must never overlap with a GC pointer slot, and code generation must initialize (and maintain) unused GC pointer slots as `null`.
  - Counterexample (why tag-dependent tracing is forbidden):
    - If an implementation overlays a reference field and a scalar field in the same word (i.e., a tag-dependent union), then scanning without reading the tag can misinterpret arbitrary scalar bits as a GC pointer:

      ```kotlin
      enum E {
          Ref(val s: String),
          Bits(val x: UInt64)
      }
      ```

      In `Bits(x)`, `x` may be any 64-bit pattern. If that word is also a GC pointer slot (per stack maps / heap bitmaps), the GC would attempt to trace it as an object pointer and may crash or exhibit undefined behavior. Requiring the GC to read the tag to decide which words are pointers is not allowed because stack maps and heap trace bitmaps are static and cannot express tag-dependent tracing, and moving GC requires in-place slot updates.
  - Recommended layout strategy (one valid approach): split the payload into a `ref_payload` segment (N machine words that store GC pointers only) and a `nonref_payload` segment (bytes for scalars), and ensure all unused `ref_payload` slots are set to `null`. Variants that cannot be represented without overlapping ref/non-ref words must use an explicit alternative representation (e.g. boxing) and must not silently rely on tag-dependent tracing.
- **Niche optimization** is applied where possible:
  - `Option<ReferenceType>` uses null pointer for `None` — zero overhead.
  - `Option<Bool>` uses value `2` for `None`.
  - For GC-managed references (and any value type whose runtime representation contains GC pointers), **non-NULL pointer-shaped niche encodings are forbidden**. In particular, nested niche (e.g., `Option<Option<RefType>>`) must not use illegal address values such as `0x1`; it must use an explicit tag (tagged union) representation.
  - `Option<ValueType>` where no niche exists uses an explicit tag.

##### 2.3.2.1 Value-only Enum (C-like Enum)

A **value-only enum** is an enum whose variants carry **no associated data** and whose in-memory representation is exactly an **integral underlying type**. This form is intended primarily for C interop and low-level runtime code.

Syntax:

```kotlin
enum SomeEnum: Int {
    A = 0
    B = 1
    C = 2
}
```

Rules:

- The underlying type is specified after `:` and must be an **integral scalar type** (e.g., `Byte`, `UByte`, `Int`, `UInt`, `Int64`, `UInt64`, ...).
- Variants in a value-only enum may not declare fields.
- Each variant has an **integral value** of the underlying type. (The exact rules for implicit auto-numbering are implementation-defined; this specification recommends requiring explicit values for FFI-facing enums.)
- The enum is still a distinct nominal type in the type system (it is not implicitly interchangeable with its underlying type), but its **ABI / memory layout is identical** to the underlying type.

FFI guarantee:

- A value-only enum has **exactly the same memory layout** as its underlying integral type.
- It may be used in `@Extern` function signatures and in `@CLayout` structs without requiring any additional annotations for layout compatibility.

#### 2.3.3 Tuple

Tuples are anonymous value types with positional fields and arbitrary arity.

**Unit / `()` — 0-arity tuple**

`()` is the Unit type, a 0-arity tuple with exactly one value (also written `()`). Functions that do not declare a return type implicitly return `Unit`.

```kotlin
val u: () = ()              // Unit value
fun greet(name: String) {   // implicit return type is Unit
    println(name)
}
```

**1-arity tuple `(T,)` — trailing comma required**

A trailing comma inside parentheses distinguishes a 1-element tuple from a grouping expression:

```kotlin
val single: (Int,) = (42,)   // 1-tuple wrapping an Int
val grouped: Int = (42)       // plain grouping, type is Int
```

**N-arity tuples**

Tuples of any arity are supported. There is no hardcoded limit (no `Tuple2`/`Tuple3` special-casing):

```kotlin
val pair: (Int, String) = (1, "hello")
val triple: (Int, Bool, String) = (1, true, "abc")
val quad: (Int, Int, Int, Int) = (1, 2, 3, 4)
```

**Field access**

Tuple fields are accessed with `._0`, `._1`, etc.:

```kotlin
val p = (10, "hi")
val x = p._0    // 10
val y = p._1    // "hi"
```

**Trailing comma rules**

- `()` — Unit literal (0-arity tuple)
- `(expr)` — grouping (not a tuple)
- `(expr,)` — 1-arity tuple (trailing comma required)
- `(a, b)` — 2-arity tuple (trailing comma optional)
- `(a, b, c)` — 3-arity tuple (trailing comma optional)

**Summary**

- Tuples are structurally typed: `(Int, String)` is the same type everywhere.
- Tuples are value types (immutable, copy semantics).
- Tuples support destructuring in `val`/`var` bindings and `when` expressions.
- `Unit` / `()` is the 0-arity tuple and the implicit return type of functions with no declared return type.

#### 2.3.4 Built-in Integer Types

Scoop provides a set of built-in integer value types that are available on all targets.

These types are **language built-ins**: their layout and semantics are fixed by the compiler. However, their *visible* declarations (including standard aliases) MUST be provided by the sysroot, so that they can be referenced from user code and tooling like normal types.

> 说明：这些类型是语言 builtin（布局/语义由编译器固定），但它们的可见声明由 sysroot 提供。

##### Word-sized integers (`Int` / `UInt`)

Following Swift's convention for native languages:

- `Int` is the target's **native signed word-sized integer**.
- `UInt` is the target's **native unsigned word-sized integer**.

Both have a bit width equal to the compilation target's pointer size:

- On 64-bit targets: `Int` / `UInt` are 64-bit.
- On 32-bit targets: `Int` / `UInt` are 32-bit.

These types are the preferred choice for:

- Collection sizes and indices
- In-memory offsets
- `sizeOf<T>()` / `alignOf<T>()` results

##### Fixed-width integers (`IntN` / `UIntN`)

For portable, layout-stable code (serialization, file formats, on-disk structures, FFI ABIs), Scoop also provides fixed-width integers:

- Signed: `Int8`, `Int16`, `Int32`, `Int64`
- Unsigned: `UInt8`, `UInt16`, `UInt32`, `UInt64`

Future extension (not in 0.1): `Int128` / `UInt128` may be provided on some targets.

##### Standard aliases

The sysroot MUST provide standard type aliases for conventional names:

```kotlin
typealias Byte   = UInt8
typealias Short  = Int16
typealias UShort = UInt16
typealias Long   = Int64
typealias ULong  = UInt64

// Explicit "address-sized integer" name for unsafe/FFI documentation.
// In Scoop 0.1 this is just an alias to UInt.
typealias UIntPtr = UInt
```

Note: `Byte` is an **unsigned** 8-bit integer (`UInt8`).

##### Operations and semantics

All integer types support:

- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Bitwise: `&`, `|`, `^`, `~`
- Shifts: `<<`, `>>`

**Bit width and representation:**

- Fixed-width integers have the specified bit width (`Int32` is 32-bit, etc.).
- Word-sized integers (`Int` / `UInt`) have a bit width equal to the target pointer size.
- Signed integers use two's complement representation.

**Overflow:**

Unless otherwise specified by an intrinsic API, integer operations are defined as **modular arithmetic** modulo `2^bitWidth` (wrap-around).

**Right shift:**

- For signed integers, `>>` is an arithmetic right shift.
- For unsigned integers, `>>` is a logical right shift.

**Shift counts:**

Shift counts are masked by the type's bit width (equivalent to `shift % bitWidth`) to avoid target-dependent behavior.

### 2.4 Nullability

Nullable types are syntactic sugar for `Option<T>`:

```kotlin
val x: Int? = 42       // Option<Int>, equivalent to Some(42)
val y: Int? = null      // Option<Int>, equivalent to None
```

- `T?` desugars to `Option<T>`.
- Nesting is **not** flattened: `T??` ≠ `T?`. `Option<Option<T>>` has three states: `Some(Some(v))`, `Some(None)`, and `None`.
- Kotlin's null-safe operators (`?.`, `?:`, `!!`) are supported as sugar over `Option` operations.

### 2.5 Boxing

When a value type is assigned to an interface type, it is **boxed** — wrapped in a GC-managed heap object.

```kotlin
val n: Number = 42          // implicit boxing, O(1)
val list: List<Number> = listOf(1, 2, 3).map { it as Number }  // explicit conversion, O(n)
```

- **O(1) boxing is implicit**: assigning a value type to an interface variable, passing to an interface parameter.
- **O(n) boxing is explicit**: converting a container of value types to a container of interface types requires explicit conversion (e.g., `.map`).

### 2.6 Value Type Update (`with` Expression)

Since value types are immutable, the `with` expression creates a modified copy.

```kotlin
val p2 = p with { x: 5 }

val line2 = line with {
    start.x: 1
    start.y: 2
}

val line3 = line with {
    start: Point { x: 1, y: 2 }
    end: Point(10, 10)
}
```

- Syntax: `expr with { path: value, ... }`
- **Parallel semantics**: All right-hand side expressions are evaluated against the original value. There is no order dependency between updates.
- Paths can be arbitrarily deep: `a.b.c: value`.
- `with` returns a new value; the original is unchanged.

## 3. Generics

### 3.1 Monomorphization

Generics are implemented via monomorphization. The compiler generates specialized code for each concrete type argument.

```kotlin
fun <T> identity(value: T): T = value

// Compiler generates:
// fun identity_Int(value: Int): Int = value
// fun identity_String(value: String): String = value
```

- `reified` is not needed — type information is always available at compile time.
- For reference type arguments, the compiler may share generated code (all reference types have the same layout: a GC pointer).
- For value type arguments, each type gets its own generated code (layouts differ).

### 3.2 Variance

Scoop supports declaration-site variance (`in`/`out`) following Kotlin semantics, with one restriction:

**Variance only applies when type arguments are reference types.**

```kotlin
interface Producer<out T> {
    fun produce(): T
}

val catProducer: Producer<Cat> = ...
val animalProducer: Producer<Animal> = catProducer  // ✅ Cat is a reference type

val intProducer: Producer<Int> = ...
val numProducer: Producer<Number> = intProducer  // ❌ Int is a value type, needs explicit conversion
```

When a value type occupies a variant position, subtyping does not apply because the memory layouts differ. Explicit boxing/conversion is required.

### 3.3 Star Projection

`List<*>` (equivalent to `List<out Any?>`) holds boxed references. Converting `List<Int>` to `List<*>` requires explicit boxing of elements.

### 3.4 Effect Row Parameters (`eff`)

Scoop supports **effect polymorphism** by allowing generic declarations to quantify over **effect rows** (see §5.8). An effect row parameter represents a set of effects that a computation may perform.

Effect row parameters are introduced in a generic parameter list using the contextual keyword `eff`:

```kotlin
interface Disposable<eff E = Pure> {
    fun dispose(): Unit / E
}
```

Use-site syntax mirrors the declaration: the effect row argument is supplied with an `eff` clause:

```kotlin
val d0: Disposable = ...                         // Disposable<eff Pure>
val d1: Disposable<eff Async> = ...
val d2: Disposable<eff (Async + Raise<IOError>)> = ...
```

Rules:

- `eff` is a **contextual keyword**: it is only treated as a keyword inside a `<...>` generic parameter/argument list. Outside of generics, `eff` is an ordinary identifier.
- A generic list may contain **at most one** `eff` clause, and it must appear **last**.
- In this draft, the `eff` clause introduces (or supplies) **exactly one** effect row parameter/argument.
- Effect row parameters are **compile-time only**. They do not affect memory layout, runtime type identity, or code generation specialization. (They are erased after type checking.)

## 4. Pattern Matching

Scoop uses `when` for pattern matching, following Kotlin's keyword choice but with richer destructuring capabilities.

```kotlin
// Enum variant patterns — no `is` prefix needed
when (value) {
    Circle(r) -> pi * r * r
    Rect(w, h) -> w * h
    else -> 0.0
}

when (option) {
    Some(x) -> process(x)
    None -> default()
}

// Type check patterns — `is` prefix for runtime type checks
when (animal) {
    is Dog -> animal.bark()      // smart cast: animal is Dog in this branch
    is Cat -> animal.purr()      // smart cast: animal is Cat in this branch
    else -> animal.toString()
}

// Tuple patterns
when (pair) {
    (0, _) -> "starts with zero"
    (x, y) if x == y -> "equal"
    (x, y) -> "different: $x, $y"
}
```

**Two kinds of patterns in `when`:**

- **Variant patterns** (enum destructuring): `Variant(bindings)`, `Variant` — no `is` prefix. The subject is statically known to be the enum type.
- **Type check patterns** (runtime type test): `is Type` — requires `is` prefix. The subject's concrete type is checked at runtime (reference types only).

### 4.1 Exhaustiveness

- **Exhaustive types** (compiler checks full coverage): `enum`, `Bool`, `Option<T>`, `Result<T, E>`, and nested combinations thereof.
- **Non-exhaustive types** (must have `else` / `_` arm): `Int`, `String`, all numeric types, interface types, and patterns with guard clauses (`if`).
- If all variants of an exhaustive type are covered and an `else` arm is still present, the compiler emits a warning (helps catch missing cases when new variants are added).

### 4.2 Destructuring

Pattern matching supports destructuring for:

- Enum variants: `Some(x)`, `Ok(value)`, `Err(e)`
- Tuples: `(a, b, c)`
- Structs: `Point { x, y }` or `Point { x: px, y: py }` (renaming)
- Nested patterns: `Some((x, y))`, `Ok(Point { x, y })`
- Or-patterns: `North | South -> "vertical"`
- Guard clauses: `Some(x) if x > 0 -> "positive"`
- Literal patterns: `42`, `true`, `"hello"`
- Wildcard: `_` (matches anything, binds nothing)
- Rest: `..` (ignore remaining fields/elements)

Destructuring is also supported in `val` bindings:

```kotlin
val (a, b) = get_pair()
val Point { x, y } = make_point()
```

Note: `var` does not support destructuring patterns (only simple bindings).

### 4.3 Type Check (`is`) and Smart Cast

The `is` operator performs a runtime type check on reference types and returns `Bool`:

```kotlin
if (animal is Dog) {
    // Smart cast: `animal` is automatically narrowed to `Dog` here
    animal.bark()
    println(animal.breed)
}
```

**Smart cast** means the compiler automatically narrows the type of a variable after a successful `is` check, eliminating the need for explicit casts. Smart casts apply in the following positions:

```kotlin
// 1. if/else — true branch narrows the type
if (obj is String) {
    println(obj.length)          // obj is String here
}

// 2. if + return/throw/break — code after the check narrows
if (obj !is String) return
println(obj.length)              // obj is String here

// 3. && — right side of && narrows
if (obj is String && obj.length > 0) {  // obj is String after &&
    println(obj)
}

// 4. when — each `is` branch narrows
when (node) {
    is BinaryExpr -> emit(node.left, node.op, node.right)
    is UnaryExpr  -> emit(node.op, node.operand)
    is Literal    -> emit(node.value)
}
```

**Smart cast limitations:**

- Only applies to **`val` bindings** and **function parameters** (immutable references). A `var` can be reassigned between the check and the use, so smart cast is unsafe.
- Only applies to **reference types**. Value types use enum variant patterns for narrowing, not `is`.
- Only applies when the compiler can prove no concurrent mutation (single-threaded analysis scope).
- If a smart cast is not possible, the compiler reports an error suggesting an explicit cast.

```kotlin
var x: Any = "hello"
if (x is String) {
    // ❌ Smart cast not possible: `x` is a var that could be reassigned
    // Use explicit cast: val s = x as String
}

val y: Any = "hello"
if (y is String) {
    println(y.length)  // ✅ Smart cast works: `y` is val
}
```

### 4.4 Explicit Casts (`as` / `as?`)

When smart cast is not applicable, explicit casts are available:

```kotlin
// Unsafe cast — raises if the type doesn't match
val dog = animal as Dog
dog.bark()

// Safe cast — returns null (Option<T>.None) if the type doesn't match
val maybeDog = animal as? Dog    // type: Dog?
maybeDog?.bark()
```

- `as`: If the runtime type doesn't match, performs `Raise.raise(RuntimeError.ClassCastFailed)` and therefore requires `Raise<RuntimeError>` unless handled by `try`/`catch` (see §5.7 and §14.7).
- `as?`: Returns `None` if the cast fails. Equivalent to `if (x is T) Some(x) else None`. Preferred for defensive code.
- Neither `as` nor `as?` perform value type conversions. Use explicit conversion functions for that (e.g., `intValue.toLong()`).

### 4.5 Negated Type Check (`!is`)

```kotlin
if (animal !is Dog) {
    println("not a dog")
    return
}
// Smart cast: animal is Dog here (early return guarantees it)
animal.bark()
```

`!is` is syntactic sugar for `!(expr is Type)`. Smart cast applies after the negative branch exits.

## 5. Effect System

### 5.1 Overview

Scoop uses an algebraic effect system to unify async operations, error handling, and other control flow effects. Effects are declared like interfaces and handled with `handle` blocks.

### 5.2 Effect Declaration

```kotlin
effect Async {
    fun <T> await(task: Task<T>): T
}

effect Raise<E> {
    fun raise(error: E): Nothing
}

effect Emit<T> {
    fun emit(value: T)
}
```

### 5.3 Using Effects

Functions that perform effects declare an **effect row** after `/` in their signature (see §5.8). Multiple effects are combined with `+` (set union).

Effect annotations may be omitted:

- For a **public** function, omitting the effect annotation is treated as `/ Pure`. Any non-`Pure` effect required by the body is a compile error (see §14.7).
- For a **private/internal** function, the effect row may be inferred from the body when omitted (see §14.7).

Effect operations are invoked with the `perform` keyword using qualified names (`Effect.operation`):

```kotlin
fun fetchData(): String / Raise<IOError> {
    val resp = httpGet("/data")
    if (resp.status != 200) {
        perform Raise.raise(IOError("bad status"))
    }
    resp.body
}
```

> **Note on `async fun`**: `async fun foo(): T` does **not** desugar to `fun foo(): T / Async`. Instead, it desugars to `fun foo(): Task<T>` where the body is captured as a lazy computation. The `/ Async` effect only exists inside the Task's computation context. See §5.7 for details.

### 5.4 Handling Effects

Handler arms have three explicit forms, distinguished by syntax:

#### Non-resuming handler (`->`)

The handled computation is abandoned. Used for error handling (try/catch pattern).

```kotlin
handle {
    val data = fetchData()
    println(data)
} with {
    Raise.raise(error) -> println("caught: " + error)
}
```

`resume()` is a compile error inside a non-resuming arm. The computation body does not continue after the handler runs.

#### Immediate-resume handler (`-> resume`)

The handler resumes the computation synchronously. The body is compiled into a state machine; each `perform` point is a suspend/resume boundary.

```kotlin
handle {
    process()
} with {
    Logger.log(level, msg) -> resume {
        println("[" + level + "] " + msg)
        resume(Unit)
    }
}
```

`resume(value)` must be called exactly once inside a `-> resume` arm.

A stack-local fast path is permitted but not required. The current compiler lowers `-> resume` through the same unified full-machine contract used by the rest of effect lowering; a dedicated stack-local fast path remains a deferred optimization and is not part of the current runtime ABI contract.

#### Escape-continuation handler (`, k ->`)

The continuation is captured as a first-class `Continuation<T, eff E>` value. It can be stored, passed to other functions, and resumed later — possibly after the handler has returned.

```kotlin
handle {
    val data = await fetchData()
    println(data)
} with {
    Async.await(task), k -> {
        // k : Continuation<T, eff E>
        // advanced API: store `k` in a GC-managed object and resume it later
        ()
    }
}
```

`resume()` is unavailable; use `k.resume(...)` explicitly. The state machine is heap-allocated (GC-managed) so it can outlive the handler scope.

`k.resume(...)` is **one-shot**: calling it twice performs `Raise.raise(RuntimeError.ContinuationAlreadyResumed)` (and therefore requires `Raise<RuntimeError>` unless handled).

Directly capturing and resuming `Continuation` values is an **advanced control-flow API**. General-purpose async code should normally expose `Task<T>` rather than raw continuations.

#### Summary

| Arm syntax | `resume()` | `k` | State machine | Allocation |
|---|---|---|---|---|
| `Effect.op(args) -> { ... }` | compile error | no | none | none |
| `Effect.op(args) -> resume { ... }` | required | no | implementation-defined state machine | implementation-defined |
| `Effect.op(args), k -> { ... }` | unavailable | yes | heap struct + step fn | GC |

### 5.5 Continuation Type

`Continuation<T, eff E = Pure>` is a built-in generic type representing a suspended computation that expects a value of type `T` to continue.

- `T` is determined by the return type of the effect operation.
- `E` is the **required effect row** of the suspended computation when it is resumed (i.e., resuming the continuation may perform effects in `E`).

```kotlin
effect Ask {
    fun question(prompt: String): String   // continuation: Continuation<String, eff E>
}

effect Yield {
    fun next(): Int                        // continuation: Continuation<Int, eff E>
}
```

Operations:
- `k.resume(payload...): Unit / (E + Raise<RuntimeError>)` — resume the suspended computation.
  - Payload surface follows the continuation payload shape:
    - If `T` is `Unit`, `k.resume()` is accepted.
    - If `T` is a tuple `(A0, A1, ...)`, `k.resume(v0, v1, ...)` is accepted; named args use `a0`, `a1`, ...
    - For all payload types, the legacy single-payload form `k.resume(value)` remains accepted; for tuple payloads this means passing the tuple value itself.
  - One-shot: if called twice, performs `Raise.raise(RuntimeError.ContinuationAlreadyResumed)` (no panic).
  - If the resumed computation suspends again through an escape-continuation handler, that handler captures a **fresh** continuation. `k.resume(...)` itself still returns `Unit`; it does not return the next continuation.

Resumption context:

- `Continuation<T, eff E>` captures the dynamic effect context (the active handler stack; Appendix A) at the suspension point.
- Calling `k.resume(...)` resumes the computation under that captured handler stack, even if `resume` is invoked from a different OS thread.
- Implementations typically realize this by storing the captured handler stack pointer in the continuation object and installing it into the current thread’s TLS effect state for the duration of the resumed step, then restoring the previous TLS state on return.
- `Continuation<T, eff E>` belongs to the language's **advanced API surface**. Typical async code should interact with `Task<T>`; runtimes and advanced libraries may store continuations internally and replace them with a fresh continuation after each suspension.

### 5.6 Compilation Strategy

| Arm form | Compilation strategy |
|---|---|
| `->` (non-resuming) | Flag-based CPS unwinding; no state machine |
| `-> resume` (immediate) | Implementation-defined state machine. The current compiler reuses the unified full-machine lowering; a dedicated stack-local fast path is deferred. |
| `, k ->` (escape) | GC-allocated state machine struct; step function driven by external caller |

Implementation details:

Note: the runtime symbol names used in this section (e.g., `__scoop_effect_active`, `scoop_perform_*`, `scoop_continuation_resume`) are illustrative only and do **not** constitute a stable ABI. The exact runtime entrypoints, task-poll hooks, and symbol names are implementation-defined and may change.

- **Per-thread runtime state**: effect dispatch/unwinding state is maintained in thread-local storage (TLS), including the active handler stack pointer and the current "perform slot" used by flag-based unwinding.
- **Cross-thread resume**: when a continuation is resumed, its captured handler stack is installed into the resuming thread’s TLS effect state. This makes the effect context conceptually fiber-local whether the continuation is resumed manually or by some future executor/scheduler framework.
- **Flag-based unwinding**: `__scoop_effect_active` is a thread-local flag. `scoop_perform_*` functions write the operation tag and arguments into the thread-local perform slot, set the flag, and return; callers check the flag and propagate. Before running any user code during propagation (handler arm bodies, `finally` blocks), the runtime/compiled code must capture the slot and clear the flag; if the effect is still unhandled at that boundary, it is re-raised after cleanup.
- **Stack state machine**: Handle body split into segments at perform/call boundaries. `Var` locals lifted across segments. While-loop dispatches to handler arms on each suspend, re-enters at next state on resume.
- **Heap state machine**: Same segmentation, but state + lifted locals live in a GC-allocated struct. A `step()`/`poll()` driver advances the computation until the next suspension or completion and reports `Pending` or `Ready(value)`. Escape continuations may keep a pointer to this struct.
- **One-shot only**: Each suspension point resumes at most once. Multi-shot continuations are not supported.

### 5.7 Syntax Sugar for Common Effects

High-frequency effects have dedicated syntax that compiles to the same code as the corresponding `handle` block.

#### try / catch / finally (sugar for `Raise` effect)

```kotlin
try {
    val data = readFile("config.json")
    parse(data)
} catch (e: IOError) {
    logError(e)
    defaultConfig
} finally {
    cleanup()
}

// Desugars to (non-resuming arm):
handle {
    val data = readFile("config.json")
    parse(data)
} with {
    Raise.raise(e) -> { logError(e); defaultConfig }
} finally {
    cleanup()
}
```

The `->` (non-resuming) form means the computation is abandoned on error.

#### Runtime errors (`RuntimeError`)

Some language constructs may fail at runtime (e.g., `!!`, `as`). These failures are expressed uniformly via the `Raise` effect using a built-in value type `RuntimeError`:

```kotlin
enum RuntimeError {
    NullAssertionFailed,
    ClassCastFailed,
    ContinuationAlreadyResumed,
}
```

Rules:

- `x!!` (not-null assertion) is sugar for:
  - if `x` is `Some(v)`, evaluate to `v`
  - if `x` is `None`, perform `Raise.raise(RuntimeError.NullAssertionFailed)`
- `x as T` (unchecked cast) performs `Raise.raise(RuntimeError.ClassCastFailed)` if the runtime type check fails.

As a consequence, these constructs require `Raise<RuntimeError>` unless the failure is handled by `try`/`catch` (or `handle`).

#### async / await (sugar for `Async` effect)

Async is built on escape continuations. The `Async` effect is a built-in effect:

```kotlin
// Built-in effect (compiler-known)
effect Async {
    fun await<T>(task: Task<T>): T
}
```

Current-stage focus:

- This stage standardizes effect codegen and the core shape of `Task<T>`.
- Executor frameworks (queues, wakeups, work-stealing, spawn scheduling) are intentionally deferred.
- A `Task<T>` must remain usable without an executor by manual polling/stepping.

Conceptual async surface:

```kotlin
enum Poll<T> {
    Pending,
    Ready(T),
}

class Task<T> {
    fun poll(): Poll<T>
}
```

A task is lazy and may be driven manually:

```kotlin
val t: Task<User> = async {
    val resp = await httpGet("/user")
    parse(resp.body)
}

while (true) {
    when (t.poll()) {
        Pending -> ()
        Ready(user) -> {
            println(user.name)
            break
        }
    }
}
```

Key semantics:
- `Task<T>` is the **general-purpose async API**. `Continuation<T, eff E>` remains the lower-level advanced API used to implement tasks and other control-flow libraries directly.
- `async { body }` creates a lazy `Task<T>`.
- `async fun foo(): T` desugars to `fun foo(): Task<T>` — the caller receives a `Task<T>`.
- `await expr` inside an async body desugars to `perform Async.await(expr)`.
- The `/ Async` effect exists only inside the Task's computation, not on the caller's signature.
- `Task<T>` stores private execution state. Before the first poll it holds an initial entry state; after suspension it may hold an internal continuation; after completion it holds the final result.
- The language contract does **not** require a dedicated executor ABI or task-specific codegen for `Task<T>`. An implementation may realize `Task<T>` as an ordinary object/class plus private suspended-state carriers, so long as the `poll()` contract is preserved.
- This stage does **not** define a public `scoop.task` executor package or adapter API. Any helper used to create or drive tasks before `poll()` is finalized is implementation-internal.
- `Task.poll()` starts or resumes the task and runs it until the task either completes (`Ready(value)`) or suspends again (`Pending`).
- If a resumed task suspends again through an escape-continuation handler, that handler captures a fresh continuation and stores it back into the task's private state. The previous continuation remains consumed (one-shot).
- Direct access to those internal continuations is not part of the common task API.
- The exact executor or wakeup mechanism, if any, is intentionally out of scope for this stage.
- When async/reactor integration eventually needs a long-lived wake token across safepoints, that token should be a stable GC handle (§15.10.1), not a pinned task reference. The task/object needs to stay alive, but it does not need to remain pinned while native code merely stores an opaque identity token.

#### spawn (structured concurrency)

`spawn` and the broader structured-concurrency / executor framework are intentionally deferred to a later stage.

This stage fixes:

- the `Async.await` effect operation,
- the lazy, manually-pollable `Task<T>` core abstraction,
- and the continuation semantics needed to implement it.

It does **not** yet fix:

- a standard executor interface,
- wakeup registration,
- queueing or work-stealing strategy,
- or the final desugaring of `spawn`.

#### Generator / yield (library-level, no dedicated syntax)

Generators use immediate-resume handlers:

```kotlin
effect Emit<T> {
    fun emit(value: T): Unit
}

fun fibonacci(): Sequence<Int> / Emit<Int> {
    var a = 0; var b = 1
    while (true) {
        perform Emit.emit(a)
        val next = a + b; a = b; b = next
    }
}

// Consumer uses immediate-resume handler:
handle {
    fibonacci()
} with {
    Emit.emit(value) -> resume {
        println(value)
        resume(Unit)
    }
}
```

#### Custom Effects

```kotlin
effect Logger {
    fun log(level: LogLevel, msg: String): Unit
}

fun process() / Logger {
    perform Logger.log(Info, "starting")
}

// Non-resuming handler:
handle {
    process()
} with {
    Logger.log(level, msg) -> println("[" + level + "] " + msg)
}

// Immediate-resume handler:
handle {
    process()
} with {
    Logger.log(level, msg) -> resume {
        println("[" + level + "] " + msg)
        resume(Unit)
    }
}

// Escape-continuation handler (e.g., buffered logging):
handle {
    process()
} with {
    Logger.log(level, msg), k -> {
        buffer.add("[" + level + "] " + msg)
        k.resume()
    }
}
```

### 5.8 Effect Rows

An **effect row** is a compile-time description of which effects a computation may perform.

#### 5.8.1 Row expressions

Effect annotations use a single row expression:

- `Pure` — the empty effect row (no effects)
- `Async`, `Raise<IOError>` — effect items
- `E` — an effect row variable (introduced by `<eff E>`; see §3.4)
- `R1 + R2` — union of rows

`+` is associative, commutative, and idempotent; `Pure` is the identity.

For readability, parentheses may be used: `/ (Async + Raise<IOError>)`.

#### 5.8.2 Default effect

If an effect annotation is omitted, the default effect row is `Pure`.

- For public functions, this default is enforced (`/ Pure`) and any non-`Pure` required effect in the body is a compile error.
- For private/internal functions, the compiler may infer a non-`Pure` effect row from the function body when the annotation is omitted (see §14.7).

#### 5.8.3 Row containment (subeffecting)

Effect rows form a partial order `⊆` ("requires no more effects than"):

- `Pure ⊆ R` for all `R`
- `R ⊆ (R + S)` for all `R, S`
- `R1 + R2` is the least upper bound (union) of `R1` and `R2`

Type checking rule:

- A call to a function with required effects `Req` is valid in a context that permits effects `Ctx` only if `Req ⊆ Ctx`.

This is a static check; effect rows do not exist at runtime.

#### 5.8.4 Open vs closed rows

By default, a declared effect row is **open** (additive): the annotation is a **lower bound**, stating which effects this function itself may perform. It does not restrict transitive effects introduced by higher-order function parameters. This avoids the Java checked-exception mistake, where callers must enumerate all effects propagated through callbacks.

```kotlin
// open row: map itself performs no effects, but the mapper may perform anything
fun <T, R, eff E = Pure> map(xs: List<T>, f: (T) -> R / E): List<R> / E
```

A **closed** effect row is written with a `!` suffix on the row expression. It is an **upper bound**, guaranteeing that no effects beyond those listed are observable at the call site. Closed rows are rare and appear primarily at system boundaries.

```kotlin
// closed row: main promises to the OS that no effects escape
fun main(): Unit / Pure!

// FFI wrapper: closed row prevents unsafe effects from leaking into the type system
extern fun ffi_read_file(path: String): Bytes / IO!
```

Syntax:

```
effectRow = effectList '!'?
effectList = effect ('+' effect)*
```

The `!` suffix applies to the entire row expression, not to the last effect item. For a single effect, parentheses are not needed: `/ IO!`. For multiple effects: `/ IO+State!`.

Semantics:

- An open row `/ R` means: the body's required effects `R_body` must satisfy `R_body ⊆ R` (lower-bound check; transitive effects from callbacks are not constrained at the declaration).
- A closed row `/ R!` means: **additionally**, any effect observable from outside this function must be a member of `R`. The compiler checks that no unhandled effects from any source (including callbacks) can escape the function boundary without being handled.
- `Pure!` is the most common closed row and is equivalent to: "this function is total and performs no observable effects". Entry points (§5.10) are implicitly `/ Pure!`.

### 5.9 Effect Polymorphism

Generic declarations may quantify over effect rows using `<eff E>` (see §3.4). Effect row variables may appear anywhere an effect row expression is expected (after `/`).

Example (uses effectful function types; see §7.5):

```kotlin
interface Disposable<eff E = Pure> {
    fun dispose(): Unit / E
}

fun <T, eff E = Pure> using(d: Disposable<eff E>, body: () -> T / E): T / E {
    try {
        return body()
    } finally {
        d.dispose()
    }
}
```

#### 5.9.1 Overriding

When overriding a method, the overriding method must not require more effects than the overridden signature (after substituting the receiver's effect row arguments). Formally, the overriding row `R_over` must satisfy `R_over ⊆ R_base`.

This allows an implementation to be "more pure" than the interface it implements.

### 5.10 Program Boundary (Entry Points)

An entry point is a function invoked by the Scoop runtime without an explicit call site in user code (implementation-defined; standalone executables typically use a `main` function).

The Scoop runtime invokes the program's entry point(s) with **no ambient effect handlers**. As a result, entry points must not require any effects:

- The effect row of an entry point is required to be `Pure!` (closed pure row; see §5.8.4).
- If an entry point declares any effect row other than `Pure!` (including open `/ Pure`), it is a compile error.
- If an entry point omits an effect annotation, it is treated as `/ Pure!` and any unhandled effect performed by the body is a compile error (see §14.7).

In other words, effects may be used inside an entry point only if they are handled within the entry point (directly via `handle` / `try`, or indirectly by calling library code that installs handlers).

## 6. Compile-time Evaluation and Static Reflection

### 6.1 Overview

Scoop provides compile-time evaluation capabilities through `const fun` (compile-time evaluable functions) and `comptime` blocks (compile-time execution contexts). Combined with compiler-intrinsic reflection functions, this enables type-safe code generation without a separate macro system.

### 6.2 `const fun`

A function marked `const` can be evaluated at compile time. It can also be called at runtime — `const` is a capability marker, not a restriction.

```kotlin
const fun add(a: Int, b: Int): Int = a + b

// Compile-time evaluation
const val X = add(1, 2)  // evaluated to 3 at compile time

// Runtime call — also valid
val y = add(a, b)
```

#### Allowed in `const fun`:

- Calling other `const fun`
- Calling compiler intrinsic functions (`fieldsOf`, `nameOf`, `sizeOf`, etc.)
- Local `var` / `val` bindings
- Control flow: `if`, `when`, `for`, `while`
- All value type operations (arithmetic, string operations, struct/tuple/enum construction)
- `String` operations — although `String` is a reference type at runtime, it has value-type semantics (immutable, content-equality) and is specially handled during compile-time evaluation
- `comptime for` / `comptime if`

#### Prohibited in `const fun`:

- Calling non-`const` functions
- Any effect (IO, Async, Raise, etc.)
- Accessing global mutable state
- Creating reference type instances (class instances require heap allocation, unavailable at compile time) — exception: `String` is allowed (see above)
- Closures / lambdas (capturing environment makes compile-time analysis intractable — whether a closure fully satisfies `const` semantics depends on its capture set, which is difficult to verify statically)

### 6.3 `comptime` Blocks

`comptime for` and `comptime if` are compile-time control flow constructs. They are evaluated (unrolled/resolved) during compilation.

#### `comptime for`

Iterates over compile-time lists (e.g., type fields, enum variants). The loop body is unrolled at compile time, generating one copy per iteration.

```kotlin
fun <T> debugPrint(value: T) {
    print(f"{nameOf<T>()}(")
    comptime for (field in fieldsOf<T>()) {
        print(f"{field.name}={value.[field]}, ")
    }
    println(")")
}

// For debugPrint<Point>, compiler unrolls to:
// fun debugPrint_Point(value: Point) {
//     print("Point(")
//     print("x=${value.x}, ")
//     print("y=${value.y}, ")
//     println(")")
// }
```

#### `comptime if`

Compile-time conditional. Branches not taken are eliminated entirely — they are not type-checked.

```kotlin
fun <T> serialize(value: T): ByteArray {
    comptime if (T is struct) {
        // struct serialization logic
        ...
    } else comptime if (T is enum) {
        // enum serialization logic
        ...
    } else {
        // fallback
        ...
    }
}
```

### 6.4 Compiler Intrinsic Reflection Functions

The following functions are compiler builtins. They are implicitly `const` and return compile-time data structures.

#### Type Introspection

```kotlin
const fun fieldsOf<T>(): ComptimeList<FieldMeta>       // struct/class fields
const fun variantsOf<T>(): ComptimeList<VariantMeta>    // enum variants
const fun nameOf<T>(): String                            // type name
const fun sizeOf<T>(): Int                               // size in bytes
const fun alignOf<T>(): Int                              // alignment in bytes
const fun superTypesOf<T>(): ComptimeList<TypeMeta>     // parent types/interfaces
const fun annotationsOf<T>(): ComptimeList<AnnotationMeta>  // type-level annotations (§15.6)
const fun paramsOf(fn: FunctionMeta): ComptimeList<ParamMeta>  // function parameters
```

#### Platform Introspection

The sysroot defines a `Platform` value type that represents a target platform using an LLVM triple and its decomposed components:

```kotlin
struct Platform {
    val triple: String
    val arch: String
    val vendor: String
    val os: String
    val env: String
}

// Available at both comptime and runtime.
const fun getPlatform(): Platform
```

Semantics:

- When evaluated at **compile time** (e.g. inside `comptime { ... }`, or in a `const val` initializer), `getPlatform()` returns the **compilation target** platform.
- When called at **runtime**, `getPlatform()` returns the platform of the **current execution environment**.

The canonical format of `Platform.triple` follows the LLVM target triple conventions (implementation-defined validation).

#### Compile-time Metadata Types

```kotlin
struct FieldMeta {
    val name: String,
    val type: TypeMeta,
    val index: Int,
    val annotations: ComptimeList<AnnotationMeta>   // field-level annotations (§15.6)
}

struct VariantMeta {
    val name: String,
    val fields: ComptimeList<FieldMeta>,
    val index: Int,
    val annotations: ComptimeList<AnnotationMeta>   // variant-level annotations (§15.6)
}

struct TypeMeta {
    val name: String,
    val kind: TypeKind,   // Struct, Enum, Class, Interface, Tuple, Primitive
    val annotations: ComptimeList<AnnotationMeta>   // type-level annotations (§15.6)
}

struct ParamMeta {
    val name: String,
    val type: TypeMeta,
    val index: Int,
    val annotations: ComptimeList<AnnotationMeta>   // parameter-level annotations (§15.6)
}

struct FunctionMeta {
    val name: String,
    val params: ComptimeList<ParamMeta>,
    val returnType: TypeMeta,
    val annotations: ComptimeList<AnnotationMeta>   // function-level annotations (§15.6)
}
```

`ComptimeList<T>` is a compile-time-only list type. It exists only during compilation and cannot appear in runtime code.

`AnnotationMeta` and `AnnotationArgMeta` are defined in §15.6.

#### Splice Operator

The `.[field]` operator accesses a value's field using a compile-time `FieldMeta` reference:

```kotlin
comptime for (field in fieldsOf<T>()) {
    val fieldValue = value.[field]   // access field dynamically at compile time
}
```

This is only valid inside `comptime for` blocks where `field` is a `FieldMeta` from the current iteration.

#### Runtime Fallback (No Special-Case)

Reflection intrinsics are `const fun` (§6.2): they may be evaluated at compile time when their inputs are compile-time constants, and otherwise remain normal runtime calls.

- If evaluated in a compile-time context (e.g. a `const val` initializer), the compiler executes the intrinsic and embeds the result.
- If the receiver / inputs are runtime values, the call falls back to a normal runtime call.
- This is not special-cased for reflection; it follows the standard `const fun` evaluation rules from §6.2.

```kotlin
struct Point(val x: Int)

const val CT: String = nameOf<Point>()      // compile-time evaluation

fun runtime(p: Point): String {
    val rt: String = nameOf<Point>()        // normal runtime call (semantically the same)
    return rt
}
```

### 6.5 Complete Example: Generic JSON Serialization

```kotlin
const fun formatFieldName(name: String): String = f"\"{name}\""

fun <T> toJson(value: T): String {
    comptime if (T is struct) {
        return buildString {
            append("{")
            var first = true
            comptime for (field in fieldsOf<T>()) {
                if (!first) append(",")
                first = false
                append(formatFieldName(field.name))
                append(":")
                append(toJson(value.[field]))
            }
            append("}")
        }
    } else comptime if (T is enum) {
        comptime for (variant in variantsOf<T>()) {
            when (value) {
                variant -> {
                    return buildString {
                        append(f"{{\"type\":\"{variant.name}\"")
                        comptime for (field in variant.fields) {
                            append(f",\"{field.name}\":")
                            append(toJson(value.[field]))
                        }
                        append("}")
                    }
                }
            }
        }
    } else comptime if (T == String) {
        return f"\"{value}\""
    } else comptime if (T == Int || T == Long || T == Float || T == Double) {
        return value.toString()
    } else comptime if (T == Bool) {
        return if (value) "true" else "false"
    }
}
```

### 6.6 Runtime Type Info (RTTI)

Scoop provides minimal runtime type information for reference types only:

```kotlin
val name = someObj.typeName          // runtime type name (reference types only)
someObj is User                      // runtime type check (reference types only), triggers smart cast (§4.3)
someObj as User                      // unsafe cast (§4.4)
someObj as? User                     // safe cast, returns User? (§4.4)
```

Full runtime reflection (dynamic field access, dynamic method invocation, dynamic instance creation) is **not provided**. Use compile-time reflection and `const fun` for all code generation needs.

## 7. Functions

### 7.1 Declaration

```kotlin
fun add(a: Int, b: Int): Int = a + b

fun greet(name: String): String {
    return "Hello, $name!"
}
```

Function syntax follows Kotlin.

### 7.2 `inline`

`inline` is an optimization hint only. The compiler may choose to inline or not. It has **no semantic effect** — there is no non-local return, and no special treatment of lambda parameters.

```kotlin
inline fun <T> measure(block: () -> T): T {
    val start = now()
    val result = block()
    println(f"took {now() - start}ms")
    return result
}
```

### 7.3 Non-local Return

Non-local return is **not supported**. `return` always exits the immediately enclosing named function. Use `for` loops with `return` or `break` for early exit patterns.

### 7.4 Extension Functions

Extension functions add new functions to existing types without modifying their declaration. The receiver type is specified before the function name with a dot.

```kotlin
fun String.wordCount(): Int {
    if (this.isEmpty()) return 0
    return this.split(" ").size
}

fun <T> List<T>.secondOrNull(): T? {
    return if (this.size >= 2) this[1] else null
}

// Usage — called as if it were a member
val count = "hello world".wordCount()    // 2
val second = listOf(1, 2, 3).secondOrNull()  // Some(2)
```

- Extension functions are **statically dispatched** — the function called is determined by the declared type of the receiver, not the runtime type.
- Extensions can be defined on any type: reference types, value types (struct, enum, tuple), and even nullable types (`T?`).
- Extensions cannot access `private` or `internal` members of the receiver type.
- If an extension has the same signature as a member function, the **member always wins** — extension is shadowed.
- Extensions can be generic and can declare effects.
- Extensions are importable: `import scoop.collections.secondOrNull`.

```kotlin
// Extension on value type
fun Point.distanceTo(other: Point): Double {
    val dx = (this.x - other.x).toDouble()
    val dy = (this.y - other.y).toDouble()
    return sqrt(dx * dx + dy * dy)
}

// Extension on nullable type
fun String?.orEmpty(): String {
    return this ?: ""
}

// Extension with effect
fun String.readAsFile(): String / (Async + Raise<IOError>) {
    return open(this).readAll()
}
```

**Compilation:** Extension functions are compiled as regular static functions with the receiver as the first parameter. `fun String.wordCount(): Int` becomes `fun wordCount(receiver: String): Int` in the generated code.

### 7.5 Function Types and Effects

Function types are written with `->`:

- `(A, B) -> C` — a function that takes `(A, B)` and returns `C`

Receiver function types (Kotlin-style) are written by prefixing the receiver type and a dot:

- `T.(A, B) -> C` — a function that can be called with receiver `T` and parameters `(A, B)`

Inside a receiver lambda, `this` has type `T`.

A function type may optionally declare an effect row after `/`:

- `(A, B) -> C / R`
- `T.(A, B) -> C / R`

The effect row syntax supports both open and closed rows (see §5.8.4). For example:

- `(Int) -> String / Pure` (open row)
- `(Int) -> String / Pure!` (closed row)

If omitted, the effect row is `Pure`.

Examples:

```kotlin
val pure: () -> Int = { 1 }                          // () -> Int / Pure
val io: () -> String / Raise<IOError> = { ... }

fun <T, eff E> run(block: () -> T / E): T / E {
    return block()
}
```

#### Effect subtyping for function types

For two function types with the same parameter and return types:

If `R1 ⊆ R2` then `(A) -> B / R1` is a subtype of `(A) -> B / R2`.

This allows a pure function to be passed where a more effectful function is expected.

#### Effect erasure and casts to `Any`

Effect rows are **compile-time only** and are not preserved at runtime (see §5.8.3). In particular, the runtime representation of a function value does not carry enough information to reliably enforce or validate its declared effect row.

As a consequence, casting/boxing a function value to `Any` is only permitted when the function type has the **closed** effect row `Pure!`:

- ✅ allowed: `((A) -> B / Pure!) as Any` (and implicit upcasts/boxing to `Any`)
- ❌ forbidden: `((A) -> B / R)` for any non-`Pure!` row `R` (including open `/ Pure`)

Rationale: once the value is erased to `Any`, the effect row cannot be recovered or checked at runtime; allowing effectful function values to be stored as `Any` would permit calling them later without the required effect handling, leading to runtime failures.

Example:

```kotlin
val ok: (Int) -> String / Pure! = { x -> x.toString() }
val a: Any = ok                  // ✅ ok

val bad: () -> Unit / Raise<RuntimeError> = {
    perform Raise.raise(RuntimeError.NullAssertionFailed)
}
val b: Any = bad                  // ❌ compile error: cannot erase effects into Any
```

### 7.6 Closure Literals and `do` Blocks

Scoop distinguishes ordinary local blocks from closure literals syntactically.

In ordinary expression position:

- A bare `{ ... }` is always a closure literal.
- A plain local block used for scoping or to compute a value must be written `do { ... }`.
- `TypeName { field: expr, ... }` remains a struct literal (§2.3.1); the type name / constructor before `{` disambiguates it from a closure literal.
- Constructs that introduce their own braced bodies (`if`, `when`, `handle`, `try`, `async`, `comptime`, declarations, etc.) are unaffected; this rule only governs ordinary brace-delimited expression/block forms.

Examples:

```kotlin
val f = { println("hello") }      // closure value
val u = do { println("hello") }   // evaluates immediately; u: Unit

val n = do {
    val base = 40
    base + 2
}

users.filter { it.age >= 18 }     // trailing lambda
combine(1) { it + 1 } { it + 2 }  // multiple trailing lambdas
consume(do { computeValue() })    // ordinary argument value, not a closure
combine(do { 3 }) { it + 1 } { it + 2 } // block arg + trailing lambdas
```

A `do { ... }` block may appear in statement position or wherever an expression is permitted. Its value is the final expression that is not terminated by `;`; if the block has no such tail expression, its value is `Unit`.

Closure bodies use the same tail-expression rule as `do` blocks. Therefore `{ 1 }` is a closure returning `1`, while `do { 1 }` is a block expression whose value is `1`.

Calls may consume one or more trailing lambdas. Only bare brace-delimited closure literals participate in this postfix syntax; an explicit `do { ... }` never becomes a trailing lambda. To pass the evaluated result of a local block to a call that also uses trailing lambdas, parenthesize the block as an ordinary argument (`combine(do { 3 }) { ... } { ... }`).

## 8. String Literals

### 8.1 Regular Strings

Regular strings have no interpolation. `$`, `{`, and `}` are literal characters.

```kotlin
val price = "costs $100"           // literal $
val json = "{ \"key\": \"value\" }"  // literal { }
val shell = "echo ${HOME}"         // literal ${HOME}
```

### 8.2 Interpolated Strings

Interpolated strings use the `f` prefix. Expressions are enclosed in `{ }`.

```kotlin
val greeting = f"Hello, {name}!"
val result = f"sum = {a + b}"
val info = f"user: {user.name}, age: {user.age}"

// Escape braces with doubling
val literal = f"use {{braces}} in f-string"  // outputs: use {braces} in f-string
```

### 8.3 Raw Strings

Multiline strings with no escape processing, using triple quotes:

```kotlin
val raw = """
    line 1
    line 2
""".trimIndent()

// Raw + interpolation
val raw = f"""
    Hello, {name}!
    Your balance is {balance}.
""".trimIndent()
```

### 8.4 `trimIndent()`

`trimIndent()` is a `const fun` on `String` that removes common leading whitespace from all lines and strips the first/last blank lines. It is the standard companion to raw strings.

```kotlin
val sql = """
    SELECT *
    FROM users
    WHERE id = 1
""".trimIndent()
// Result: "SELECT *\nFROM users\nWHERE id = 1"
```

- When the receiver is a **compile-time constant** (string literal, `const val`, `const fun` return value), `trimIndent()` is evaluated at compile time — zero runtime cost.
- When the receiver is a **runtime value**, it falls back to a normal runtime call.
- This is not special-cased in the compiler — it follows the standard `const fun` evaluation rules from Section 6.2.

## 9. Variable Binding

```kotlin
val x: Int = 10     // immutable binding — cannot reassign x
var y: Int = 10     // mutable binding — can reassign y
y = 20              // ✅ rebinds y to new value 20
```

- `val`: the binding is immutable (cannot be reassigned).
- `var`: the binding is mutable (can be reassigned to a new value).
- Value types themselves are always immutable. `var` allows the **slot** to be reassigned, not the value to be mutated.

## 10. Properties

### 10.1 Basic Properties

Properties in Scoop follow Kotlin semantics. In classes, properties can have custom accessors (getters and setters). In structs and enums (value types), only computed properties (getter-only) are allowed since value types are immutable.

```kotlin
class User(private var _name: String, private var _age: Int) {
    // Property with default getter and setter (backing field)
    var email: String = ""

    // Property with custom getter
    val displayName: String
        get() = f"{_name} (age {_age})"

    // Property with custom getter and setter
    var name: String
        get() = _name
        set(value) {
            require(value.isNotEmpty()) { "Name cannot be empty" }
            _name = value
        }

    // Read-only computed property (no backing field)
    val isAdult: Bool
        get() = _age >= 18
}
```

**Backing field (`field`):**

When a property has a default accessor or its custom accessor references `field`, the compiler generates a backing field. The `field` identifier is only available inside accessors.

```kotlin
var counter: Int = 0
    set(value) {
        if (value >= 0) field = value    // `field` refers to the backing field
    }
```

If neither accessor references `field`, no backing field is generated — the property is purely computed.

### 10.2 Properties on Value Types

Structs and enums can have computed properties (getter-only). Since value types are immutable, setters are not allowed.

```kotlin
struct Point(val x: Int, val y: Int) {
    val magnitude: Double
        get() = sqrt((x * x + y * y).toDouble())

    val isOrigin: Bool
        get() = x == 0 && y == 0
}

enum Shape {
    Circle(val radius: Int),
    Rect(val width: Int, val height: Int)

    val area: Int
        get() = when (this) {
            Circle(r) -> r * r * 3
            Rect(w, h) -> w * h
        }
}
```

### 10.3 Extension Properties

Properties can be added to existing types via extension syntax, analogous to extension functions (§7.4). Extension properties cannot have backing fields — they must be computed.

```kotlin
val String.lastChar: Char
    get() = this[this.length - 1]

val <T> List<T>.lastIndex: Int
    get() = this.size - 1

// Mutable extension property (reference types only, no backing field)
var StringBuilder.lastChar: Char
    get() = this[this.length - 1]
    set(value) { this[this.length - 1] = value }
```

**Compilation:** Extension properties compile to static getter (and setter) functions with the receiver as the first parameter, identical to extension functions.

### 10.4 Delegated Properties

Delegated properties offload getter/setter logic to a delegate object using the `by` keyword. This eliminates boilerplate for cross-cutting property patterns like lazy initialization, observable changes, and map-backed storage.

```kotlin
val heavyData: List<Data> by lazy {
    loadFromDatabase()    // computed on first access, then cached
}

var name: String by observable("unnamed") { old, new ->
    println(f"name changed: {old} -> {new}")
}
```

#### Delegate Interface

A delegate object must implement `getValue` (and `setValue` for `var` properties):

```kotlin
interface ReadOnlyProperty<in T, out V> {
    fun getValue(thisRef: T, property: PropertyMeta): V
}

interface ReadWriteProperty<in T, V> : ReadOnlyProperty<T, V> {
    fun setValue(thisRef: T, property: PropertyMeta, value: V)
}
```

`PropertyMeta` is a compile-time metadata struct (similar to `FieldMeta` in §6.4) that provides the property name, type, and other metadata. The compiler passes it automatically.

#### How Delegation Works

```kotlin
class Example {
    var text: String by TrimDelegate()
}

// The compiler generates:
class Example {
    private val text$delegate = TrimDelegate()

    var text: String
        get() = text$delegate.getValue(this, /* PropertyMeta for text */)
        set(value) { text$delegate.setValue(this, /* PropertyMeta for text */, value) }
}
```

#### Standard Delegates

Scoop provides standard delegates in `scoop.delegates`:

```kotlin
// lazy — compute on first access, thread-safe by default
val config: Config by lazy { loadConfig() }

// lazy with mode
val config: Config by lazy(LazyThreadSafetyMode.None) { loadConfig() }

// observable — callback on every change
var score: Int by observable(0) { old, new ->
    println(f"score: {old} -> {new}")
}

// vetoable — callback can reject the change
var age: Int by vetoable(0) { old, new ->
    new >= 0    // reject negative values
}

// map-backed — property values stored in a map
class JsonObject(private val data: Map<String, Any>) {
    val name: String by data
    val age: Int by data
}
```

#### Concurrency Notes (Standard Delegates)

In Scoop 0.1, `lazy`, `observable`, and `vetoable` are currently implemented as **compiler-known delegates** (an early-stage strategy) rather than allocating a runtime delegate object. In particular, for `observable` / `vetoable`, the compiler injects a hidden **per-property mutex** to protect the backing field.

Minimal concurrency contract (host pthread backend):

- Backing field reads/writes are synchronized by the injected mutex (`lock`/`unlock` act as acquire/release boundaries).
- `observable`: the backing field is updated first, then `onChange(old, new)` is invoked **after** the write and **outside** the mutex. If `onChange` raises, the value remains updated.
- `vetoable`: `onChange(old, new)` is invoked **before** the write and **outside** the mutex. If it returns `false` (or raises), the value is not updated.

Because callbacks are invoked outside the lock, callbacks from multiple threads may interleave. Reentrant reads/writes from within a callback are permitted but may observe such interleavings.

#### Delegated Properties and Value Types

Delegated properties are available for **reference types** (classes) only. Value types (struct, enum) are immutable and have no identity, so delegation (which requires storing a delegate object alongside the property) does not apply. Use computed properties (§10.2) for value types instead.

## 11. Scope Functions

Scoop retains most Kotlin scope functions except `with`:

| Function | Receiver | Return |
|---|---|---|
| `let` | `it` | lambda result |
| `run` | `this` | lambda result |
| `also` | `it` | context object |
| `apply` | `this` | context object |

Kotlin's `with(obj) { ... }` is removed — use `obj.run { ... }` instead. The `with` keyword is reserved for value type update expressions.

Scope functions are library-defined and are expected to be **effect-polymorphic**: effects performed inside the lambda contribute to the scope function call's required effect row (and therefore to the caller) unless handled by an enclosing `handle` / `try` block.

Typical signatures (illustrative):

```kotlin
fun <T, R, eff E = Pure> T.let(block: (T) -> R / E): R / E
fun <T, R, eff E = Pure> T.run(block: T.() -> R / E): R / E
fun <T, eff E = Pure> T.also(block: (T) -> Unit / E): T / E
fun <T, eff E = Pure> T.apply(block: T.() -> Unit / E): T / E
```

## 12. Disambiguation: Struct Literal vs Closure vs `do` Block

In ordinary expression position, Scoop does not infer a plain local block from bare braces. The forms are distinguished syntactically:

- **Struct literal**: `TypeName { identifier: expr, ... }` — brace body follows a type/constructor name and contains field-value pairs.
- **Closure**: `{ params -> body }` or `{ body }` — bare braces, optionally with a parameter list / `->`.
- **Local block**: `do { ... }` — explicit `do` introducer.

```kotlin
Point { x: 1, y: 2 }            // struct literal
val f = { println("hello") }    // closure
val u = do { println("hello") } // local block, result is Unit

list.map { it * 2 }             // trailing lambda (closure)
list.map { x: Int -> x * 2 }    // trailing lambda with typed parameter
combine(1) { it + 1 } { it + 2 } // multiple trailing lambdas
```

Because plain local blocks require `do`, `val a = { println("hello") }` is always interpreted as assigning a closure. To assign the evaluated block result, write `val a = do { println("hello") }`.
Likewise, `combine(1) { ... } { ... }` is still one call with multiple trailing lambdas; a following `do { ... }` starts the next expression/statement unless it is parenthesized as an ordinary argument.

## 13. Package System (Cone)

### 13.1 Overview

A Scoop package is called a **Cone**. A compiled `.cone` file is a binary archive (similar to `.a` / `.rlib`) designed for binary distribution. Unlike Rust's `.rlib` (which is tied to rustc internals), the `.cone` format has a **stable IR specification** enabling cross-version compatibility, similar to JVM bytecode stability.

### 13.2 Cone File Structure

```
example.cone (archive)
├── CONE_META              # Package metadata (name, version, deps, target, ir_version)
├── api.scoopir            # Public API signatures + type definitions (non-executable)
├── generics.scoopir       # Generic / const fun bodies in Scoop IR (non-executable)
├── sort.o                 # Non-generic function — precompiled machine code
├── hash.o                 # Non-generic function — precompiled machine code
├── list_int.o             # Pre-specialized List<Int> (optional)
├── list_string.o          # Pre-specialized List<String> (optional)
└── ...
```

| Component | Purpose | Executable? |
|---|---|---|
| `CONE_META` | Package info, dependency resolution | No |
| `api.scoopir` | Type checking, IDE support, comptime metadata (FieldMeta etc.) | No |
| `generics.scoopir` | Consumer compiler monomorphizes generics and evaluates `const fun` from this | No |
| `*.o` (non-generic) | Directly linkable machine code | Yes |
| `*.o` (pre-specialized) | Pre-compiled common monomorphization instances | Yes |

### 13.3 Consumer Compilation and Linking

When a project depends on a `.cone`:

```
Consumer compiler
│
├── 1. Read api.scoopir → type check, resolve imports
│
├── 2. For each generic used by consumer:
│   ├── Check if .cone has a pre-compiled .o for this type combination
│   │   ├── Yes → use it directly (e.g., List<Int> → list_int.o)
│   │   └── No  → read IR from generics.scoopir, monomorphize, compile to new .o
│   └── Evaluate any const fun from generics.scoopir as needed
│
├── 3. Link
│   ├── Pre-compiled .o files from .cone (direct link, no recompilation)
│   ├── Newly generated .o files (from monomorphization)
│   └── Consumer's own .o files
│
└── 4. Output final binary
```

For well-maintained cones that pre-compile common type combinations (`Int`, `String`, `Long`, `Bool`, etc.), most consumers link almost entirely from pre-compiled `.o` files — approaching JVM-level "download and use" experience.

### 13.4 IR Stability

```toml
# Cone.toml
[cone]
name = "scoop-collections"
version = "1.0.0"
scoop = "0.1.0"
ir_version = 1
```

- The Scoop IR format has an explicit version number with backward compatibility guarantees.
- Newer compilers can always read older IR versions.
- This is the key enabler for binary distribution — analogous to JVM bytecode version stability.
- LLVM Bitcode may optionally be included alongside Scoop IR for cross-cone LTO (Link-Time Optimization).

### 13.5 Compilation Speed

Monomorphization requires downstream compilation for generic code. Mitigations:

- **Pre-compiled specializations**: Cone authors pre-compile common type combinations
- **Incremental compilation**: Only re-compile affected monomorphization instances on change
- **Parallel monomorphization**: Each specialization is independent, naturally parallelizable
- **Direct .o linking**: Non-generic code never recompiled, just linked

### 13.6 Package Declaration and Imports

```kotlin
// Package declaration (Kotlin-like)
package scoop.collections

// Imports
import scoop.collections.List
import scoop.collections.buildList
import scoop.io.*

// Visibility modifiers
public fun <T> sort(list: List<T>): List<T> { ... }     // public API
internal fun <T> partition(list: List<T>): ... { ... }    // visible within cone
private fun helper() { ... }                               // visible within file
```

### 13.7 Cone Manifest

```toml
# Cone.toml
[cone]
name = "scoop-http"
version = "2.1.0"
scoop = "0.1.0"
ir_version = 1
license = "MIT"

[dependencies]
scoop-core = "1.0.0"
scoop-collections = "1.0.0"
scoop-io = "1.2.0"

[pre-specialize]
# Instruct the compiler to pre-compile these monomorphization instances
types = [
    "List<Int>", "List<String>", "List<Bytes>",
    "Map<String, String>", "Map<String, Int>",
    "Result<Bytes, IOError>"
]

[targets]
platforms = ["linux-x64", "macos-arm64", "windows-x64"]

# Optional platform-based source selection rules.
# (Extensible: future versions may add feature flags and richer conditions.)
[[select]]
when = { platform = "windows-x64" }
include = ["src/platform/windows/**.scoop"]
exclude = ["src/platform/posix/**.scoop"]
```

The `[pre-specialize]` section lets cone authors declare which type combinations to pre-compile, minimizing downstream compilation work for consumers.

### 13.8 Cone Source Layout

By convention, a cone's source files live under a `src/` directory:

```
MyCone/
├── Cone.toml
└── src/
    ├── main.scoop
    └── ...
```

- `Cone.toml` is located at the cone root.
- Source discovery defaults to `src/**/*.scoop`.
- The conventional entry file for executable cones is `src/main.scoop` (the actual entry point is still the program boundary `main` function; see §5.10).

### 13.9 Platform-based Selection in `Cone.toml`

Some cones need to include different source sets for different target platforms (e.g. POSIX vs Windows). Scoop supports a platform-based selector mechanism in `Cone.toml` to include or exclude source files based on the compilation target.

Design goals:

- **Manifest-driven** selection (no preprocessor in the language).
- **Extensible conditions** (today: platform; future: features, toolchains, environment predicates).
- **Deterministic source graph** for `.cone` packaging and IR stability.

Selector format (v0):

```toml
[[select]]
when = { platform = "linux-x64" }
include = ["src/platform/posix/**.scoop"]
exclude = ["src/platform/windows/**.scoop"]
```

Rules (v0):

- Each `[[select]]` entry is evaluated against the compilation target platform.
- If `when` matches, the `include` globs are added to the source set and the `exclude` globs are removed from the source set.
- If multiple selectors match, they are applied in file order.
- Exact glob syntax is implementation-defined, but it must be stable and platform-independent (forward slashes, UTF-8 paths).

## 14. Type Inference

Scoop uses a **constraint-based bidirectional type inference** system. It is not Hindley-Milner (which is incompatible with subtyping, overloading, and bounded polymorphism), but provides stronger inference than Kotlin in several areas.

### 14.1 Design Principle

- **Function signatures are API boundaries** — explicit types serve as documentation and are required for `.cone` binary distribution.
- **Function bodies are implementation details** — the compiler should infer as much as possible to reduce boilerplate.

### 14.2 Annotation Rules

| Position | Required? | Notes |
|---|---|---|
| Function parameter types | Yes | Always required, no exception |
| Public function return type | Yes | API boundary, serves as documentation |
| Private/internal function return type | No | Inferred from all `return` paths |
| Recursive function return type | Yes | Inference does not converge without it |
| Local variable (`val`/`var`) | No | Inferred from initializer expression |
| Lambda parameter types | No (usually) | Inferred from expected type context |
| Generic type arguments | No (usually) | Inferred from actual arguments and expected type |
| Public function effects | Yes (unless `Pure`) | API boundary; may omit only if the function is effect-free |
| Private/internal function effects | No | Inferred from `perform` operations in the body |

### 14.3 Local Variable Inference

```kotlin
val x = 42                      // Int
val name = "hello"              // String
val users = listOf(u1, u2)     // List<User>
val map = mutableMapOf("a" to 1)  // MutableMap<String, Int>
```

### 14.4 Lambda Parameter Inference

Lambda parameter types are propagated downward from the expected type:

```kotlin
val adults = users.filter { user -> user.age >= 18 }  // user: User inferred
val names = users.map { it.name }                      // it: User inferred

// Multi-parameter lambda
val comparator: (User, User) -> Int = { a, b -> a.age - b.age }  // a, b: User inferred
```

### 14.5 Generic Argument Inference

Generic type arguments are inferred from actual arguments and the expected type:

```kotlin
fun <T> listOf(vararg elements: T): List<T> = ...

val ints = listOf(1, 2, 3)           // T = Int
val strs = listOf("a", "b")          // T = String

fun <T> emptyList(): List<T> = ...
val names: List<String> = emptyList() // T = String (from expected type)
```

### 14.6 Return Type Inference

Private and internal functions with block bodies can omit the return type:

```kotlin
// Return type inferred as Int
private fun fibonacci(n: Int): Int {    // ← recursive, must annotate
    if (n <= 1) return n
    return fibonacci(n - 1) + fibonacci(n - 2)
}

// Return type inferred as String
internal fun greet(name: String) {      // ← non-recursive, can omit
    if (name.isEmpty()) return "Hello!"
    return f"Hello, {name}!"
}

// Public functions always require return type
fun process(data: Data): Result { ... } // ← public, must annotate
```

### 14.7 Effect Inference

For private and internal functions, effect signatures are inferred from the function body:

```kotlin
// Effects inferred as / (Async + Raise<IOError>)
private fun readConfig(path: String) {
    val file = open(path)           // performs Async
    val content = file.readAll()    // performs Async + Raise<IOError>
    parse(content)
}

// Public functions must declare effects explicitly
fun loadSettings(path: String): Settings / (Async + Raise<IOError>) {
    val config = readConfig(path)
    Settings.from(config)
}
```

#### 14.7.1 Required effects (unhandled effects)

During type checking, each expression has a **required effect row**: the minimal effect row needed to evaluate the expression to a value *without relying on an implicit ambient handler*.

Rules:

- `perform E.op(...)` requires the effect item `E` unless the operation is handled by an enclosing `handle { ... } with { ... }` (or `try`/`catch`) that has a matching arm for `E.op`.
- Calling a function `f` requires the effect row declared on `f` (after substituting any type/effect arguments).
- Invoking a function value (e.g., `block()`) requires the effect row declared on that function type (see §7.5).
- Language constructs that are specified to perform effects (e.g., `!!` and `as` performing `Raise.raise(RuntimeError.…)`) contribute those effects to the required effect row (see §5.7).
- Effects performed in handler arms contribute normally to the enclosing context (handler arms execute outside the handler instance’s dispatch scope; see Appendix A).

These rules implement **unhandled effect detection**: if an effect is not handled locally, it must be accounted for by the surrounding declared/inferred effect row.

#### 14.7.2 Lambda (closure) effect inference and checking

A lambda expression has a function type (§7.5). Its effect row is:

- **Checked** against the expected function type when one exists.
- **Inferred** from the lambda body when no expected effect row is available.

If a lambda appears in a context with an expected function type `(...)->T / R_expected`, then the lambda body must have required effects `R_body` such that `R_body ⊆ R_expected`. Otherwise the compiler reports an error.

If a lambda has no expected function type (or the expected effect row is not yet known), the compiler infers the lambda’s effect row as the minimal row that contains all unhandled effects performed by the lambda body.

#### 14.7.3 Effect row argument inference (`eff`)

Effect row arguments (from `eff` parameters; see §3.4) are inferred similarly to type arguments. When a call supplies a lambda argument to a parameter of function type `(...)->T / E`, the row argument for `E` is inferred from the lambda body’s required effects (subject to subeffecting constraints).

#### 14.7.4 Mismatches and errors

For a function (or lambda) with an explicit declared effect row `R_decl`, the compiler checks that the body’s required effects `R_body` satisfy `R_body ⊆ R_decl`. If not, it is a compile error.

For a **public** function that omits an effect annotation, `R_decl` is `Pure` (see §5.3), so any unhandled effect in the body is an error.

For a **private/internal** function that omits an effect annotation, the compiler infers `R_decl` from the body’s required effects.

For an **entry point** (see §5.10), the declared effect row is required to be `Pure!` (closed), even if an explicit effect annotation is present. Declaring any effect row other than `Pure!` on an entry point is a compile error.

Example:

```kotlin
fun noEffect(): Unit {
    // The inner closure performs Raise<String>.
    // If `run` is effect-polymorphic (as in §11), then the call to `run` also requires Raise<String>.
    someObj.run {
        perform Raise.raise("Error")
    }
}
```

Since `noEffect` omits an effect annotation, it is treated as `/ Pure` and this is a compile error unless the effect is handled:

```kotlin
fun noEffect(): Unit {
    try {
        someObj.run { perform Raise.raise("Error") }
    } catch (e: String) {
        println(e)
    }
}
```

Or unless the function declares the required effect:

```kotlin
fun noEffect(): Unit / Raise<String> {
    someObj.run { perform Raise.raise("Error") }
}
```

### 14.8 Subtype Unification

When branches produce different types, the compiler computes the **least upper bound (LUB)**:

```kotlin
open class Animal
class Dog : Animal
class Cat : Animal

val pet = if (cond) Dog() else Cat()   // type: Animal (LUB of Dog and Cat)
```

For value types without a common supertype, this is a compile error — explicit conversion is required.

### 14.9 Algorithm

The inference engine uses **constraint generation + solving**, not HM's Algorithm W:

1. Traverse the function body AST, assigning a fresh type variable `τ` to each expression.
2. Generate constraints from syntax:
   - Assignment/binding: `τ_rhs <: τ_lhs`
   - Function call: instantiate callee signature, `τ_arg <: τ_param`
   - If/when branches: `τ_branch <: τ_result` (LUB)
   - Lambda: propagate expected type downward to parameter types
3. Solve constraints via worklist algorithm.
4. Report errors for unsolved variables or conflicting constraints.

This approach handles subtyping, bounded polymorphism, and overload resolution in a unified framework.

## 15. Annotations

### 15.1 Overview

Annotations attach metadata to declarations (functions, types, fields, parameters, properties). They are used by the compiler for built-in behavior (`@intrinsic`, `@extern`, `@inline`, `@deprecated`) and by user code for compile-time metaprogramming via `comptime` (§6).

Annotations are declared as `annotation class` — a restricted form of class that acts as an immutable data container. Annotation instances exist only at compile time; they have no runtime representation.

### 15.2 Annotation Class Declaration

```kotlin
annotation class Deprecated(
    val message: String = "",
    val replaceWith: String = ""
)

annotation class Inline
annotation class Extern(val lib: String = "", val name: String = "")
annotation class CLayout(val aligned: Int = 0, val packed: Int = 0)
annotation class ThreadLocal
annotation class Global
annotation class CallingConvention(val name: String)
annotation class Safe
annotation class Intrinsic
annotation class NoGC
annotation class Unsafe
```

**Rules:**
- All parameters must be `val` (immutable).
- Allowed parameter types: `String`, `Int`, `Float`, `Bool`, enum values, `Array<T>` of the preceding types, and other annotation types.
- Parameters may have default values. Parameters without defaults are required at the use site.
- Annotation classes cannot have methods, computed properties, or implement interfaces.
- Annotation classes cannot be instantiated with constructor syntax outside of annotation position (`@Name(...)`).

Annotation class primary-constructor `val` parameters define the annotation's **member variables** (properties). At compile time, an annotation instance conceptually has a value for each such property, either from the use-site arguments or from the property's default value.

### 15.3 Annotation Usage

Annotations are placed before the declaration they modify, prefixed with `@`:

```kotlin
@Deprecated("Use newFoo() instead", replaceWith: "newFoo")
fun foo() { ... }

@Inline
fun fastPath(x: Int): Int = x * 2
```

**Argument syntax:** The first parameter may be passed positionally. All subsequent parameters must be named:

```kotlin
@Deprecated("old")                              // positional first arg
@Deprecated(message: "old", replaceWith: "new") // all named
@Deprecated("old", replaceWith: "new")          // mixed
```

**Annotation targets:** Annotations may appear on:
- **Functions and methods** (`fun`, including extension functions)
- **Types** (`struct`, `class`, `enum`, `interface`, `object`)
- **Fields** (struct/class/enum variant fields)
- **Properties** (`val`/`var` declarations and computed properties)
- **Function parameters** (including receiver parameters)
- **Type parameters** (generic type variables)
- **Local variables** (`val`/`var` inside function bodies)
- **Expressions** (e.g., `@Suppress("unchecked") value as T`)
- **Modules / files** (file-level annotations like `@AllowIntrinsic`)
- **Annotation classes** themselves (meta-annotations like `@Target`, `@Retention`)
- **Enum variants** (individual cases of an enum)
- **Constructor parameters** (when the annotation applies to the constructor, not the property)

**Use-site targets:** When a declaration maps to multiple underlying elements (e.g., a constructor `val` parameter is both a parameter and a property), an explicit use-site target prefix disambiguates:

```kotlin
class Config(
    @property:Serialization.Rename("db_url") val dbUrl: String,   // targets the property
    @param:Validated val port: Int                                 // targets the constructor param
)
```

Available use-site targets: `field:`, `property:`, `param:`, `get:`, `set:`, `file:`. If no use-site target is specified, the annotation applies to the primary target of the declaration (property for `val`/`var`, function for `fun`, etc.).

### 15.4 Namespaced Annotations

Annotations can be nested inside `object` declarations for logical grouping:

```kotlin
object Serialization {
    annotation class Rename(val key: String)
    annotation class Ignore
    annotation class Format(val pattern: String)
    annotation class DateFormat(val pattern: String = "ISO8601")
}

struct User {
    val name: String
    @Serialization.Rename("created_at")
    val createdAt: String
    @Serialization.Ignore
    val cacheKey: String
    @Serialization.DateFormat("yyyy-MM-dd")
    val birthDate: String
}
```

Namespaced annotations are referenced via dot-path syntax: `@Namespace.AnnotationName(args)`.

### 15.5 Built-in Annotations

The compiler recognizes the following annotations (declared in the `core` sysroot module):

| Annotation | Target | Description |
|---|---|---|
| `@Intrinsic` | Functions, types | Implementation provided by the compiler/runtime |
| `@Extern(lib?, name?)` | Functions, global variables | Links to an external symbol (function or variable), optionally specifying the library and symbol name (see §15.5.1). Treated as `@NoGC` and requires an unsafe context to call/access (see §15.8 and §15.9) |
| `@Deprecated(message, replaceWith)` | Any | Marks declaration as deprecated; compiler emits warning on use |
| `@Inline` | Functions | Hint to inline the function body at call sites |
| `@TailRec` | Functions | Asserts tail-call optimization; compiler error if not tail-recursive |
| `@AllowIntrinsic` | Modules/files | Permits `@Intrinsic` declarations in user code |
| `@Suppress(warnings...)` | Any | Suppresses specific compiler warnings |
| `@CLayout(aligned?, packed?)` | Structs | Forces C-compatible field layout for FFI and optionally customizes alignment/packing (see §15.5.2) |
| `@ThreadLocal` | Global variables | Declares a mutable global GC-free variable as thread-local storage (TLS) (see §15.5.3) |
| `@Global` | Global variables | Declares a mutable global GC-free variable as process-global storage (see §15.5.3) |
| `@CallingConvention(name)` | Functions, type aliases | Specifies the calling convention for FFI / function pointers (see §15.5.4) |
| `@NoGC` | Functions | Requires the function to be allocation-free (no GC-managed heap allocations) and restricts calls to other `@NoGC` / `@Extern` functions (see §15.8) |
| `@Unsafe` | Functions, expressions | Enables unsafe raw memory operations (pointer arithmetic, unchecked loads/stores) within an unsafe context (see §15.9) |
| `@Safe` | Functions, expressions | Marks a region as safe (unsafe primitives forbidden) even inside an enclosing unsafe context (see §15.9.5) |
| `@Target(targets...)` | Annotation classes | Restricts which declaration kinds an annotation can appear on |
| `@Retention(policy)` | Annotation classes | Controls whether annotation is available at comptime only or preserved in `.cone` |

The `@Target` annotation uses `AnnotationTarget` enum values:

```kotlin
enum AnnotationTarget {
    Function, Property, Field, Param, Type, Constructor,
    LocalVariable, Expression, Module, TypeParam, EnumVariant
}

@Target(AnnotationTarget.Field, AnnotationTarget.Property)
annotation class Column(val name: String)
```

#### 15.5.1 `@Extern` (External Symbols and Libraries)

`@Extern` declares that a function or global variable is defined outside of Scoop (typically in C) and will be resolved by the linker.

```kotlin
@Extern(lib = "mylib", name = "myfunc")
fun myFunc(): Int
```

Parameters:

- `name`: the symbol name in the native object/library. If omitted (empty string), the compiler uses the Scoop declaration name.
- `lib`: an optional native library name. If specified, the compiler must pass a link directive for that library when producing an executable (exact flags are implementation-defined; library search paths are out of scope for this specification).

`@Extern` may also be applied to global variables (GC-free types only):

```kotlin
@Extern(name = "errno")
@ThreadLocal
var errno: Int
```

Rules:

- An `@Extern` variable declaration must not have an initializer (it is a declaration, not a definition).
- Accessing an `@Extern` variable requires an unsafe context (see §15.9.1).
- `@Extern` may be combined with `@ThreadLocal` to refer to an externally-defined TLS variable.

#### 15.5.2 `@CLayout` (C-compatible Struct Layout)

`@CLayout` forces a `struct` to use a C-compatible field layout, and optionally customizes alignment and packing.

```kotlin
@CLayout(aligned = 4)
struct MyStruct {
    val field1: Int
    val field2: Byte
}
```

Parameters:

- `aligned`: minimum alignment in bytes for the struct (must be a power of two). If omitted or set to `0`, the compiler chooses the natural alignment based on the fields and target ABI.
- `packed`: maximum alignment in bytes for struct fields (similar to C `#pragma pack` / `packed`). If omitted or set to `0`, the compiler chooses the natural packing based on the fields and target ABI.

Rules:

- A `@CLayout` struct must be **GC-free** (it must not contain GC-managed references, directly or transitively).
- The computed layout must match the target C ABI rules for the given `aligned`/`packed` configuration.

#### 15.5.3 `@ThreadLocal` / `@Global` (Mutable Global GC-free Variables)

Scoop permits mutable global variables only when they are explicitly marked as either thread-local or process-global. This restriction makes the risk of race conditions and global mutable state more visible.

```kotlin
@ThreadLocal
var threadLocalCounter: Int = 0

@CLayout(aligned = 4)
struct MyStruct {
    val field1: Int
    val field2: Byte
}

@Global
var myStructInstance: MyStruct = MyStruct{ field1 = 42, field2 = 1 }
```

Rules:

- A mutable global variable (`var` at module/file scope) must have either `@ThreadLocal` or `@Global`, otherwise it is a compile error.
- Such variables must be **GC-free**.
- A `@ThreadLocal` variable has a distinct instance per OS thread (TLS).
- A `@Global` variable has a single instance shared by all threads.

Initialization of global variables is implementation-defined. This specification recommends restricting `@ThreadLocal`/`@Global` initializers to compile-time constants for the early runtime.

#### 15.5.4 `@CallingConvention` (Calling Convention)

`@CallingConvention(name)` specifies the calling convention used for a function or function pointer type.

```kotlin
@CallingConvention("stdcall")
@Extern(name = "MessageBoxA", lib = "user32")
fun messageBoxA(hwnd: UIntPtr, text: Ptr<Byte>, caption: Ptr<Byte>, flags: UInt32): Int
```

The set of supported calling convention names is implementation-defined. The default calling convention is the platform C ABI.

### 15.6 Compile-time Annotation Access

Annotations are accessible via compile-time reflection (§6) on **all annotatable elements**. Every compile-time metadata struct carries an `annotations` field:

```kotlin
// ── Intrinsics for querying annotations ──

// Annotations on a type (struct, class, enum, interface)
const fun annotationsOf<T>(): ComptimeList<AnnotationMeta>

// Annotations on a specific function (by reference)
const fun annotationsOf(fun: FunctionMeta): ComptimeList<AnnotationMeta>

// ── Metadata structs (see §6.4) — all carry annotations ──

struct FieldMeta {
    val name: String
    val type: TypeMeta
    val index: Int
    val annotations: ComptimeList<AnnotationMeta>
}

struct VariantMeta {
    val name: String
    val fields: ComptimeList<FieldMeta>
    val index: Int
    val annotations: ComptimeList<AnnotationMeta>
}

struct TypeMeta {
    val name: String
    val kind: TypeKind
    val annotations: ComptimeList<AnnotationMeta>
}

struct ParamMeta {
    val name: String
    val type: TypeMeta
    val index: Int
    val annotations: ComptimeList<AnnotationMeta>
}

struct FunctionMeta {
    val name: String
    val params: ComptimeList<ParamMeta>
    val returnType: TypeMeta
    val annotations: ComptimeList<AnnotationMeta>
}

// ── Annotation metadata ──

struct AnnotationMeta {
    val name: String                              // e.g. "Serialization.Rename"
    val args: ComptimeList<AnnotationArgMeta>     // resolved named arguments (including defaults)
}

struct AnnotationArgMeta {
    val name: String
    val value: Any   // compile-time constant value
}
```

**Querying annotations by element kind:**

| Element | How to access annotations |
|---|---|
| Type (struct/class/enum/interface) | `annotationsOf<T>()` |
| Field | `field.annotations` inside `comptime for (field in fieldsOf<T>())` |
| Enum variant | `variant.annotations` inside `comptime for (variant in variantsOf<T>())` |
| Function parameter | `param.annotations` inside `comptime for (param in paramsOf(fn))` |
| Function | `annotationsOf(fn)` where `fn` is a `FunctionMeta` |

**Checking for a specific annotation:**

```kotlin
const fun hasAnnotation(annotations: ComptimeList<AnnotationMeta>, name: String): Bool {
    comptime for (ann in annotations) {
        comptime if (ann.name == name) { return true }
    }
    return false
}

const fun getAnnotation(annotations: ComptimeList<AnnotationMeta>, name: String): AnnotationMeta? {
    comptime for (ann in annotations) {
        comptime if (ann.name == name) { return ann }
    }
    return null
}
```

**Example: annotation-driven serialization**

```kotlin
fun <T> toJsonKey(field: FieldMeta): String {
    comptime for (ann in field.annotations) {
        comptime if (ann.name == "Serialization.Rename") {
            return ann.args[0].value as String
        }
    }
    return field.name  // default: use field name
}

fun <T> shouldSerialize(field: FieldMeta): Bool {
    comptime for (ann in field.annotations) {
        comptime if (ann.name == "Serialization.Ignore") {
            return false
        }
    }
    return true
}

fun <T> toJson(value: T): String {
    comptime if (T is struct) {
        return buildString {
            append("{")
            var first = true
            comptime for (field in fieldsOf<T>()) {
                comptime if (shouldSerialize<T>(field)) {
                    if (!first) append(",")
                    first = false
                    val key = toJsonKey<T>(field)
                    append(f"\"{key}\":")
                    append(toJson(value.[field]))
                }
            }
            append("}")
        }
    }
    // ...
}
```

### 15.7 `@Intrinsic` and Sysroot Declarations

The `@Intrinsic` annotation marks declarations whose implementation is provided by the compiler or runtime rather than in Scoop source. Intrinsic declarations have signatures but no body:

```kotlin
@Intrinsic
// Word-sized signed integer (bit width equals the target pointer size).
struct Int {
    fun toString(): String
    fun toFloat(): Float
    fun abs(): Int
}

@Intrinsic
fun println(value: Int)

@Intrinsic
fun println(value: String)
```

Intrinsic declarations live in the **sysroot** — a directory of `.scoop` files that define the compiler's built-in API surface (see SCOOP_RUNTIME.md §8 for the sysroot directory structure). The compiler parses these files before user code and resolves calls to intrinsics using its own internal implementations.

This design serves two purposes:
1. **Tooling**: LSP, debuggers, and documentation generators can read the sysroot `.scoop` files to discover built-in APIs, show hover documentation, and provide go-to-definition.
2. **Specification**: The sysroot files are the source of truth for which built-in functions, methods, and types exist.

### 15.8 `@NoGC` (Allocation-Free Code)

The `@NoGC` annotation marks a function as safe to run in contexts where **GC-managed heap allocation must not occur**, such as parts of the runtime itself (including the GC implementation written in Scoop).

#### 15.8.1 Call restrictions

A `@NoGC` function may only call:

- Other `@NoGC` functions
- `@Extern` functions

This restriction is transitive: calling into non-`@NoGC` Scoop code is forbidden.

Note: calling an `@Extern` function still requires an unsafe context (see §15.9). Use an `@Unsafe do { ... }` block to localize the unsafe call when needed.

#### 15.8.2 Allocation restrictions

The compiler must reject a `@NoGC` function if it contains any operation that can perform a GC-managed heap allocation, including (non-exhaustive):

- Allocating a reference type instance (`class` / `object` construction)
- Boxing a value type to a reference type (e.g., storing a value type into an interface or `Any`)
- Creating an escaping continuation / state machine that is GC-allocated (e.g., escape-continuation effect handlers)
- Creating a heap-allocated closure object (when a lambda capture cannot be proven to be allocation-free)
- Creating GC-managed arrays or other heap objects (library-defined)

Calls to `@Extern` functions are permitted (and must occur in an unsafe context; see §15.9), but the call boundary must still obey the above rules (e.g., argument passing must not require boxing or allocating temporary heap objects).

The `@NoGC` annotation is a **compiler-checked guarantee**. If the compiler cannot prove that the function is allocation-free, it must report an error.

`@NoGC` only constrains allocations performed by the annotated function and its (statically permitted) callees. It does not, by itself, guarantee that the GC will not run due to other threads or explicit runtime triggers (implementation-defined).

#### 15.8.3 `@Extern` is implicitly `@NoGC`

By default, a function annotated `@Extern` is treated as if it were also annotated `@NoGC` for the purposes of type checking and call restrictions.

Rationale:

- FFI boundaries are opaque to the compiler.
- Treating `@Extern` as `@NoGC` by default enables low-level runtime code (including the GC) to call OS / libc functions without accidentally depending on GC-managed allocation.

If an external function needs to interact with GC-managed objects, it must do so via the standard GC interop mechanisms:

- For **short-lived** raw pointers into GC-managed memory: use pinning (`GC.pin` / `GC.unpin`, §15.10).
- For **long-lived** references that must survive safepoints: use stable GC handles (`GC.handleNew` / `GC.handleGet` / `GC.handleDrop`, §15.10.1).

External code must not retain raw object addresses across safepoints (moving/compacting GCs may relocate objects). The toolchain/runtime defines the concrete native ABI surface for these operations (see `SCOOP_RUNTIME.md`).

#### 15.8.4 Example

```kotlin
@NoGC
@Unsafe
fun bumpAlloc(alloc: Ptr<Byte>, size: Int): Ptr<Byte> {
    // Implementation is expected to use raw pointers and manual memory management.
    // All operations must be allocation-free and only call other @NoGC / @Extern functions.
    return alloc + size
}
```

### 15.9 `@Unsafe` (Unsafe Memory Operations)

The `@Unsafe` annotation is an explicit opt-in to use **unsafe low-level operations** that are not memory-safe, such as pointer arithmetic and unchecked loads/stores. This is intended for implementing the runtime and other low-level components in Scoop.

The exact set of unsafe primitives is defined by the sysroot API surface (§15.7) and may include raw pointer types (e.g., `Ptr<T>`) and memory intrinsics.

#### 15.9.1 Unsafe context

Scoop defines an **unsafe context** as either:

- The body of a function annotated `@Unsafe`, or
- The body of a `do` block annotated `@Unsafe` (see §15.9.2)

Unsafe primitives are only permitted within an unsafe context. Using an unsafe primitive outside an unsafe context is a compile error.

Calling a function annotated `@Unsafe` is only permitted within an unsafe context. Violations are compile errors.

Calling an `@Extern` function is only permitted within an unsafe context. (External functions are assumed to be able to violate memory safety unless wrapped by a safe Scoop API.)

#### 15.9.2 `@Unsafe` blocks

`@Unsafe` may be applied to a `do` block to create a localized unsafe context without marking the entire enclosing function as unsafe:

```kotlin
fun doIO(): Unit / Async {
    val buf = ByteBuffer.allocate(4096)
    @Unsafe do {
        // Call to @Extern and raw pointer operations are allowed here.
        submitToKernel(buf)
    }
    await completion()
}
```

An unsafe block has the form `@Unsafe do { ... }`. Because bare `{ ... }` is reserved for closure literals (§7.6), the `do` introducer is required for a localized unsafe block. The annotation scopes to the immediately following `do { ... }` block. `@Unsafe { ... }` is not a localized block form and must be rejected; write `@Unsafe do { ... }` instead.

#### 15.9.3 Relationship to `@NoGC`

`@Unsafe` does not imply `@NoGC`. Functions (or `do` blocks) that require both properties may be annotated with both.

#### 15.9.4 Raw pointers (`Ptr<T>`) and address integers (`UIntPtr`)

The sysroot exposes a raw pointer type:

```kotlin
@Intrinsic
struct Ptr<T> {
    // Unsafe pointer casts. The caller must uphold all invariants.
    @NoGC @Unsafe fun <U> cast(): Ptr<U>

    // Unsafe memory operations. The caller must ensure the pointer is valid.
    @NoGC @Unsafe fun load(): T
    @NoGC @Unsafe fun store(value: T)

    // Pointer arithmetic in units of T (not bytes).
    @NoGC @Unsafe operator fun plus(offset: Int): Ptr<T>
    @NoGC @Unsafe operator fun minus(offset: Int): Ptr<T>
}

// Address-of intrinsic: takes the address of a variable slot.
// The `var` parameter position indicates the argument must be an addressable variable (an lvalue).
@Intrinsic @NoGC @Unsafe
fun <T> addressOf(var: T): Ptr<T>

// Stack allocation intrinsic: allocates space for a GC-free T on the stack and returns its address.
@Intrinsic @NoGC @Unsafe
fun <T> stackAlloc(): Ptr<T>

// Function pointers.
@Intrinsic
struct FunPtr<F>
```

`Ptr<T>` is a low-level primitive intended for runtime/FFI code. Unsafe pointer operations (such as dereferencing, raw memory loads/stores, pointer arithmetic, and pointer/integer casts) must only be permitted within an unsafe context (see §15.9.1).

**GC-free pointee restriction:**

To avoid hiding GC-managed references inside untracked memory, `Ptr<T>` is only well-formed when `T` is a **GC-free value type**:

- `T` must be a value type (`struct` / `enum` / tuple / built-in scalar).
- `T` must not contain (directly or transitively) any reference-typed fields.
- In particular, any field whose runtime representation contains a GC pointer (including `Option<RefType>` niches) makes the containing type non-GC-free.

This restriction is enforced statically.

**Pointer-sized integers:**

`UIntPtr` is defined by the sysroot as an alias to the target word-sized unsigned integer:

```kotlin
typealias UIntPtr = UInt
```

`UIntPtr` exists for clarity when documenting address arithmetic and FFI APIs. The type itself is not unsafe.

**Pointer ↔ integer casts:**

Converting between pointers and integers is unsafe and must require an unsafe context. The exact API surface is sysroot-defined (typically via intrinsics).

For example:

```kotlin
@Intrinsic @NoGC @Unsafe
fun <T> ptrToUIntPtr(p: Ptr<T>): UIntPtr

@Intrinsic @NoGC @Unsafe
fun <T> uintPtrToPtr(addr: UIntPtr): Ptr<T>
```

Note: these conversions are **not** expressed via `as` / `as?` (those are runtime type casts expressed via `Raise`). Pointer/integer conversions must be provided via sysroot intrinsics and must require an unsafe context.

Note: using a raw pointer into GC-managed memory requires pinning (see §15.10) if the pointed-to object may move.

Function pointers:

- `FunPtr<F>` represents an opaque native function pointer for the function type `F` (e.g. `F = (Int, Int) -> Int`).
- If `F` is a receiver function type `T.(A1, ..., An) -> R`, the receiver is passed as the first explicit argument when invoking the pointer (for example, `fp(receiver, a1, ..., an)` or `fp.invoke(receiver, a1, ..., an)`).
- Calling a function pointer is unsafe and must require an unsafe context.
- `@CallingConvention(...)` may be used to specify the calling convention of a `FunPtr` alias.

Example:

```kotlin
@CallingConvention("stdcall")
typealias MyFuncPtr = FunPtr<(Int, Int) -> Int>

@Unsafe do {
    val funcPtr: MyFuncPtr = /* get function pointer from somewhere */
    val result = funcPtr.invoke(1, 2)
}
```

Internal atomic value types (FFI-oriented):

- The sysroot/runtime may define internal atomic value types such as `__AtomicInt`, `__AtomicLong`, `__AtomicBoolean`, ...
- These types are GC-free and have the same memory layout as their underlying scalar types.
- Atomic operations (load/store/CAS/etc.) are compiler intrinsics that generate LLVM IR directly.

#### 15.9.5 `@Safe` (Safe Regions Inside Unsafe Context)

`@Safe` marks a function body, `do` block, or closure body as **safe**, even if it appears syntactically inside an enclosing unsafe context.

This is useful when a mostly-unsafe function wants to carve out a local safe region, or when an API wants a callback body to be checked as safe. For a local block region, use `@Safe do { ... }`; a bare `{ ... }` without `do` remains a closure literal (§7.6).

```kotlin
@Unsafe
fun unsafeFunction() {
    // unsafe operations allowed here

    @Safe do {
        // Unsafe operations are NOT allowed here.
        // Calling @Extern or @Unsafe functions is a compile error unless re-wrapped in @Unsafe do { ... }.
    }
}
```

Note: `someFunTakesTrailingLambda(@Safe { ... })` continues to annotate a closure literal. Use `@Safe do { ... }`, not `@Safe { ... }`, when the intent is a local safe block.

Rules:

- The body of a `@Safe` function, `@Safe do { ... }` block, or `@Safe { ... }` closure is type-checked as if it were **not** in an unsafe context.
- Nested `@Unsafe do { ... }` blocks inside a `@Safe` region re-enable unsafe operations locally.

### 15.10 GC Pinning API (`pin` / `unpin`)

To support low-level runtime code and proactive-style I/O APIs (e.g., `io_uring`), Scoop exposes a GC pinning API that prevents the GC from moving a heap object while a foreign subsystem retains a raw pointer into it.

Motivation:

- For a synchronous OS call, a pointer to GC-managed memory is typically only used for the duration of the call.
- For proactive/asynchronous I/O, the OS may retain the pointer beyond the call and complete the I/O later. The buffer must remain at a stable address until completion.

The pinning API is provided by the sysroot/runtime (see §15.7). In Scoop 0.1 the standard shape is:

```kotlin
@Intrinsic
object GC {
    @NoGC @Unsafe
    fun pin(obj: Any): Pinned

    @NoGC @Unsafe
    fun unpin(pinned: Pinned)
}

// Value type handle.
// `Pinned.value` exists so the handle keeps the object alive while pinned.
struct Pinned(val value: Any)
```

Semantics:

- `GC.pin(obj)` marks `obj` as **pinned**: it must not be moved by the GC until it is unpinned.
- A pinned object is treated as a GC root (kept alive) while it is pinned. Each successful `pin` must be paired with exactly one `unpin`. Dropping a `Pinned` handle without calling `unpin` is a resource leak (the object remains pinned indefinitely).
- `GC.unpin(pinned)` removes the pin associated with the `Pinned` handle. Unpinning twice is a runtime error.
- Pinning/unpinning is an unsafe operation because early-unpin (while a foreign party still holds the pointer) can cause memory corruption.
- Pinning is per-object. The object whose address is shared with a foreign subsystem must be the object that is pinned; pinning a wrapper object does not automatically pin other objects it references.

Pinning does not imply that the GC will not run; it only constrains object movement. Excessive or long-lived pinning may reduce GC compaction effectiveness (implementation-defined).

#### 15.10.1 Stable GC Handles (`handleNew` / `handleGet` / `handleDrop`)

Pinning is the mechanism for **short-lived** raw pointers into GC-managed memory (typically for the duration of a single synchronous OS call). It is **not** the general mechanism for letting native code keep references across safepoints.

For moving/compacting GCs, native code must not retain raw object addresses across safepoints. Instead, the runtime provides **stable GC handles**: an opaque token that remains stable even if the object moves.

Standard API shape (sysroot/runtime):

```kotlin
@Intrinsic
object GC {
    @Unsafe
    fun handleNew(obj: Any): GcHandle

    @NoGC @Unsafe
    fun handleGet(handle: GcHandle): Any

    @NoGC @Unsafe
    fun handleDrop(handle: GcHandle): Unit
}

// Opaque token. The concrete representation is toolchain-defined but must be stably passable to native code.
struct GcHandle(val raw: UIntPtr)
```

Semantics:

- `GC.handleNew(obj)` creates a new handle that keeps `obj` alive and returns the handle token.
- `GC.handleGet(handle)` returns the current object reference for this handle. The returned reference may point to a different address at different times (object movement is allowed).
- `GC.handleDrop(handle)` releases the handle. Dropping/forgetting to call `handleDrop` is a resource leak.
- A GC handle does **not** imply pinning: it does not guarantee a stable object address. If raw pointers into the object are required, the object must additionally be pinned for that duration (see §15.10).
- Stable GC handles are the intended long-lived token for async I/O completion callbacks, reactor registrations, wakeup identities, and similar native bookkeeping. They keep the object alive but do not pin it; pinning remains a separate short-lived operation only for borrowing raw addresses.

### 15.11 Type Descriptor Release Callback (FFI-managed Resources)

Scoop does not support general-purpose finalizers. However, for FFI and managed-unmanaged interop, the runtime may associate an optional **release callback** with a GC-managed object type.

Semantics (high-level):

- Each GC-managed type has an implementation-defined type descriptor used by the GC for scanning and layout.
- The type descriptor may optionally include a `release_callback: (ptr: Ptr<Byte>) -> Unit` function pointer.
- When the GC determines that an object is unreachable and is about to be reclaimed, it calls the release callback (if present) with a pointer to the object's storage.
- The callback is intended for releasing **unmanaged** resources associated with the object (e.g. memory in a non-GC arena, OS handles), and must not assume any ordering relative to other objects.

Important restrictions:

- This is **not** a user-facing language feature in Scoop 0.1: the callback is **auto-synthesized by the compiler** for specific runtime/library types.
- The callback must not resurrect the object or depend on other GC-managed objects being alive.
- The exact calling context and allowed operations are implementation-defined; implementations should treat the callback as `@NoGC` and `@Unsafe`-like.

## 16. Other Features

### 16.1 Array Literals (`[a, b, c]`)

Scoop supports array literals:

```kotlin
val xs = [1, 2, 3]
val ys = ["hello", "world"]
val ps = [Point { x: 12, y: 21 }, Point { x: 3, y: 4 }]
```

#### 16.1.1 Result type (`Array<T>` vs `MutableArray<T>`)

The type of an array literal is selected by the **expected type**:

- If the expected type is `Array<T>`, the literal has type `Array<T>`.
- If the expected type is `MutableArray<T>`, the literal has type `MutableArray<T>`.
- If there is no expected type, the literal defaults to `Array<T>`.

The element type `T` is inferred as the **least upper bound** (LUB) of all element expression types (see §14.8).

The empty literal `[]` requires an expected type to infer `T`; otherwise it is a compile error.

#### 16.1.2 No implicit boxing inside literals

Array literals do **not** perform implicit boxing/conversion of individual elements.

This matters when the inferred/expected element type is a reference type such as `Any`:

```kotlin
val a1 = [1 as Any, 2 as Any]   // ✅ explicit boxing per element
val a2 = [1, 2 as Any]          // ❌ compile error (would require implicit boxing of 1)
```

Rationale: allowing implicit per-element boxing in literals makes the element type inference non-local and can hide allocations. Scoop keeps boxing explicit at the element level for array literals.

#### 16.1.3 Evaluation order

Element expressions are evaluated left-to-right. The array is then allocated and populated in source order.

### 16.2 Iteration Protocol and `for` Loops

#### 16.2.1 Iterator surface

The standard iterator protocol uses `Option<T>` to avoid a separate `hasNext()` cache:

```kotlin
interface Iterable<T> {
    fun iterator(): Iterator<T>
}

interface Iterator<T> {
    fun next(): Option<T>
}
```

#### 16.2.2 `for` desugaring

`for (x in xs) { body }` desugars to the following (schematically):

```kotlin
val it = xs.iterator()
while (true) {
    when (it.next()) {
        Some(x) -> { body }
        None -> break
    }
}
```

The element binder `x` is scoped only within the loop body.

Any effects required by `iterator()`, `next()`, and the loop body are handled by normal effect typing rules: the enclosing function (or handler arm) must permit the union of those effects.

### 16.3 Kotlin-derived features

The following features follow Kotlin semantics unless otherwise noted in this specification:

- Operator overloading
- Object declarations and companion objects
- Type aliases
- Destructuring declarations
- Ranges and progressions
- Collection operations

---

*This specification is a living document. Sections will be expanded as the language design evolves.*

---

# Appendices

The appendices below are **additive** and clarify or expand specific areas of the language design.

## Appendix A: Scoped Effect Dispatch (Handler Stack)

This appendix incorporates the **new effect dispatch specification** described in `HANDLER-STACK.md`.

### A.1 Intent

The effect system is lexically scoped by `handle { ... } with { ... }` blocks. When multiple handlers are active due to nesting, `perform` must be delivered to the **nearest** enclosing handler that can handle that specific operation.

This appendix makes that scoping rule explicit and also specifies the semantics for effects performed **inside handler arms**.

### A.2 Definitions

- **Effect operation**: a qualified operation name such as `Raise.raise` or `Async.await`.
- **Handler instance**: a dynamic instance of a `handle` block during execution. (A single syntactic `handle` expression may be entered multiple times via loops or continuation resumes.)
- **Active handler**: a handler instance that is currently in the dynamic scope of execution.
- **Handled set**: the set of effect operations that have an arm in the handler’s `with { ... }` block.

### A.3 Dispatch Rule (Nearest Matching Handler)

When evaluating `perform E.op(args...)`:

1. Evaluate `args...` (in left-to-right source order) to values.
2. Identify the **target handler** as the nearest enclosing **active handler instance** whose handled set contains `E.op`.
3. The effect is dispatched to **only that** target handler.
4. If there is no such active handler instance, the effect remains unhandled and propagates outward. If an unhandled effect reaches the program boundary, it is a runtime error (panic). Well-typed programs should prevent this by ensuring entry points require `Pure!` (see §5.10).

This rule is independent of the handler arm form (`->`, `-> resume`, or `, k ->`).

### A.4 Handler Arm Execution Is Outside the Handler’s Dispatch Scope

A handler arm is conceptually executed **outside** the handler’s own dynamic scope for the purposes of effect dispatch.

Normative rule:

- During evaluation of the body of a handler arm (regardless of arm form), the handler instance that selected that arm is treated as **inactive** for the purposes of selecting a target handler for `perform`.

Consequence:

- If a handler arm performs an operation that the same handler would normally handle, the performed effect is dispatched to the next outer matching handler (or remains unhandled), rather than re-entering the same handler.

This rule prevents accidental self-capture and makes nested handlers behave predictably.

#### Example: re-performing the same effect inside an arm

```kotlin
handle {
    handle {
        perform Raise.raise("inner")
    } with {
        Raise.raise(e), k -> {
            perform Raise.raise("from arm")  // targets the OUTER handler
        }
    }
} with {
    Raise.raise(e) -> println(e)
}
```

### A.5 Interaction With Continuations

- **Immediate-resume arms (`-> resume`)**: The handler is inactive during the arm body; after the arm calls `resume(value)` and control returns to the resumed computation, the handler becomes active again for that resumed computation.
- **Escape-continuation arms (`, k ->`)**: When `k.resume(...)` is invoked, the resumed computation runs under the dynamic scope of the handler that created `k`, except that the handler is inactive while executing any of its own arms (per §A.4).

### A.6 Non-goals

- This appendix does **not** introduce new user-facing syntax.
- This appendix does **not** change the surface forms of effect declaration, `perform`, or `handle`.
- This appendix does **not** specify scheduling, synchronization, or a memory model. Concurrency behavior remains out of scope beyond the rule that dispatch follows the current computation’s active handler stack (which is restored when a continuation is resumed; §5.5).

## Appendix B: Kotlin Semantics Adopted by Scoop (Expanded)

This specification states that several features “follow Kotlin semantics”. This appendix expands those references so that this document can be read as a **standalone** language specification.

### B.1 Normative Reference

For each item below, unless otherwise noted by Scoop-specific rules in this specification, Scoop adopts the corresponding behavior from the Kotlin language specification and reference documentation:

- Kotlin Language Specification: `https://kotlinlang.org/spec/`
- Kotlin Language Reference: `https://kotlinlang.org/docs/`

Where Kotlin’s semantics depend on JVM-specific runtime behavior, Scoop follows the **language-level** rule but uses its own runtime model (GC + value types) as specified elsewhere in this document.

### B.2 Classes (Expanded)

§2.2.1 says “Classes follow Kotlin semantics”. In addition to the Scoop-specific rules already listed, the following Kotlin-like rules apply.

#### B.2.1 Final / open / abstract / sealed

- Classes are `final` by default.
- A class must be marked `open` to allow inheritance.
- An `abstract` class cannot be instantiated; it may declare abstract members.
- A `sealed` class restricts direct subclassing to the same compilation unit (as in Kotlin).

#### B.2.2 Constructors and initialization

- A class may declare a **primary constructor** in the class header.
- Constructor parameters declared with `val`/`var` become properties.
- Initialization is performed by:
  - evaluating property initializers,
  - executing `init { ... }` blocks,
  - and executing the constructor body (for secondary constructors),
  in Kotlin-like order.

(Exact ordering details are inherited from Kotlin; Scoop-specific restrictions may apply where value-type immutability or effect declarations would otherwise be violated.)

#### B.2.3 Inheritance and overriding

- Single inheritance of classes; multiple interface implementation.
- Overriding a member requires `override`.
- A member must be declared `open` (or `abstract`) to be overridable.

### B.3 Null-safe Operators (`?.`, `?:`, `!!`) (Expanded)

§2.4 defines `T?` as `Option<T>` and says Kotlin’s null-safe operators are sugar.

In this section, “nullable” means “an `Option<T>` value”.

#### B.3.1 Safe-call (`?.`)

For a nullable receiver `x: T?`:

- `x?.member` is sugar for:
  - if `x` is `Some(v)`, evaluate `v.member` and wrap as `Some(...)`.
  - if `x` is `None`, the result is `None`.

- `x?.call(args...)` is sugar for:
  - evaluate `args...` only if `x` is `Some(v)`.

The result type of a safe-call is always nullable (i.e., `R?`).

#### B.3.2 Elvis (`?:`)

For `x: T?`:

- `x ?: y` evaluates to:
  - `v` if `x` is `Some(v)`
  - `y` if `x` is `None`

`y` is evaluated only if needed.

#### B.3.3 Not-null assertion (`!!`)

For `x: T?`:

- `x!!` evaluates to `v` if `x` is `Some(v)`.
- If `x` is `None`, it performs `Raise.raise(RuntimeError.NullAssertionFailed)` and therefore requires `Raise<RuntimeError>` unless handled by `try`/`catch` (see §5.7 and §14.7).

### B.4 Declaration-site Variance (`in` / `out`) (Expanded)

§3.2 adopts Kotlin-like declaration-site variance, with the Scoop-specific restriction that variance-based subtyping only applies when the type arguments are reference types.

Additional Kotlin-like rules:

- A type parameter is invariant by default.
- `out T` makes the type constructor covariant in `T`.
- `in T` makes it contravariant in `T`.

Variance position rules follow Kotlin:

- An `out` type parameter may only appear in **out-positions** (e.g., return types) of the public API of the declaration.
- An `in` type parameter may only appear in **in-positions** (e.g., parameter types) of the public API of the declaration.

Scoop may allow implementation-internal uses that are erased or proven safe by the type checker, following Kotlin’s general approach.

### B.5 Functions (Expanded)

§7.1 says “Function syntax follows Kotlin”, with Scoop-specific differences noted in §7.2–§7.4.

The following Kotlin-like rules apply.

#### B.5.1 Expression body vs block body

- `fun f(): T = expr` is an expression-body function.
- `fun f(): T { ... }` is a block-body function.
- Local blocks outside declaration bodies use `do { ... }`; bare `{ ... }` remains a closure literal (§7.6).

#### B.5.2 Default arguments

- Parameters may have default values.
- A call may omit arguments that have defaults.

#### B.5.3 Named arguments

- Arguments may be passed by name: `f(x = 1, y = 2)`.
- Named arguments may be mixed with positional arguments, but positional arguments must come first.

#### B.5.4 Trailing lambda(s)

When the final one or more parameters of a call are function types, the corresponding trailing lambdas may be written outside parentheses, Kotlin-style:

```kotlin
users.filter { it.age >= 18 }
combine(1) { it + 1 } { it + 2 }
```

Since bare `{ ... }` is always a closure literal (§7.6), `foo { ... }` and `foo { ... } { ... }` are always calls with trailing lambda(s), not plain local blocks. Use `foo(do { ... })` when the intent is to pass the evaluated result of a local block instead of a closure; a following `do { ... }` starts the next expression/statement unless it appears inside the call parentheses.

#### B.5.5 Varargs

- A parameter may be marked `vararg` to accept zero or more arguments.
- Type inference treats vararg arguments as Kotlin-style collection of elements of the parameter type.

### B.6 Properties (Expanded)

§10 already specifies most Kotlin-like property behavior (backing fields, accessors, extension properties, delegated properties).

This appendix adds the following Kotlin-like clarifications:

- A property declaration introduces a getter; `var` additionally introduces a setter.
- Accessor visibility and modifiers follow Kotlin-style rules.
- Property access (`obj.prop`) is equivalent to invoking the getter or setter, not direct field access, except where the compiler chooses a direct access as an optimization.

### B.7 Package Declarations and Imports (Expanded)

§13.6 shows Kotlin-like `package` and `import` syntax.

Additional Kotlin-like rules:

- Each source file may declare at most one `package` directive at top level.
- Imports may use wildcard syntax `import foo.bar.*`.
- Imports may use aliases: `import foo.bar.Baz as Qux`.

### B.8 Operator Overloading (Expanded)

Scoop supports Kotlin-style operator overloading via specially-named functions.

- For each operator token, there is a corresponding function name (e.g., `+` → `plus`, `-` → `minus`, `*` → `times`, `/` → `div`).
- Bitwise operators map to conventional names:
  - `a & b` → `and`
  - `a | b` → `or`
  - `a ^ b` → `xor`
  - `~a` → `inv`
  - `a << b` → `shl`
  - `a >> b` → `shr` (arithmetic for signed integers, logical for unsigned integers)
- Indexing uses `get` / `set` (`a[i]`, `a[i] = v`).
- Comparison uses `compareTo` for ordering operators.

Whether an explicit `operator` modifier is required is implementation-defined unless specified elsewhere.

### B.9 Object Declarations and Companion Objects (Expanded)

Scoop adopts Kotlin-style singleton `object` declarations:

- `object Name { ... }` declares a single instance named `Name`.
- A class may declare a `companion object` for namespaced members accessible as `ClassName.member`.

### B.10 Type Aliases (Expanded)

Scoop adopts Kotlin-style type aliases:

```kotlin
typealias UserId = Int
typealias Handler<T> = (T) -> Unit
```

A type alias introduces a name that is equivalent to its expanded type during type checking.

### B.11 Destructuring Declarations (Expanded)

Scoop adopts Kotlin-style destructuring declarations for tuples and for types that support destructuring.

- Tuple destructuring:

```kotlin
val (a, b) = (1, 2)
```

- For non-tuple types, destructuring is supported where Scoop already specifies destructuring patterns (e.g., enum variants and struct patterns in `when`).

### B.12 Ranges and Progressions (Expanded)

Scoop adopts Kotlin-style range/progression syntax at the language level, backed by library definitions:

- `a..b` constructs an inclusive range.
- `a until b` constructs a half-open range.
- `downTo` and `step` construct descending and stepped progressions.

The exact set of built-in range types and their performance characteristics are specified by the standard library and sysroot declarations.

### B.13 Collection Operations (Expanded)

Scoop adopts Kotlin-style collection operations primarily as standard-library extension functions (e.g., `map`, `filter`, `flatMap`, `fold`, `associate`, `groupBy`).

- The semantics of these operations are library-defined.
- Their availability and signatures are part of the sysroot API surface (see §15.7).
