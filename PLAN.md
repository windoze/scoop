# Scoop：当前计划（effect 主线优先，后续任务顺延）

> 生成时间：2026-04-15  
> 历史归档：`PLAN-3.md` / `TODO-3.md`  
> 范围：本计划先覆盖当前 effect 统一主线（`T30`）；为避免下一批任务继续停留在归档里，也顺延保留前端 / 并发 / 类型系统的后续队列（`T31`～`T34`）。当前执行顺序仍以 `T30` 全部收口为先。

## 0. 工作原则

- 当前最高优先级是 `T30`；`T31`～`T34` 只作为 effect 主线收口后的后续队列，不与当前修线争抢顺序。
- `T30` 继续遵守“删除优先于修补”。看到 shape-based 生产逻辑就直接删除，不以“先补一个 case”维持旧路径。
- `T30` 中 LLVM effect codegen 的单一输入是 state machine。除类型、符号与 ABI 必需信息外，不能再读取源码形状、旧 scanner 结果或旧分类器输出。
- `T30` 当前阶段只实现 heap-allocated full state machine lowering，不做 simplification，不做模式化优化。
- `T30` 中每个实现任务后立即插入一个 review 任务；review 必须显式确认生产代码中不存在 shape-based logic。
- `T30` 的 review 范围只看生产代码，重点是 `crates/scoopc/src/llvm/codegen/**`；测试命名不作为问题。
- `T31`～`T34` 维持“小步可回归”原则：先收口语义与表示，再扩展 lowering / runtime / 测试；除显式写出的依赖外，不额外插入 effect 风格的 review 子任务。

## 1. 当前状态与已知缺口（T30）

- `T2999` 已完成：
  - `cargo check -p scoopc` 已恢复零 warning。
  - `cargo clippy --all-targets -- -D warnings` 已通过。
  - `scoop.core.__scoop_effect_*` sysroot 测试辅助 intrinsic 已重新直连 runtime ABI，`cargo test --all` 已恢复通过。
- `T2999R` 已完成：
  - 已删除 `runtime_abi.rs` 中无生产调用点、也不属于当前统一 effect 合同的 `declare_runtime_alloc` / `declare_runtime_gc_collect`。
  - 已把 `runtime_symbols.rs` 中散落的冗余 `#[allow(dead_code)]` 清掉，并删除 `state_machine_plan.rs` / `state_machine_transform.rs` 中被统一骨架边界覆盖的重复豁免。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- `T3001` 已完成：
  - 已从 `crates/scoopc/src/llvm/codegen/mod.rs` 删除 `CalleeSuspendResumeMode`、`scan_for_callee_suspend`、`codegen_top_level_fun_suspendable`、`codegen_closure_fun_body_suspendable` 及其入口接线。
  - 顶层函数与 closure 的 codegen 已收口回常规路径，不再按 `perform` 所在源码形状选择专用 suspendable lowering。
  - 已同步清理 `effect/mod.rs`、`runtime_abi.rs`、`runtime_symbols.rs` 中仅服务于这条旧路径的 helper / ABI 声明。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。
