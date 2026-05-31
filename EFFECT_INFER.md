# Effect Row Inference Proposal

本文描述一个降低 public 函数 effect row 噪音的方案：显式写出的函数 effect row 仍是完整调用面 contract；只有完全省略的函数 row 才由编译器推导。对于 public concrete 函数，若函数体直接产生 effect，则必须显式写出完整调用面 row。

## Goals

- 减少 public concrete 函数上重复书写的传递性 `/ E`。
- 保留函数类型、higher-order 参数、以及所有动态分派/可 override API 的显式 effect contract。
- 确保所有表达式形式、调用点、跨 cone API、虚分派和语义降糖仍能得到完整 outward effect 信息。
- 不改变 entry point、export、`@NoGC`、`@Extern` 等强边界的 Pure/Pure! 约束。

## Terminology

- `declared_surface_row`：源码声明中显式写出的函数 effect row。若存在，它就是该 callable 的 published 调用面 contract。
- `direct_effect_row`：函数体直接 perform 的 effect row，用于判断省略 row 是否允许。
- `inferred_surface_row`：从函数体和表达式语义推导出的完整 outward effect row，包含 direct effect 和传递 effect。
- `published_surface_row`：调用者、override 检查、跨 cone API 和 ABI 使用的 outward effect contract。若写了 `declared_surface_row`，它等于声明 row；若省略，则等于可推导出的 `inferred_surface_row`。
- `step_effect_row`：后端 effect lowering/state machine 需要覆盖的内部 effect universe，可包含被本地 `handle` 吸收的 perform、resume runtime error、hidden init effects 等。

这些 row 不应继续混用同一个 `declared_row` 名字。

## Source Rules

函数声明尾部的 `/ Row` 保持为完整调用面 contract。简化规则是：只推导省略的函数 row；只要写了 row，就按写出的 row 作为 published surface contract，并检查函数体推导出的 `inferred_surface_row` 不超过它。

因此，显式 row 不能只写函数自己额外加入的 direct effects。如果一个函数在传递性 effects 之外又加入新的 effect，它必须把完整调用面 row 写出来。

```kotlin
effect Raise<E> {
    fun raise(error: E): Nothing
}

public fun direct(): Unit / Raise<Int> {
    Raise.raise(1)
}

public fun transitive(callback: () -> Unit / Raise<Int>): Unit {
    callback()
}
```

`direct` 必须写 `/ Raise<Int>`，因为它直接 perform。`transitive` 可以省略尾部 `/ Raise<Int>`，因为 effect 纯粹来自 callback 调用；编译器推导并发布它的 `published_surface_row = Raise<Int>`。

如果函数同时有 direct effect 和传递 effect，显式 row 必须覆盖完整调用面：

```kotlin
public fun <eff E> mixed(f: () -> Unit / E): Unit / (Log + E) {
    Log.write("start")
    f()
}
```

写成 `/ Log` 是错误，因为 `inferred_surface_row = Log + E` 不满足 declared contract。

### Must Remain Explicit

- 函数类型必须完整写 effect row。`()->Unit` 等价于 `()->Unit / Pure`，和 `()->Unit / Raise<E>` 是不同类型。
- Higher-order 参数、返回类型、字段、property、typealias 中出现的函数类型必须完整写 row。
- 使用 effect row 参数的函数类型必须显式写 `/ E`，例如 `callback: (T) -> U / E`。
- 函数声明一旦显式写 `/ Row`，该 row 必须是完整调用面 contract，不能只列出函数自己新增的 direct effects。
- 没有 body 的 API 必须显式写调用面 effect contract，包括 extern/intrinsic 声明、只发布 metadata 的声明。
- 所有会进入动态分派 ABI 的 method 必须显式写调用面 effect contract，包括 interface method（无论是否有 default body）、abstract method、open method。该 row 是 itable/vtable ABI 的语义组成部分，不能由实现类或子类改变。
- 递归 SCC 若无法稳定推导，必须要求显式 row 或报错。
- override 必须满足 base/interface 已显式声明的 effect contract；实现可以更 Pure，但不能要求更多 outward effects，也不能改变 dispatch ABI。

### May Be Inferred

- Concrete 函数尾部的纯传递性 row 可以省略；省略时发布推导出的 `published_surface_row`。
- 表达式求值过程中产生的所有 outward effect 都可计入 `inferred_surface_row`，无论它来自显式调用、函数值调用、语义降糖、初始化流程，还是其它 core semantic operation。
- private/internal/public concrete 函数在这一点上可以统一，只是 public 的 inferred surface row 需要进入发布 API。

## HIR Facts

HIR/typecheck 阶段应发布 source-level callable facts。

