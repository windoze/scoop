# Scoop：下一轮计划（核心语言 / codegen 优先，Task 次之）

> 生成时间：2026-04-18  
> 历史归档：`PLAN-4.md` / `TODO-4.md`  
> 依据：`ISSUES.md` 当前审计结果  
> 本轮主题：先收口核心语言与 codegen 缺口，再推进 effect 完整性与 `Task` 设计；executor framework 明确留到下一阶段。
>
> 2026-04-18 当前轮完成更新：`T4001` 与 `T4001R` 已完成。`where` 子句现已支持带类型实参的 nominal bound；`TypeEnv` / `TypeLowering` 会保留并具体化 direct supertypes；assignable / 上转规则已改为沿具体化后的 supertype 链做 DFS；`*` 现作为 typecheck 内部 star projection 保留，并在导出 / RTTI / LLVM 侧擦除为 `Any?` 读视图。复审确认上述语义走统一主线，没有新增 `Array` / `Collection` / 单个 interface 的旁路特判，也没有把 star projection 在前端主线上重新降回 `Any`。已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (326)`）与 T4001 定向 run-pass（`target/debug/scoop test --fixtures target/t4001r-fixtures/run-pass`，`fixtures: ok (2)`）。当前下一项推进到 `T4002`。
>
> 2026-04-19 当前轮完成更新：`T4002` 已完成。lambda 推断现统一按函数签名驱动：expected-type 向下传播不再写死 0/1/2 参数；无 expected type 的 lambda 在“显式参数类型”与零参数场景下也能直接定型；receiver lambda 的隐式 `this` 已在 resolver / typecheck / HIR / LLVM closure codegen 主线上贯通，并按 receiver 实际类型完成 member access / method call late resolve。补充回归覆盖了多参数 expected-type 推断、无 expected type 的显式参数 lambda、scope functions 的 receiver lambda，以及 receiver lambda 遮蔽外层 `this` 的执行路径。已验证 `target/debug/scoop test --fixtures target/t4002-fixtures/infer`（`fixtures: ok (4)`）、`target/debug/scoop test --fixtures target/t4002-fixtures/run-pass`（`fixtures: ok (4)`）、`target/debug/scoop test --fixtures tests/fixtures/typecheck`（`fixtures: ok (326)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。当前下一项推进到 `T4002R`。
>
> 2026-04-19 当前轮完成更新：`T4002R` 已完成。复审确认 lambda expected-type 推断仍统一走“按函数签名收集候选，再仅对 lambda 实参进入 expected-context typecheck”的主线，没有回流到按 direct/member/function-value 某一调用形状分别补推断；同时发现并修复了一个既有 lowering 裂缝：嵌套 receiver lambda 的 `this.member(...)` 在 typecheck 阶段会按 receiver 实际类型晚解析，但 HIR lowering 仍可能沿用 resolver 的旧成员决议与旧 `this` 绑定，导致 `receiver_lambda_this_shadows_outer_this` 错误输出 `99`。当前已通过 `typechecked_member_resolved` side table 与 receiver-lambda lowering `this` 上下文统一成员最终决议，并让内建字符串/标量成员方法继续保留为后端 intrinsic member-call 路径，修复后该回归输出已改为 `3`。已验证 `target/debug/scoop test --fixtures target/t4002r-fixtures/infer`（`fixtures: ok (1)`）、`target/debug/scoop test --fixtures target/t4002r-fixtures/run-pass`（`fixtures: ok (4)`）、`target/debug/scoop test --fixtures tests/fixtures/typecheck`（`fixtures: ok (326)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。当前下一项推进到 `T4003`。
>
> 2026-04-19 当前轮计划调整：原 `T4003` 同时覆盖 `FunPtr<F>` receiver signature、顶层泛型函数值 / `callee<T>` 一等值、以及 ctor delegation 的命名 / 默认参数绑定，横跨 typecheck 公共调用绑定、HIR/lowering 表示与 class ctor side table 三套基础设施，单轮完整收口风险过高。现已将其拆分为 `T4003a -> T4003b -> T4003c`：先打通 `FunPtr` receiver function type 调用主线，再处理顶层泛型函数值与 `callee<T>`，最后收口函数值 / funptr / ctor delegation 的命名实参和默认参数绑定。当前本轮执行目标切换为 `T4003a`。
>
> 2026-04-19 当前轮完成更新：`T4003a` 已完成。`FunPtr<F>` 不再把 receiver function type 当作 early error；typecheck 中的 funptr direct call 已改为与函数值调用一致，按“receiver 作为第 0 个显式实参”检查；LLVM indirect funptr call 也同步支持 receiver 参数位。与此同时，sysroot `scoop.unsafe.FunPtr` 现在补齐了 receiver 形态的 `invoke` overload，并在 intrinsic codegen 入口把 named args 按 `receiver` / `a0` / `a1` 约定重排为位置实参，因此 `fp(receiver, arg)`、`fp.invoke(receiver, arg)` 与 `fp.invoke(receiver = ..., a0 = ...)` 现已统一可执行。已新增 `unsafe_funptr_receiver_call_basic` run-pass 回归，并复验既有 funptr 运行用例。已验证 `cargo run -p scoop -- test --fixtures target/t4003a-fixtures/run-pass`（`fixtures: ok (3)`）、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (326)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。当前下一项推进到 `T4003b`。
>
> 2026-04-19 当前轮完成更新：`T4003b` 已完成。顶层函数在值位置现在可形成一等函数值，`callee<T>` 也不再限于“仅能作为直接调用 callee 的透明包装”，而是可以被赋值、传递给 higher-order 调用并在后续再次调用。实现上，AST / parser / typecheck 新增了顶层函数值的 side table 与 expected-context 反推路径：bare 顶层函数值与 `callee<T>` 会先进入统一的函数值推断，generic function value 可根据 expected function type 反推出 type args，而 higher-order 场景下 bare 顶层函数值候选会先以占位 `Any` 参与参数预收集，避免在 expected-type 生效前过早报 `generic_type_arg_not_inferred`。HIR lowering 则把最终选中的顶层函数值统一合成为零捕获 closure 包装，直接复用现有 function-value call / monomorph / codegen 主线，没有再引入第三套运行时表示。parser 同时放宽了 type-apply lookahead，因此 `callee<T>` 既能继续直接调用，也能作为普通值表达式存在。已新增 `top_level_generic_function_value_basic` run-pass 与 `top_level_generic_function_value_needs_type_info_is_error` typecheck 回归；已验证定向 build+run、定向 run-pass / typecheck fixtures、全量 `tests/fixtures/typecheck`（`fixtures: ok (327)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。当前下一项推进到 `T4003c`。
>
> 2026-04-19 当前轮完成更新：`T4003c` 已完成。函数值 direct call、direct `FunPtr` call 与 class ctor / ctor delegation 现在共用一套命名实参与默认参数绑定主线：typecheck 侧对函数值 / funptr 使用统一的合成形参名 `receiver` / `a0` / `a1` / ... 做 named-arg 映射，ctor 侧则把“已选中的 ctor 目标 + `arg_mapping`”写入新的 side table，供 HIR lowering 与 LLVM codegen 直接消费，不再按 arity 猜目标或在后端重做一遍绑定。ctor 参数默认值也已进入 HIR / LLVM 主线，显式实参按源码顺序求值，缺失形参再按绑定后的形参顺序补默认值；`class header : Base(...)`、secondary ctor `: this(...) / : super(...)`、以及普通 direct class ctor call 现已全部走同一套绑定数据。收口过程中还顺手把 effect state-machine 消费 ctor call target 的旧接口切到 `CtorCallInfo`，并给无完整 typecheck 的 IR 测试入口补了 resolver 级 ctor fallback，避免 `emit_minimal_main_ir` 在 direct class ctor call 上退化到 enum variant ctor 分支。已新增 `function_value_named_args_basic`、`unsafe_funptr_direct_named_call_basic`、`class_ctor_named_default_and_delegation_basic` 三个 run-pass 回归，并验证定向 build+run、`/tmp/t4003c-fixtures` run-pass 子集（`fixtures: ok (6)`）、`/tmp/t4003c-typecheck` 子集（`fixtures: ok (10)`）、全量 `tests/fixtures/typecheck`（`fixtures: ok (327)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。当前下一项推进到 `T4003R`。
>
> 2026-04-19 当前轮完成更新：`T4003R` 已完成。复审确认调用系统先前仍有一条后端裂缝：function value / funptr 的命名实参已经在 LLVM 侧走统一 callable-value binder，但顶层 direct call、vtable member call 与 itable member call 仍各自要求纯位置实参，导致 `f(b = ..., a = ...)` / `obj.mix(b = ..., a = ...)` 在 build 阶段掉进 `named call arg` / `named vtable call arg` / `named itable call arg`；此外，顶层泛型 direct call 的 monomorph FQN 解析也仍按位置索引读取实参，无法和命名实参共享同一套 concrete type 绑定。当前已将 LLVM 侧参数绑定收口为共享的 `map_call_args_to_params_by_name` + `codegen_bound_call_args` 主线，供 direct call、vtable、itable、function-value 与 funptr 共用；`scoop.unsafe.invoke` 也改为直接复用 funptr binder，不再单独重排命名实参。与此同时，泛型顶层 direct call 的 monomorph 重写已切到同一套命名映射，确保 `pick(b = ..., a = ...)` 这类调用仍能命中正确实例。已新增 `top_level_generic_named_args_basic`、`member_call_virtual_named_args_basic`、`member_call_interface_named_args_basic` 三个 run-pass 回归，并与既有 `function_value_named_args_basic` / `unsafe_funptr_*` 回归一起在 `/tmp/t4003r-run-pass` 子集验证通过（`fixtures: ok (6)`）；同时复验了 `tests/fixtures/typecheck`（`fixtures: ok (327)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。当前下一项推进到 `T4004`。
>
> 2026-04-19 当前轮计划调整：在正式实现 `T4004` 前，先用最小 probe `val x: Int = 41; fun main(): Int { return x + 1 }` 验证顶层 immutable value 主线，结果 `cargo run -p scoop -- build /tmp/t4004_plain_top_level_val_probe.scoop -o /tmp/t4004_plain_top_level_val_probe.out` 直接在 LLVM codegen 阶段报 `scoop::llvm::unsupported_main_body: top-level value ref`。这说明“普通顶层 `val` 的可执行读取语义”本身仍未完成，当前后端只真正支持 `const val` 与显式静态存储的顶层 `var`；而顶层 pattern binding 会把多个 binder 暴露成普通顶层 immutable value，如果继续实现 `T4004`，就只能错误地把它绑到 `const` / 静态存储旁路上。基于这一新发现，现已在 `T4003R` 与 `T4004` 之间插入新的前置任务 `T4003S -> T4003SR`：先收口普通顶层 `val` 的 HIR / LLVM 表示与读取主线，再继续做顶层 pattern binding。当前本轮到此停止，等待下一次调用从 `T4003S` 开始。

