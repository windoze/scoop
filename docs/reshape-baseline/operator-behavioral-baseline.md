# Scalar Operator Behavioral Baseline

本文记录 P8 开始前标量 operator 的现有 lowering 行为。P8-T02 到 P8-T05 的 method intrinsic lowering 必须保持这些边界行为逐位一致；任何行为修正都必须拆成独立变更，不在 P8 内顺手修改。

## Source Of Truth

- 主 baseline 来自 `crates/scoopc/src/llvm/codegen/mir_body/op.rs` 的 `codegen_mir_unary` / `codegen_mir_binary`。
- HIR 直接 codegen 路径 `crates/scoopc/src/llvm/codegen/main/{expr_op.rs,numeric.rs,coerce.rs}` 当前与 MIR 路径使用同一组 LLVM 指令选择规则。
- `Int` / `UInt` 的位宽是 host pointer width；当前 host target 为 64 bit。`Int8/16/32/64` 与 `UInt8/16/32/64` 使用各自固定位宽。
- `Char` 在 LLVM 侧是 unsigned `i32`；`Bool` 是 `i1`；`Float32` / `Float64` 分别是 LLVM `float` / `double`。
- 除明确写出的 runtime call 外，当前 lowering 不插入 overflow check、divide-by-zero check、NaN check 或 panic/Raise。

## Integer Arithmetic

| Operator class | LLVM instruction | Signedness choice | Boundary behavior | Repro expression |
| --- | --- | --- | --- | --- |
| `+` | `add` | same bits as target integer type | two's-complement modulo `2^bits`; no overflow trap | `Int.MAX_VALUE + 1 == Int.MIN_VALUE` on 64-bit `Int` |
| `-` | `sub` | same bits as target integer type | two's-complement modulo `2^bits`; no overflow trap | `Int.MIN_VALUE - 1 == Int.MAX_VALUE` on 64-bit `Int` |
| `*` | `mul` | same bits as target integer type | two's-complement modulo `2^bits`; no overflow trap | `Int.MIN_VALUE * 2 == 0` on 64-bit `Int` |
| unary `-` | `sub 0, x` via `build_int_neg` | same bits as operand | two's-complement modulo `2^bits`; `MIN_VALUE` is unchanged | `-Int.MIN_VALUE == Int.MIN_VALUE` on 64-bit `Int` |
| unary `~` | `xor x, -1` via `build_not` | same bits as operand | flips every bit | `~0 == -1` for signed `Int`; for `UInt8`, `~0 == 255` |

For signed `n`-bit integers, the observable value is the LLVM bit pattern interpreted as two's-complement signed. For unsigned integers, the observable value is the same bit pattern interpreted as `0..2^n-1`.

Concrete 64-bit `Int` examples:

```scoop
fun main(): Int {
    val min: Int = -9223372036854775808
    val max: Int = 9223372036854775807
    require(-min == min)
    require(max + 1 == min)
    require(min - 1 == max)
    require(min * 2 == 0)
    return 0
}
```

## Integer Division And Remainder

| Operator class | LLVM instruction | Signedness choice | Boundary behavior | Repro expression |
| --- | --- | --- | --- | --- |
| signed `/` | `sdiv` | `Int`, `Int8/16/32/64` | divisor `0` is LLVM UB; `MIN_VALUE / -1` is LLVM UB; no check is emitted | with `val min: Int = -9223372036854775808`, `min / -1` reaches `sdiv` |
| signed `%` | `srem` | `Int`, `Int8/16/32/64` | divisor `0` is LLVM UB; `MIN_VALUE % -1` is treated as the same signed overflow boundary by LLVM; no check is emitted | with `val min: Int = -9223372036854775808`, `min % -1` reaches `srem` |
| unsigned `/` | `udiv` | `UInt`, `UInt8/16/32/64` | divisor `0` is LLVM UB; otherwise mathematical unsigned quotient modulo type width | for `UInt8`, `255 / 2 == 127` |
| unsigned `%` | `urem` | `UInt`, `UInt8/16/32/64` | divisor `0` is LLVM UB; otherwise mathematical unsigned remainder modulo type width | for `UInt8`, `255 % 2 == 1` |

There is no divide-by-zero guard in `op.rs`, HIR lowering, MIR lowering, or typecheck. P8 method intrinsics must keep this behavior unless a separate behavior-change task explicitly adds checks.

The signed overflow reproducer is intentionally not a run-pass assertion because its result is undefined by LLVM:

```scoop
fun main(): Int {
    val min: Int = -9223372036854775808
    val overflow = min / -1
    println(overflow)
    return 0
}
```

## Integer Shifts