```text
CallableSourceEffectFacts {
    declared_surface_row,
    direct_effect_row,
    inferred_surface_row_template,
    published_surface_row_template,
    row_is_closed,
    inference_status,
}
```

`declared_surface_row` 直接来自源码 `/ Row`，省略则为 `None`。

`direct_effect_row` 从 body 中直接 perform 的 effect operation 得到。public concrete 函数若省略 `/ Row` 且 `direct_effect_row` 非 Pure，应报错并要求显式声明完整调用面 row。

`inferred_surface_row_template` 是仍可含 type/effect 参数的模板级 outward row。例如：

```kotlin
public fun <T, U, eff E> mapOne(x: T, f: (T) -> U / E): U {
    return f(x)
}
```

HIR 应发布：

```text
declared_surface_row = None
direct_effect_row = Pure
inferred_surface_row_template = E
published_surface_row_template = E
```

HIR/typecheck 需要用 `published_surface_row_template` 做普通调用点诊断、override 兼容、跨 cone API 发布前检查。不能等到后端 codegen 才发现缺 handler。对于 interface/abstract/open method，`published_surface_row_template` 必须来自显式 ABI contract。

## MIR And Materialized Facts

MIR/materialization 阶段应在 effect monomorphization 后发布 instance-level facts。

```text
CallableInstanceEffectFacts {
    declared_surface_row,
    actual_surface_row,
    published_surface_row,
    step_effect_row,
}
```

`actual_surface_row` 是 materialized body 推导出的实例级真实 outward row。

`published_surface_row` 是实例化后的调用者可见 row。例如 `mapOne::<Int, String, eff Raise<IOError>>` 的 row 是 `Raise<IOError>`。若源码显式写了 row，`published_surface_row` 使用写出的 row 实例化结果，并要求 `actual_surface_row ⊆ published_surface_row`；若源码省略 row，则 `published_surface_row = actual_surface_row`。

`step_effect_row` 供 effect lowering 和 ABI/state-machine 选择使用。它可以比 `published_surface_row` 更大，因为本地 handled perform、resume runtime error、hidden init effects 等可能不向调用者暴露，但仍需要 lowering 知道。

## Expression Surface Effects

该方案必须是统一的 expression-level effect inference，而不是为某个语法功能开后门。

每个表达式都应有一个 source-level `expr_surface_row`：求值该表达式并把结果交给外层上下文时，可能向外暴露的完整 effect row。

统一规则：

- literal、local read 等 core pure 表达式的 `expr_surface_row = Pure`。
- direct effect operation 的 `expr_surface_row` 包含该 effect。
- 显式 callable call 的 `expr_surface_row` 是 callee `published_surface_row` 与所有 argument expression rows 的 union。
- 函数值调用使用函数值类型中的 surface row，并 union arguments。
- 控制流表达式按实际求值路径 union 条件、分支、body、handler/finally 等子表达式 rows，并按 handler 规则移除被本地处理且不 outward 的 effect。
- 任何语义降糖或 lowering-introduced operation 必须先映射到同一套 core semantic operations，再用相同规则计算 row。

这意味着 computed property、delegated property、operator、loop、constructor/init 等都不需要专门的 effect 后门。它们只需要在 typecheck/HIR 阶段发布自己的 canonical semantic expansion facts，使 effect inference 能看到同一种 core operation graph。

非规范例子：

- property read 若语义上是 getter call，就发布“该 expression 的 core operation 是 getter call”的 fact。
- delegated property read/write 若语义上是 delegate method call，就发布同样的 core call fact；不按 delegate 名称特判。
- operator expression 若语义上是 method/function call，就发布同样的 core call fact；不按 operator 种类特判。
- loop 若语义上调用 iterator/next/hasNext，就发布同样的 core call facts；不按 loop 语法特判。

核心要求：source-level effect inference 消费的是 canonical semantic facts，而不是源码表面 token。任何会在后续 lowering 中变成 outward callable operation 的结构，都必须在前端以统一 fact 形式可见。

## Recursion

递归函数的 `inferred_surface_row` 可以作为 SCC fixed point 推导。

基本规则：

- 非递归函数按 body 单次推导。
- 递归 SCC 从 direct effects 和外部调用 facts 开始迭代到不动点。
- 推导器可以选用任何有意义的 fixed-point 策略；若达到实现定义的迭代深度/复杂度上限后仍未收敛，报错并要求显式 outward row。
- 如果 SCC 内存在 effect-polymorphic recursion、未解析动态分派、跨 cone body 缺失、或 row 变量无法确定，也可以直接要求显式 outward row。

这是诊断策略，不是语法限制。

## Row Template Representation

HIR 的 `inferred_surface_row_template` 和 `published_surface_row_template` 应是符号化 row 模板，而不要求在 HIR 阶段变成完全 concrete 的 effect item 集合。

