# Issues

## 范围

- 本清单只记录当前阶段应优先处理的“语言特性 / 编译器实现”缺口。
- 明确排除：`stdlib/`、`sysroot/` 中属于下一阶段的库能力补齐，以及 `fs/io/net/path/collections` 等 API 表面完整性问题。
- 复核后已排除的陈旧注释示例：成员方法直连 / vtable / itable 端到端链路、f-string 基本链路，这些已经有实现或回归用例支撑，不再计入问题。

## 1. effect / continuation lowering 仍然只覆盖很窄的 v0 子集

- 现状：`handle` 目前直接拒绝在同一个表达式里混用 `->`、`-> resume`、`, k ->` 三种 arm 形态；自定义 non-resuming effect codegen 只支持 `op(arg)` 且 `arg` 必须是 word-sized `Int`，并要求匹配的 `handle` 边界在同一函数内；immediate-resume 路径只支持单个 `val x = perform` 形式、禁止 `finally`，并且 `resume(value)` 仅支持一个位置实参。
- 影响：effect 系统虽然已能覆盖一部分样例，但复杂 handler 组合、真正通用的 continuation 语义、以及更完整的控制流场景都还没有落地。
- 证据：`crates/scoopc/src/typecheck/expr/infer.rs:1254-1261`；`crates/scoopc/src/llvm/codegen/effect.rs:782-787`；`crates/scoopc/src/llvm/codegen/effect.rs:882-885`；`crates/scoopc/src/llvm/codegen/effect.rs:1582-1585`；`crates/scoopc/src/llvm/codegen/effect.rs:1998-2005`；`crates/scoopc/src/llvm/codegen/effect.rs:2008-2043`；`crates/scoopc/src/llvm/codegen/effect.rs:2221-2239`；`crates/scoopc/src/llvm/codegen/effect.rs:8487-8499`。

## 2. `spawn` / `join` 的可执行路径仍然硬编码在 `Int`

- 现状：`spawn { ... }` 目前要求 body 可赋给 `Int`；typecheck 虽然把结果包装成 `Task<Int>`，但执行链路仍按 “word-sized handle + `Int` 结果” 建模；HIR lowering 中的 `join` 也仍然写死为 `Int` 句柄与 `Int` 返回值。
- 影响：结构化并发语法已经可用，但还不是完整的 `Task<T>` 语义，泛型结果类型、统一的 `await/join` 模型与后续 executor 语义都还没有贯通。
- 证据：`crates/scoopc/src/typecheck/expr/infer.rs:476-481`；`crates/scoopc/src/typecheck/expr/infer.rs:508-511`；`crates/scoopc/src/typecheck/expr/infer.rs:565-569`；`crates/scoopc/src/hir/lower/expr.rs:626-628`；`crates/scoopc/src/hir/lower/expr.rs:651-652`。

## 3. lambda 推断与 receiver lambda 仍不完整

- 现状：没有 expected function type 时，未标注类型的 lambda 参数会直接报错；当前只支持 0/1/2 参数 lambda 的 expected-type 向下传播；receiver function type 虽可进入类型系统，但 lambda body 里还不会自动注入 `this`。
- 影响：语言已经支持 lambda 基本语法，但常见的“先写 lambda 再让上下文推断”“多参数 lambda”“receiver lambda 内直接用 `this`”等体验仍不完整。
- 证据：`crates/scoopc/src/typecheck/expr/infer.rs:303-319`；`crates/scoopc/src/typecheck/expr/infer.rs:2025-2032`；`crates/scoopc/src/typecheck/expr/infer.rs:2034-2058`。

## 4. 调用语义仍有多处早期门禁

- 现状：函数值调用目前不支持命名实参；函数指针调用同样不支持命名实参，并明确拒绝 receiver function type 作为签名；`super(...)` / `this(...)` 构造器委托调用目前也只允许位置参数。
- 影响：Kotlin-like 调用规则并没有在所有 callee 形态上统一落地，调用语义仍存在明显的“特判分层”。
- 证据：`crates/scoopc/src/typecheck/expr/call.rs:789-800`；`crates/scoopc/src/typecheck/expr/call.rs:943-950`；`crates/scoopc/src/typecheck/expr/call.rs:974-978`；`crates/scoopc/src/typecheck/expr/entry.rs:901-911`；`crates/scoopc/src/typecheck/lower.rs:146-150`。