## 0. 工作原则

- 本轮严格按 `ISSUES.md` 指定顺序推进，不提前展开后续条目。
- 每个 issue 至少要完成四件事：实现、回归用例、必要的规范/文档同步、复审。
- 未完成前一条 issue 前，不开始后一条 issue 的实现任务；允许在同一条 issue 内拆子任务，但不得跨条目并行推进。
- 本轮的目标是“核心语言 / lowering / codegen 收口优先”。只有 effect / `Task` 之前的所有条目完成后，才进入这两项。
- `Continuation<T, eff E>` 视为 advanced API；`Task<T>` 视为 general API。
- executor、wakeup、queueing、work-stealing、spawn scheduling 统一留到下一阶段；本轮不把它们纳入完成标准。

## 1. 顺序总览

1. `ISSUES.md` 第 5 条：泛型约束、参数化超类型与 star projection
2. `ISSUES.md` 第 3 条：lambda 推断与 receiver lambda
3. `ISSUES.md` 第 4 条：调用语义早期门禁
4. 新增 blocker：普通顶层 `val` 的可执行读取语义
5. `ISSUES.md` 第 6 条：顶层 pattern binding
6. `ISSUES.md` 第 13 条：Elvis `?:` lowering / codegen
7. `ISSUES.md` 第 14 条：跨文件 / 跨包编译链路
8. `ISSUES.md` 第 15 条：RTTI 对泛型 / `eff` 参数化类型的支持
9. `ISSUES.md` 第 1 条：effect / continuation 完整性
10. `ISSUES.md` 第 2 条：`Task` 设计与 pollable object 语义