| Operator class | LLVM instruction | Count handling | Boundary behavior | Repro expression |
| --- | --- | --- | --- | --- |
| `<<` | `shl` | `rhs` is truncated to lhs width, then `rhs & (bitWidth - 1)` | no LLVM out-of-range shift UB from source count; shifted-out bits are discarded | with `val one: Int = 1`, `one << 64 == 1` on 64-bit `Int` |
| signed `>>` | `ashr` | same mask as `<<` | sign-extending right shift | `-8 >> 1 == -4` |
| unsigned `>>` | `lshr` | same mask as `<<` | zero-filling right shift | for `UInt8`, `255 >> 1 == 127` |

Negative shift amounts are accepted when the RHS typechecks as `Int`; they are handled by the same low-bit mask. On a 64-bit `Int`, `-1 & 63 == 63`, so `1 << -1` lowers to `1 << 63`.

```scoop
fun main(): Int {
    val one: Int = 1
    require((one << 64) == 1)
    require((one << -1) == -9223372036854775808)
    require((-8 >> 1) == -4)
    return 0
}
```

## Integer Bitwise Operators

| Operator class | LLVM instruction | Boundary behavior | Repro expression |
| --- | --- | --- | --- |
| `&` | `and` | bitwise AND over the full integer width | `(10 & 12) == 8` |
| `|` | `or` | bitwise OR over the full integer width | `(10 | 5) == 15` |
| `^` | `xor` | bitwise XOR over the full integer width | `(10 ^ 12) == 6` |
| `~` | `xor x, -1` via `build_not` | flips all bits | `~0 == -1` for signed `Int` |

These operations are purely bitwise. Signedness only affects how the final bit pattern is interpreted by later operations or printing.

## Integer And Char Comparison

| Operand type | Operators | LLVM predicate | Boundary behavior | Repro expression |
| --- | --- | --- | --- | --- |
| signed integer | `< <= > >=` | `slt/sle/sgt/sge` | signed two's-complement ordering | `Int.MIN_VALUE < -1` is true |
| unsigned integer | `< <= > >=` | `ult/ule/ugt/uge` | unsigned ordering | for `UInt8`, `255 > 0` is true |
| `Char` | `< <= > >=` | `ult/ule/ugt/uge` over unsigned `i32` | Unicode codepoint numeric ordering; no locale/collation | `'A' < 'a'` is true |
| integer / `Char` / `Bool` equality | `== !=` | `icmp eq/ne` | raw scalar value equality | `'A' == 'A'` is true; `true != false` is true |

For arithmetic expressions whose operands are integer literals and typed integers, typecheck absorbs the literal into the typed side. MIR codegen then emits the predicate for the selected concrete integer type.

```scoop
fun main(): Int {
    val min: Int = -9223372036854775808
    val umax: UInt8 = 255
    require(min < -1)
    require(umax > 0)
    require('A' < 'a')
    return 0
}
```

## Floating-Point Arithmetic

| Operator class | LLVM instruction | Operand type choice | Boundary behavior | Repro expression |
| --- | --- | --- | --- | --- |
| `+` | `fadd` | `Float32` or `Float64`; unsuffixed literal may be absorbed into typed side | IEEE-style addition without fast-math flags; NaN propagates; `+inf + -inf` is NaN | `(1.0 / 0.0) + (-1.0 / 0.0)` is NaN |
| `-` | `fsub` | same | IEEE-style subtraction without fast-math flags; NaN propagates | `nan - 1.0` is NaN |
| `*` | `fmul` | same | IEEE-style multiplication without fast-math flags; NaN propagates | `nan * 1.0` is NaN |
| `/` | `fdiv` | same | IEEE-style division without fast-math flags; `0.0 / 0.0` is NaN; `1.0 / 0.0` is `+inf` | `1.0 / 0.0 > 1.0` is true |
| `%` | `frem` | same | LLVM floating remainder; NaN propagates through unordered inputs | `(0.0 / 0.0) % 1.0` is NaN |
| unary `-` | `fneg` | same as operand | flips the sign bit; NaN remains NaN, with sign bit flipped | `-(0.0 / 0.0)` is NaN |

```scoop
fun main(): Int {
    val nan: Float64 = 0.0 / 0.0
    val posInf: Float64 = 1.0 / 0.0
    val negInf: Float64 = -1.0 / 0.0
    require(posInf > 1.0)
    require(!((posInf + negInf) == (posInf + negInf)))
    require(!(nan == nan))
    return 0
}
```

`+0.0 == -0.0` is true under ordered equality, but their bit patterns remain distinct. There is no current bit-pattern operator in the operator path itself; this note is a baseline constraint for future helpers that inspect representation.