## 5. `where` 子句还不支持带类型实参的 nominal bound

- 现状：`where` 约束一旦写成带类型实参的 nominal bound，就会被直接拒绝，例如 `where T: Box<Int>` 这一类形式。
- 影响：泛型约束系统目前还只能覆盖较浅的上界表达能力，约束“某个已实例化泛型类型”的场景尚不可用。
- 证据：`crates/scoopc/src/typecheck/where_clause.rs:35-40`；`crates/scoopc/src/typecheck/where_clause.rs:209-215`。

## 6. 顶层 pattern binding 仍不支持

- 现状：顶层 `val` 的声明头检查会直接拒绝 `val (a, b) = ...` 这类 pattern binding。
- 影响：局部解构绑定已经在语言里逐步推进，但顶层声明还不能使用同一套语法，特性覆盖不一致。
- 证据：`crates/scoopc/src/typecheck/headers.rs:26-32`；`crates/scoopc/src/typecheck/headers.rs:177-190`。

## 7. 值类型建模仍不完整：struct 字段与 `with` 更新都有硬限制

- 现状：`struct` 字段目前不允许 `var`，也不允许默认值；`with` 更新的 base 虽然语义上想覆盖值类型，但实现上当前只支持 `struct`，并且嵌套字段路径更新仍被显式拒绝。
- 影响：值对象的声明与更新能力仍偏弱，无法覆盖更接近 Kotlin / record-style 的常见写法。
- 证据：`crates/scoopc/src/typecheck/structs.rs:35-50`；`crates/scoopc/src/typecheck/structs.rs:106-149`；`crates/scoopc/src/typecheck/expr/infer.rs:2507-2527`；`crates/scoopc/src/typecheck/expr/error.rs:925-940`。

## 8. `when` 的 or-pattern 仍不能引入 binder

- 现状：or-pattern 已能用于简单判别，但一旦在 `A(x) | B(x)` 这类模式里引入 binder，就会被直接拒绝。
- 影响：模式匹配的表达能力仍被限制在“无绑定的 or-pattern”，不能把多个分支合并成共享绑定的写法。
- 证据：`crates/scoopc/src/typecheck/when_pat.rs:89-98`。

## 9. annotation class 仍只支持 data-only 子集

- 现状：annotation class 当前不支持继承 / 实现接口，也不支持类型体；实现上只接受 “主构造参数承载数据” 的最小子集。
- 影响：注解系统已经有基本声明与使用能力，但 richer annotation model 还没有落地。
- 证据：`crates/scoopc/src/typecheck/annotations.rs:77-90`。

## 10. FFI / calling convention 仍停留在 C ABI

- 现状：extern side table 明确只支持 C ABI；`@CallingConvention` 只接受默认 C ABI 名称（`"c"` / `"cdecl"`），其它 calling convention 会被拒绝。
- 影响：系统编程通道虽然已经打开，但 ABI 维度的表达能力还很窄，无法覆盖更复杂的宿主互操作场景。
- 证据：`crates/scoopc/src/hir/mod.rs:641-646`；`crates/scoopc/src/typecheck/annotations.rs:145-156`。

## 11. `const fun` 仍只允许纯函数签名

- 现状：声明头检查会直接拒绝 `const fun` 上的非 `Pure` effect row，以及任何 `eff` 参数。
- 影响：编译期函数当前只能停留在最保守的纯函数模型，effect-polymorphic 或更复杂的 comptime 语义尚未展开。
- 证据：`crates/scoopc/src/typecheck/headers.rs:34-49`；`crates/scoopc/src/typecheck/headers.rs:93-111`。

## 12. MIR lowering 仍大量依赖 `Todo` 占位

- 现状：MIR 路径里，struct literal、tuple literal、interpolated string、member access、call、cast、type check 等表达式仍会直接降成 `Todo`；`perform` / `handle` 也只具备非常粗糙的占位语义。
- 影响：当前 MIR 更像“结构回归视图”而不是一条完整可执行的中间表示；如果下一步希望把 MIR 作为优化 / 验证 / 解释的稳定基础，这一块仍有明显缺口。
- 证据：`crates/scoopc/src/mir/mod.rs:269-270`；`crates/scoopc/src/mir/mod.rs:352-387`；`crates/scoopc/src/mir/lower.rs:324-343`；`crates/scoopc/src/mir/lower.rs:527-579`；`crates/scoopc/src/mir/lower.rs:638-716`。