## 2. 分阶段目标

### P1. 类型系统与子类型关系收口

- 先解决参数化 nominal bound、参数化超类型、star projection。
- 目标是把 generic subtype / assignable / lowering 的基础打稳，避免后续 lambda、调用与跨文件实例化继续叠加在不稳定的类型关系上。
- 当前状态：`T4001` / `T4001R` 已完成；P1 阶段收口完毕，下一步进入 P2 的 `T4002`。

### P2. 表达式推断与调用语义收口

- 依次补齐 lambda 推断、receiver lambda、函数值 / funptr / constructor delegation 的调用语义缺口。
- 目标是把前端最常用的表达式与调用规则统一到同一条类型检查主线上。
- 当前状态：`T4002` / `T4002R` / `T4003a` / `T4003b` / `T4003c` / `T4003R` 已完成；P2 阶段收口完毕，下一项进入 P3 的 `T4003S`。

### P3. 语法到 lowering 的缺口收口

- 先收口普通顶层 `val` 的可执行读取语义，再推进顶层 pattern binding 与 Elvis `?:` 的 lowering / codegen。
- 目标是把“语法 + typecheck 已存在，但 lowering / codegen 不完整”的 feature 清掉，避免继续堆积半实现特性。
- 当前状态：`T4004` 开始前确认到一个新的前置 blocker：普通顶层 `val` 读取仍会在 LLVM codegen 阶段报 `top-level value ref`。因此 P3 现在先执行 `T4003S` / `T4003SR`，完成后再继续 `T4004`。