例如：

```kotlin
public fun <T, U, eff E> mapOne(x: T, f: (T) -> U / E): U {
    return f(x)
}
```

HIR fact 不能把 `E` 提前变成某个具体 row，因为 `E` 要到调用点或 materialization 实例才知道。它应发布类似：

```text
inferred_surface_row_template = RowParam(E)
published_surface_row_template = RowParam(E)
```

如果函数同时有直接 effect 和传递 effect：

```kotlin
public fun <eff E> mixed(f: () -> Unit / E): Unit / Log {
    Log.write("start")
    f()
}
```

这是错误声明。`mixed` 的实际调用面 row 是 `Log + E`，显式写出的 `/ Log` 不足以覆盖它，应报错。正确写法是：

```kotlin
public fun <eff E> mixed(f: () -> Unit / E): Unit / (Log + E) {
    Log.write("start")
    f()
}
```

HIR fact 应是：

```text
declared_surface_row = Log + RowParam(E)
direct_effect_row = Log
inferred_surface_row_template = Log + RowParam(E)
published_surface_row_template = Log + RowParam(E)
```

到 MIR/materialization 时，`RowParam(E)` 被具体 effect argument 替换，得到 instance-level `actual_surface_row` 和 `published_surface_row`。

因此这里的设计要求是：HIR facts 保留 row 参数引用，但必须使用稳定、可序列化、可替换的表示，例如 owner + param index/name；不能依赖 AST span 或本地临时 TypeId。

这不是语言语义上的 open question，而是 facts schema 的实现约束。

## Dynamic Dispatch ABI

动态分派的 effect row 是 dispatch ABI 的一部分。

- interface method、abstract method、open method 必须显式声明调用面 effect row。
- default method 即使有 body，也仍按 interface method 处理：显式 row 是 ABI contract，body 只需被检查为不超过该 contract。
- override/implementation 不能扩展 outward row。若实现体实际需要更多 effect，必须在实现体内 handle 掉，或修改 base/interface contract。
- dynamic dispatch call site 只读取静态 receiver 类型上的 method contract，不需要 union 所有实现类的实际 row。
- vtable/itable slot 的 effect ABI 由 base/interface contract 固定，避免不同实现产生不同 callable ABI。

## Boundaries

entry point、export、`@NoGC`、`@Extern` 仍保持强约束。

- entry point 要求完整 `published_surface_row` 为 `Pure!`。
- export 要求完整 `published_surface_row` 为 `Pure!`。
- `@NoGC` 要求不能声明 effect-row 参数，`direct_effect_row` 和 `published_surface_row` 都必须为 Pure/Pure!。
- `@Extern` 无 body，surface row 等于声明 contract，并继续强制 Pure/Pure!。

这些边界不参与“是否可以省略传递 row”的语法设计，但必须消费推导后的完整 surface facts。

## Compatibility With Current Spec

当前 spec 已有一个方向相近但不一致的表述：open row 被描述为“函数自身可 perform 的 lower bound”，但 required-effects 规则又把 callee 声明 row 直接并入 caller。

需要调整为：

- 函数声明 `/ Row` 表示完整调用面 contract，而不是 direct-only lower bound。
- 调用点检查使用 callee 发布的 `published_surface_row`。
- 函数类型 `/ Row` 仍表示该函数值调用时的 surface row，不参与 direct-row 简化。
- public concrete 函数省略 `/ Row` 不再表示“该函数调用面 Pure”，而表示“如果 direct effect 为 Pure，则推导并发布 `published_surface_row`；如果 direct effect 非 Pure，则要求显式声明”。

## Implementation Sketch

建议分阶段迁移。

1. 在 HIR/typecheck facts 中拆出 `declared_surface_row`、`direct_effect_row`、`inferred_surface_row_template` 与 `published_surface_row_template`。
2. 将当前 `required_effects` 收集结果从“只用于本函数报错”改为“发布本函数 surface facts”。
3. 调用点 effect checking 改为读取 callee surface facts，而不是只读取 `fun.effects`。
4. 所有非核心表达式形式在 typecheck/HIR 阶段发布 canonical semantic expansion facts；effect inference 只消费这些统一 facts，不为具体语法开后门。
5. MIR/materialized facts 拆出 `actual_surface_row`、`published_surface_row` 和 `step_effect_row`，替换现有混用的 `declared_row` 语义。
6. 递归 SCC 增加 fixed-point 推导或显式 row fallback 诊断。
7. 更新 spec、fixtures、goldens，删除 public omitted effect 必报错的旧规则。

## Open Questions

- source-level canonical semantic expansion facts 的最小稳定 schema 如何设计，才能同时服务 typecheck、HIR lowering 与 MIR lowering。