- `T3001R` 已完成：
  - 已定向检索 `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/mod.rs` 与相邻调用点，确认删除后的旧 callee-shape scanner / mode enum / suspendable top-level/closure route 没有换名回流。
  - 已复查 `codegen_top_level_fun`、`codegen_closure_fun_body`、`codegen_top_level_fun_call` 与 `ExprKind::Perform` / `ExprKind::Handle` 接线，确认当前只剩常规函数/闭包 codegen 与统一 effect 占位入口，不再按源码 / callee 形状分流。
  - 已重新验证 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all` 全部通过。
- `crates/scoopc/src/llvm/codegen/effect/mod.rs` 统一入口仍未完全接到真正的 state-machine-driven LLVM lowering。
- 这意味着当前 effect codegen 虽然已经摆脱 `mod.rs` 的旧 callee-suspend 主分流，并保留了统一的 segmentation / state machine transformation 基线，但 LLVM 生产主线仍未完成统一 state-machine-driven lowering 接线。

## 2. 阶段顺序

### 阶段 0：先恢复零 warning 基线

#### T2999：清理当前 `scoopc` 基线中的编译 / lint 警告（已完成）
- 先处理当前基线已经存在的 `dead_code` / `unused` 级警告，恢复 `cargo check -p scoopc` 与 `cargo clippy --all-targets -- -D warnings` 的可通过状态。
- 原则是删除无价值死代码，或为确有保留理由的骨架建立可审计边界；不能用模糊的允许属性长期压住真实缺口。
- 本轮结果：
  - 统一 state-machine 骨架改为单一共享作用域的保留边界，避免散落 `allow`。
  - effect runtime ABI 与相关符号表的保留边界已显式收口。
  - 顺手修复了既有的 sysroot effect intrinsic 回归，保证全量测试恢复绿色。

#### T2999R：Review（已完成）
- 审查 warning 清理后的 effect / LLVM 相关生产代码，确认零 warning 基线不是靠临时压制或掩盖实现问题达成。
- 本轮结果：
  - 删除了不属于当前统一 effect 合同的无调用点 ABI 声明，避免继续靠 `allow(dead_code)` 留存。
  - 把 runtime symbol table 与 unified state-machine 骨架中的重复 `dead_code` 允许项收口回已有共享边界。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

### 阶段 A：先把残余 shape-based 主路径删干净

#### T3001：删除 `llvm/codegen/mod.rs` 中剩余的 callee-suspend shape-based 主路径（已完成）
- 先删 `mod.rs` 里的旧 callee-suspend 路线，不允许顶层函数或 closure 再按源码形状走专用 lowering。
- 本轮结果：
  - 旧的 callee-shape scanner、mode enum 与 top-level / closure suspendable route 已从生产代码移除。
  - 与该路径绑定的 effect helper 与 runtime ABI 声明已同步删除，避免形成新的死代码边界。
  - 删除后无需额外补丁即可维持编译、lint 与现有测试全绿。

#### T3001R：Review（已完成）
- 定向检查 `mod.rs` 与调用点，确认旧 callee-shape scanner / mode enum / suspendable top-level/closure 路线已经完全消失，没有换名保留。
- 本轮结果：
  - `ExprKind::Perform` / `ExprKind::Handle` 统一直接进入 `effect/mod.rs`，没有在 `mod.rs` 或 `expr.rs` 中先按形状挑选另一套 lowering。
  - `codegen_top_level_fun`、`codegen_closure_fun_body`、`codegen_top_level_fun_call` 当前仅保留常规路径；effect 相关调用只保留基于函数 effect row 的 flag-unwind 检查，不涉及 callee/source shape 分流。
  - 复验通过：`cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`。

#### T3002：清扫 effect codegen 生产代码中的其余 shape-based 分流
- 在 `mod.rs` 清掉之后，继续删除 effect codegen 里的旧 scanner、旧 resolver、旧 dedicated matrix、旧 fallback route。
- 目标不是把它们改成“更像统一主线”，而是让它们不再构成生产主路径。

#### T3002R：Review
- 审查 `crates/scoopc/src/llvm/codegen/**`，确认生产代码里已经不存在按源码 / site / arm / callee 形状做主选路的 effect codegen。

### 阶段 B：冻结 LLVM lowering 的唯一输入面

#### T3003：冻结 `state machine -> LLVM lowering` 的统一合同
- 在真正发射 LLVM 之前，先把 emitter 输入收口到统一合同。
- 这个合同必须完整表达 state、edge、suspend/resume、cleanup、frame layout、dispatch、capture / payload / slot 等 lowering 所需信息。
- 合同的全部结构信息都必须来自 state machine 本身，而不是回头看 HIR shape 或旧 scanner 输出。

#### T3003R：Review
- 审查 lowering 入口及其依赖链，确认 LLVM lowering 主线的结构输入只剩 state machine。

### 阶段 C：实现 full state machine LLVM emitter

#### T3004：实现 heap-allocated full state machine 的 LLVM lowering 主体
- 从统一 state machine 发射 frame、`pc` dispatch、state block、resume edge、cleanup/unwind、handler stack 交互与 payload transport。
- 当前阶段不做 simplification；即使后续会优化，也必须先保证 full state machine 语义完整可发射。

#### T3004R：Review
- 审查 emitter 主体，确认所有发射分支都来自 state machine 语义边，而不是代码形状推断。

### 阶段 D：把统一 emitter 接回生产入口

#### T3005：将统一 state-machine LLVM lowering 接回 effect codegen 主入口
- `handle` / `perform` / continuation 相关入口都要接到统一 emitter。
- 旧主线路由、旧 fallback、旧占位入口要同步删除或彻底失效，不能保留“双轨”。

#### T3005R：Review
- 审查 effect codegen 主入口，确认统一 lowering 已经成为唯一主路径，不存在“失败时退回旧路线”的隐藏逻辑。

### 阶段 E：用测试补齐覆盖，但修复必须仍在统一主线内完成

#### T3006：补齐统一 LLVM lowering 的定向测试与代表性 fixture
- 这一步的目标是确认当前 state machine 合同与 LLVM lowering 真正能覆盖合法输入。
- 如果测试暴露出缺口，先补 plan builder / state machine / emitter 合同，再继续测试；不能为了过例子新增 shape-based 快捷分支。
- 当前阶段以定向测试为主，不把 full suite 当作前置门槛。

#### T3006R：Review
- 审查测试修复后的生产代码，确认没有因为补 case 把 shape-based logic 带回主线。

### 阶段 F：收尾 legacy 清理

#### T3007：删除统一主线接管后剩余的 legacy effect codegen 死代码
- 在统一 LLVM 主线稳定后，继续删除仓库里剩余的 legacy effect codegen 文件、helper、注释与过渡分支。
- 目标是让仓库结构本身也与“统一主线”一致，而不是代码虽然不走、但旧实现还大面积留存。

#### T3007R：Review
- 最终审查 effect codegen 生产实现，确认仓库中只剩统一主线，没有可重新接回的 shape-based legacy。

### 阶段 G：effect 主线收口后，切回 `do` block / closure 消歧

#### T3101：Parser / AST 引入显式 `do { ... }` block，并将裸 `{}` 固定为 closure
- 普通局部 block 统一写作 `do { ... }`；没有 `do` 的 `{ ... }` 一律按 closure / trailing lambda 规则解析。
- parser / AST 必须为 `DoBlock` 与 `Closure` 保留稳定且可区分的形状，避免后续阶段继续依赖上下文猜测。

#### T3102：Typecheck / HIR 收口 `do` block 的 expression statement 与 tail value 语义
- 统一“只有未终止 tail expr 才产生 block 值；`expr;` 只是 expression statement，结果视为 `Unit`”。
- `if` / `when` / `handle` / lambda body / `do` block 的值语义都要按同一规则收口。

#### T3103：effect nested-block fixtures 切到 plain `do` block
- 在 `T3006R` 之后，把仅为 nested block 消歧而保留的 `@Safe { ... }` workaround 切回 `do { ... }`。
- 真正依赖 safe-region 语义的测试继续保留 `@Safe`，并同步锁定 multiple trailing lambdas 与 `do` block 的边界规则。

#### T3104：同步规范 / 文档中的 `do` block、closure 优先级与 trailing-lambda 规则
- 更新 `SCOOP_FULL_SPEC.md`、doctest / fixture 示例，以及当前 `TODO.md` / `PLAN.md` 等相关文档叙述。
- 若规范代码块变更，配套完成 `spec-fixtures sync/check`。

### 阶段 H：Structured Concurrency / `Task<T>`

#### T3201：`spawn` / `join` 的 typecheck 与 HIR 去 `Int` 硬编码
- 先把前端表示收口到真实的 `Task<T>`，不再把 handle / result 擦成 `Int`。
- 已确认仍缺失的 lowering / codegen / runtime 缺口，分别由 `T3202`～`T3204` 明确承接。

#### T3202：`spawn` / `join` 语法糖与 sysroot glue 去 `_int` 专用路径
- HIR lowering、block rewrite 与 sysroot internal glue 不再依赖 `__scoop_task_spawn_int` / `__scoop_task_join_int` 这类 `_int` 专用入口。
- desugar 后的 HIR 必须继续保留任务结果类型，给后续 LLVM / runtime 泛型化提供稳定输入。

#### T3203：LLVM codegen 去 `scoop_task_*_int` 专用路径
- codegen 不再把 `Task<T>` 压回 `i64`/`Int` 专线，而是支持 scalar / ref / aggregate / 泛型实例的统一 task payload。
- task payload transport 要尽量与 continuation payload ABI 对齐，避免维护 task-only 特例。

#### T3204：runtime executor / `Task<T>` 完成回调泛型化
- runtime task 状态机、executor job、completion waiter 与 sysroot glue 都不能再固定在 `Task<Int>` / `resume_u64`。
- ref / aggregate payload 在 pinning、GC stress、跨线程或跨 executor 恢复时都要保持稳定语义。

#### T3205：结构化并发回归矩阵与语义锁定
- 用 nested `spawn` / `join`、控制流 join、多任务交错、GC 压力等真实并发场景锁定边界。
- 当前阶段明确不支持的并发组合，要么形成稳定诊断，要么在文档中清楚限制。

### 阶段 I：Lambda 推断与调用语义补齐

#### T3301：expected function type 向任意参数个数传播
- 把 lambda expected-type 传播从 0/1/2 参数推广到任意参数个数。
- 变量初始化、返回语境、调用实参、集合/构造器上下文等常见入口都要统一接入。

#### T3302：receiver lambda 体内 `this` 与成员解析
- receiver lambda 进入 typecheck / lowering 时自动建立 `this` 绑定与成员查找环境。
- `this`、成员访问、扩展调用与闭包捕获的局部作用域规则要与普通 lambda 对齐。

#### T3303：统一函数值 / funptr / ctor delegation 的实参匹配
- 函数值调用、函数指针调用、`super(...)` / `this(...)` 构造器委托调用要共用同一套参数匹配规则。
- 命名实参与 receiver function type 的处理不能再靠零散的早期门禁分流。

### 阶段 J：泛型约束 / Pattern / 值类型能力补齐

#### T3401：`where` nominal bound 支持类型实参与 instantiated supertype 满足性
- 把带类型实参的 nominal bound 贯通到解析、检查、子类型关系与诊断。
- 实例化处的 bound 检查、函数体内成员分发都必须基于实例化后的 bound，而不是回退到未参数化 nominal type。

#### T3401a：`where` nominal bound 的子类型满足性回归矩阵
- 专门锁定接口/类继承链上的实例满足 generic bound 的语义，不依赖当前实现“碰巧有效”。
- 补齐变量透传、泛型 passthrough、builtin/value 类型 boxing 满足 interface bound 等回归。

#### T3401b：`where` bound 驱动的方法分发补齐接口继承链与多 bound 歧义
- 让接口继承链上的成员对 bound receiver 可见，并为多 bound 同名成员建立稳定的候选集 / 歧义规则。
- 不能再按遍历顺序提前返回首个命中项。

#### T3401c：成员方法签名收集与 `where` bound 分发对齐 richer generic/effect 调用
- 普通 member call 与 `where` bound member call 都要支持显式类型实参、2+ type params 与 `<eff E>` 成员方法。
- shared helper 与 top-level call 之间的语义不能继续缩水分叉。

#### T3402：顶层 `val` 支持 pattern binding
- 顶层 tuple / struct / enum destructuring 复用既有 pattern binding 规则，不再保留“顶层只允许标识符”的特判。
- 顶层符号安装、初始化顺序、多文件可见性与循环引用诊断保持稳定。

#### T3403：`struct` 字段支持 `var` 与默认值
- 先收口字段模型，让 `struct` 声明能力覆盖 `var` 字段与默认值。
- 构造、布局、默认值与值语义冲突处都需要统一规则与诊断。

#### T3404：`with` 更新扩展到更完整的值类型语义
- `with` 的 base 类型不再局限于当前最小 `struct` 子集，嵌套字段路径更新要 lower 成稳定的 copy-update 链。
- 诊断必须区分字段不存在、字段不可更新、类型不匹配、base 非值类型等不同错误。

#### T3405：`when` 的 or-pattern 支持共享 binder
- 当各分支 binder 集、名称与类型兼容时，允许 or-pattern 引入共享 binder。
- binder 数量、名称或类型不一致时，要给出具体而稳定的诊断。

## 3. 验收策略

- `T30` 的清理阶段允许临时破坏编译；目标是先删旧主线。
- `T30` 接统一 LLVM emitter 后，再用定向测试恢复并扩大覆盖。
- `T30` 的 review 任务是强制门，不是可选项；任何实现任务完成后，如果 review 发现 shape-based 逻辑回流，必须先回退到清理状态，再进入下一任务。
- `T31`～`T34` 按各自 TODO 中列出的 fixtures / `cargo test` / LLVM run-pass 验收，不额外插入独立 review gate。

## 4. 当前执行顺序

1. `T3002`
2. `T3002R`
3. `T3003`
4. `T3003R`
5. `T3004`
6. `T3004R`
7. `T3005`
8. `T3005R`
9. `T3006`
10. `T3006R`
11. `T3007`
12. `T3007R`
13. `T3101`
14. `T3102`
15. `T3103`
16. `T3104`
17. `T3201`
18. `T3202`
19. `T3203`
20. `T3204`
21. `T3205`
22. `T3301`
23. `T3302`
24. `T3303`
25. `T3401`
26. `T3401a`
27. `T3401b`
28. `T3401c`
29. `T3402`
30. `T3403`
31. `T3404`
32. `T3405`