### P4. compilation-unit 与 runtime type info 收口

- 跨文件 / 跨包 compilation chain 与 RTTI 参数化支持放在同一阶段处理。
- 目标是先让语言规则跨 compilation unit 一致，再补运行时类型描述符对泛型 / `eff` 的覆盖。

### P5. effect 完整性收口

- 在核心语言与 codegen feature 收口后，再回头补 effect / continuation 剩余缺口。
- 目标不是扩 executor，而是把手动 stepping `Task` 所需的 effect 语义补完整，包括更自然的多 suspend 组合、continuation 类型语义与相关 lowering。

### P6. `Task` 设计定型

- 只聚焦 `Task<T>` 本体：pollable object、manual stepping、private continuation state、advanced `Continuation` 隐藏边界。
- 不在本轮定义 executor interface、wakeup API 或 `spawn` 最终调度模型。

## 3. 各阶段完成标准

### C1. effect / `Task` 之前的核心语言 / codegen 条目

- 对应 `ISSUES.md` 条目已被关闭，或至少收缩为新的、更窄的剩余 blocker。
- 新增或更新的 fixtures 覆盖 typecheck、HIR / MIR / LLVM lowering、run-pass 或相关 regression。
- 若规范文字被实现改变或澄清，需同步 `SCOOP_FULL_SPEC.md`，必要时同步相关 runtime / sysroot 文档。

### C2. effect / continuation 条目

- 明确区分“已足够支撑 `Task` manual stepping 的能力”和“仍未覆盖的 richer effect 语义”。
- 去掉阻碍 `Task` 设计落地的主要 effect/codegen 限制，尤其是 continuation 组合与相关 lowering 缺口。

### C3. `Task` 条目

- `Task<T>` 的通用 API 形状、`Poll<T>` 合同与 manual stepping 语义要固定下来。
- raw `Continuation` 不应继续作为通用 async API 暴露；若仍需保留，只能作为 advanced API。
- executor 仍可留白，但 `Task` 本体不再依赖 executor-centric 的叙事才能成立。

## 4. 非目标

- 本轮不完成 executor framework。
- 本轮不定义 work-stealing、event loop、I/O driver、waker、queueing 或 `spawn` 的最终调度语义。
- 本轮不扩展与上述九项无直接关系的 stdlib surface。

## 5. 最终验收

- `PLAN.md` 与 `TODO.md` 中本轮任务已按顺序推进并留下明确结论。
- 相关实现通过必要的定向测试；阶段收口时复验 `cargo test --all` 与 `cargo run -p scoop -- test`。
- 若修改了 `SCOOP_FULL_SPEC.md` 中带 fixture 的代码块，还需执行 `cargo run -p scoop_tools -- spec-fixtures check`。