## Floating-Point Comparison

| Operator | LLVM predicate | NaN behavior | Repro expression |
| --- | --- | --- | --- |
| `<` | `fcmp olt` | false if either operand is NaN | `(0.0 / 0.0) < 1.0` is false |
| `<=` | `fcmp ole` | false if either operand is NaN | `(0.0 / 0.0) <= 1.0` is false |
| `>` | `fcmp ogt` | false if either operand is NaN | `(0.0 / 0.0) > 1.0` is false |
| `>=` | `fcmp oge` | false if either operand is NaN | `(0.0 / 0.0) >= 1.0` is false |
| `==` | `fcmp oeq` | false if either operand is NaN; `+0.0 == -0.0` is true | `(0.0 / 0.0) == (0.0 / 0.0)` is false |
| `!=` | `fcmp une` | true if either operand is NaN, or if ordered operands are unequal | `(0.0 / 0.0) != 1.0` is true |

The current `!=` behavior is intentionally recorded from the implementation: it uses LLVM `UNE`, not ordered-not-equal. Future P8 intrinsics must preserve that behavior.

```scoop
fun main(): Int {
    val nan: Float64 = 0.0 / 0.0
    require(!(nan < 1.0))
    require(!(nan <= 1.0))
    require(!(nan > 1.0))
    require(!(nan >= 1.0))
    require(!(nan == nan))
    require(nan != nan)
    require(0.0 == -0.0)
    return 0
}
```

## Boolean Operators

| Operator class | Current lowering | Boundary behavior | Repro expression |
| --- | --- | --- | --- |
| unary `!` | `build_not` on `i1`, equivalent to `xor x, true` | boolean negation | `!true == false` |
| bitwise/non-short-circuit `&` on bool if it reaches MIR | `and i1` | evaluates both operands before the instruction | `true & false == false` |
| bitwise/non-short-circuit `|` on bool if it reaches MIR | `or i1` | evaluates both operands before the instruction | `true | false == true` |
| `&&` | MIR short-circuit control flow (`CondBr`) for normal source expressions | RHS is not evaluated when LHS is false | `false && rhs()` skips `rhs` |
| `||` | MIR short-circuit control flow (`CondBr`) for normal source expressions | RHS is not evaluated when LHS is true | `true || rhs()` skips `rhs` |
| `^` on bool | no supported source-level baseline today | not typechecked as a builtin bool operator | future `bool_xor` introduces new method surface |

`codegen_mir_binary` contains a defensive `LogAnd` / `LogOr` bool instruction path, but normal MIR lowering handles source `&&` / `||` with if-else control flow before codegen. P8 must not replace short-circuit operators with `Bool.and` / `Bool.or` method calls.

```scoop
var calls: Int = 0

fun rhs(): Bool {
    calls = calls + 1
    return true
}

fun main(): Int {
    require((!true) == false)
    require((false && rhs()) == false)
    require(calls == 0)
    require((true || rhs()) == true)
    require(calls == 0)
    return 0
}
```

## String And Reference Equality

| Operand type | Operators | Current lowering | Boundary behavior | Repro expression |
| --- | --- | --- | --- | --- |
| `String` / `String` | `== !=` | call `scoop_string_equals(lhs, rhs) -> i64`, compare result with zero, invert for `!=` | value equality over runtime string contents; not pointer identity and not `icmp` over pointers | `"a" + "b" == "ab"` is true |
| other ref types | `== !=` | no general pointer-identity operator baseline in the current typecheck/codegen path | unsupported unless a concrete type supplies another method path | do not use as P8 scalar baseline |

```scoop
fun main(): Int {
    val a = "a" + "b"
    val b = "ab"
    require(a == b)
    require(!(a != b))
    return 0
}
```

## P8 Implementation Rules

- Method intrinsic lowering must emit the same LLVM instruction classes and runtime calls listed above for every covered scalar method.
- Integer arithmetic, unary negation, bitwise operations, and shifts must preserve the current wrap/mask behavior; do not add overflow or range checks in P8.
- Integer and unsigned division/remainder must preserve the current LLVM UB boundaries for divisor zero and signed `MIN_VALUE / -1` unless a separate behavior-change task lands first.
- Floating-point lowering must preserve current no-fast-math ordered predicates, including `fcmp une` for `!=`.
- `&&` and `||` must remain short-circuit control-flow constructs; `Bool.and` / `Bool.or` are non-short-circuit method/intrinsic surfaces only.
- `String == String` must continue to use content equality via the current string-equality runtime path, not pointer identity and not integer equality.
- Any change that intentionally deviates from this baseline requires its own PR, review, and fixture update outside P8's operator-methodization tasks.
