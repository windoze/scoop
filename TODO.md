# TODO（Scoop：近期任务清单）

> 生成时间：2026-04-11  
> 说明：本文件是新的短版 TODO，只记录“接下来要做的新任务”。历史任务与已完成事项请看 `TODO-2.md` / `PLAN-2.md`。  
> 范围：本轮只覆盖 `ISSUES.md` 中确认仍存在的语言特性 / 编译器实现缺口，不把下一阶段的 stdlib 扩面混入主线。

## 约定

- 状态：
  - `[TODO]`：可立即实现与验收
  - `[BLOCKED]`：依赖未满足（例如缺文件/缺前置能力）
  - `[DONE]`：已完成（短版 TODO 一般不搬运历史 DONE）
- 每个任务包含：**描述 / 目标 / 验收 / 依赖**。
- 本轮优先级：
  - 主线：effect / continuation、`Task<T>`、lambda / 调用语义、泛型约束 / pattern / 值类型、`const fun` / MIR。
  - 末尾低优先级：annotation class、FFI / calling convention。

常用验收命令：

```bash
cargo test --all
cargo run -p scoop_tools -- spec-fixtures check
cargo run -p scoop -- test
```

LLVM 端到端（本机需 `clang` + `llvm-config`）：

```bash
cargo run -p scoop --features llvm -- test
```

---

## T20：Effect / Continuation 完整化

### T2001 [DONE] Effect：统一 `handle` arm 形态与 typecheck/HIR 不变量
- 描述：当前 `handle` 仍直接拒绝在同一个表达式里混用 `->`、`-> resume`、`, k ->` 等 arm 形态，导致语言语义被实现层的早期门禁截断。先收口 arm 的表示与兼容性检查，再推进后端链路。
- 目标：
  - typecheck 不再用“是否混用 arm 形态”作为直接拒绝条件，而是按 op 签名、resume 模式、binder 约束做真实兼容性检查。
  - HIR 为 handler arm 保留足够信息，能够区分 non-resuming / immediate-resume / continuation-binder 三类语义，而不是在 lowering 前折叠或拒绝。
  - 对不兼容组合给出稳定诊断，避免把语义错误延后成 LLVM/codegen 阶段崩溃。
- 验收：
  - 新增 typecheck / HIR fixtures：合法 mixed-arm、非法 mixed-arm、binder / resume 不匹配。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无
- 完成说明：
  - 已移除 mixed-arm 的统一 early reject，并把 `handle` 结果类型改成按真实可返回路径确定。
  - 已补 mixed-arm typecheck/HIR fixtures，覆盖合法组合、返回类型不匹配、resume 语义冲突不可达。

### T2002a [DONE] Effect：non-resuming 单 payload ABI 泛化（direct + indirect perform）
- 描述：当前自定义 non-resuming effect 的 codegen 虽然已经能通过 flag-propagation + handler stack 跨函数分发，但 payload / handler binder 仍被硬编码为单个 word-sized `Int`。这使 `String`、引用类型以及含引用字段的聚合值在 non-resuming handler 上仍不可用。
- 目标：
  - 单 binder / 单 payload 的 non-resuming effect 不再要求 payload 为 `Int`；支持 scalar、`String` / ref，以及常见 aggregate payload。
  - direct perform 与通过函数/闭包触发的 indirect perform 共享同一套 payload encode/decode 语义，并具备正确的 GC rooting。
  - 保持既有 non-resuming dispatch 语义不回归：最近 handler 优先、arm body 在自身 handler scope 外执行、flag-propagation 仍可跨函数传播。
- 验收：
  - 新增 run-pass fixtures：non-resuming effect 传 `String` / `struct`，且至少一例经由函数或闭包的 indirect perform。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2001
- 完成说明：
  - runtime perform slot 已新增 `gc_ref` 通道，并在 slot 生命周期内负责 pin/unpin，避免 `String` / ref / boxed aggregate payload 成为 TLS roots hole。
  - LLVM codegen 已为 non-resuming perform / handler 引入共享 payload encode/decode helper，并让 `Continuation.resume` 复用同一套 ABI 规则。
  - 已新增 run-pass fixtures：`effect_nonresuming_payload_string_direct`、`effect_nonresuming_payload_struct_indirect`；`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2002b [DONE] Effect：escape continuation / CalleeSuspendState 恢复值 ABI 泛化
- 描述：`Continuation.resume` 本身已能传递 ref / compound payload，但 escape continuation 的间接 perform / call-site suspension 路径仍主要按 `resume_word` / 标量恢复值建模。top-level function 与 closure 的 CalleeSuspendState 还没有和 continuation 的双通道 payload 语义对齐。
- 目标：
  - top-level function / closure 的 CalleeSuspendState 不再只支持 `resume_word` 标量恢复值；间接 perform 的恢复值可覆盖 `String` / ref / aggregate。
  - 间接 perform + resume 的跨函数路径与 direct continuation step 共享同一套 payload encode/decode helper，而不是继续维护 `Int` 专用分支。
  - 对既有 `Continuation.resume(...)` lowering 做收口，确保 effect 路径和 continuation 路径的 payload 规则保持一致。
- 验收：
  - 新增 run-pass fixtures：间接 perform + `resume(String)`、间接 perform + `resume(struct)` 或等价 aggregate payload。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2002a
- 完成说明：
  - `CalleeSuspendState` 已统一为 `resume_word + resume_gc_ref + locals...` 形状，并让 GC trace 从 `resume_gc_ref` 起点覆盖恢复值与后续 GC locals。
  - top-level function / closure 的 callee-suspend resume path 已复用 `decode_abi_payload_transport`，恢复值不再局限于 `resume_word` 标量分支。
  - 间接 perform 的 escape continuation step 已把双通道 payload 写回 callee state，并对 `resume_gc_ref` 槽位走写屏障。
  - 已新增 run-pass fixtures：`effect_escape_continuation_indirect_perform_resume_string`、`effect_escape_continuation_indirect_perform_resume_struct_with_ref`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003a [DONE] Effect：immediate-resume 单-perform 路径的 `finally` cleanup 语义
- 描述：当前 immediate-resume 的 LLVM lowering 仍是“单个 `val x = perform` + 栈 state machine”的最小实现，并在 codegen 入口直接拒绝 `finally`。先在这个已经可运行的单 suspension 子集上补齐 cleanup 语义，再继续扩展控制流恢复。
- 目标：
  - 现有单个 `val x = perform` immediate-resume handle 可与 `finally` 组合。
  - `finally` 在正常 resume 完成、arm/body raise 向外传播时都恰好执行一次，不漏跑也不重复跑。
  - 不回归 `resume(value)` 的 one-shot 断言、handler inactive/active 切换与 handler scope 边界。
- 验收：
  - 新增 run-pass fixtures：immediate-resume + `finally` 的正常路径、raise 路径。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2002b
- 完成说明：
  - `codegen_handle_expr_immediate_resume` 已新增 `finally` / `finally_unwind` 收口：state0、arm、state1 内的 raise 统一先清理 handler frame，再执行 `finally` 并向外传播。
  - resumed computation 正常完成后会先退出 handler scope，再执行 `finally`，保持与既有 non-resuming / escape continuation 路径一致的 cleanup 顺序。
  - 已新增 run-pass fixtures：`effect_resume_finally_normal`、`effect_resume_finally_arm_raise`、`effect_resume_finally_body_raise_after_resume`。
  - `cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003b1 [DONE] Effect：immediate-resume 嵌套 block 中单 direct perform 的恢复
- 描述：`T2003b` 原始范围同时覆盖 block / branch / while 三类控制流，单轮改动和回归面过大。先收口最小但真实的“statement-position block 嵌套 + 单个 direct perform”路径，验证 immediate-resume 已不再局限于顶层 `val x = perform`。
- 目标：
  - immediate-resume 允许 `perform` 出现在嵌套 block 的语句列表中，而不再只接受 handle body 顶层局部绑定。
  - `resume(value)` 后可从该 block 内正确语句位置继续执行，并继续回到外层 handle body。
  - 对 if / while / value-position 嵌套 perform 先保留稳定诊断，不在本子任务里混入。
- 验收：
  - 新增 run-pass fixture：nested block 中 direct perform 的 immediate-resume。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003a
- 完成说明：
  - LLVM immediate-resume lowering 现已支持 statement-position nested block 中的单个 direct perform，不再只接受 handle body 顶层 `val x = perform`。
  - resume 后会先继续执行命中的 block tail，再回到外层 handle body；perform 前 block locals 的 slot 会跨 suspend/resume 复用。
  - 已新增 run-pass fixture：`effect_resume_nested_block_single_perform`；`cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003b2 [DONE] Effect：immediate-resume branch 内 direct perform 的恢复
- 描述：在 block 嵌套恢复打通后，再把 immediate-resume 扩到 `if` 分支中的 direct perform，补齐“分支命中 perform / 未命中 perform”两侧 CFG 与恢复后的合流。
- 目标：
  - immediate-resume 可覆盖 `if` then/else block 中的 direct perform。
  - resume 后能从命中的 branch 内正确位置继续执行，并在 branch 结束后回到外层后续语句。
  - 未命中 perform 的分支仍按普通控制流执行，不引入伪 suspension。
- 验收：
  - 新增 run-pass fixtures：then/else branch 中的 immediate-resume 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003b1
- 完成说明：
  - immediate-resume 的路径扫描与 lowering 已从“仅 statement-position block”扩展到 statement-position `if` then/else branch，并继续保持“单 direct perform + 单次 resume”约束。
  - `state0` 现已区分“命中 perform 的分支”和“未命中 perform 的分支”：前者进入 arm/resume state machine，后者按普通分支控制流直接完成 handle，不再产生伪 suspension。
  - 已新增 run-pass fixtures：`effect_resume_if_then_branch_single_perform`、`effect_resume_if_else_branch_single_perform`。
  - `cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。

### T2003b3 [TODO] Effect：immediate-resume while 内 direct perform 的恢复与诊断收口
- 描述：最后处理 loop 场景，把 direct perform 放进 `while` body，并对本阶段仍未覆盖的嵌套形状统一为稳定诊断。
- 目标：
  - immediate-resume 可覆盖 `while` body 中的 direct perform，并在 resume 后从循环体内正确位置继续执行。
  - loop locals / binder / one-shot resume 语义在多次迭代下保持稳定。
  - 对当前仍未支持的形状给出稳定诊断，而不是在 LLVM 阶段静默错编。
- 验收：
  - 新增 run-pass fixture：while body 中的 immediate-resume 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003b2

### T2003c [TODO] Effect：immediate-resume 高风险组合回归矩阵
- 描述：在 `finally` 与控制流主链路落地后，补 mixed-arm、nested handle、GC stress 等高风险组合回归，锁定语义边界并防止后续 ABI / lowering 调整回归。
- 目标：
  - 为 mixed-arm、nested handle、GC stress 等组合补齐端到端回归。
  - 相关 immediate-resume 用例在 `SCOOP_GC_STRESS=1` 下稳定。
  - 明确记录当前阶段仍不支持的组合，避免语义漂移。
- 验收：
  - 新增 run-pass fixtures：mixed-arm / nested handle / GC stress 组合。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2003b3

## T21：Structured Concurrency / `Task<T>`

### T2101 [TODO] 并发：`spawn` / `join` 的 typecheck 与 HIR 去 `Int` 硬编码
- 描述：`spawn { ... }` 目前仍要求 body 可赋给 `Int`，`join` lowering 也仍写死为 `Int` 句柄与 `Int` 结果。先把前端表示与类型系统改成真实的 `Task<T>`。
- 目标：
  - `spawn { body }` 从 body 推导结果类型 `T`，表达式类型为 `Task<T>`，不再要求 body 可赋给 `Int`。
  - HIR 对 `spawn` / `join` 保留任务结果类型与必要的运行期元信息，不再把 handle/result 擦成 `Int`。
  - 已确认仍缺失的后端功能由后续任务显式承接，而不是继续用前端 `Int` 特判掩盖：
    - T2102：HIR lowering / sysroot glue 仍写死 `__scoop_task_spawn_int` / `__scoop_task_join_int` 与 `Task<Int>` 表面；
    - T2103：LLVM codegen 仍只支持 `scoop_task_spawn_int` / `scoop_task_join_int`；
    - T2104：runtime executor 仍是 `ScoopTaskU64` / `result_u64` / `resume_u64` 单载荷模型。
- 验收：
  - 新增 typecheck / HIR fixtures：`Task<Int>`、`Task<String>`、`Task<Struct>`、`Task<Task<Int>>`。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2102 [TODO] 并发：`spawn` / `join` 语法糖与 sysroot glue 去 `_int` 专用路径
- 描述：即使 T2101 把前端类型改成 `Task<T>`，当前 lowering 仍固定生成 `__scoop_task_spawn_int` / `__scoop_task_join_int`，`wrap_task_spawn_int_call` 与 sysroot `task/core` 表面也仍把结果类型收窄为 `Task<Int>`。这属于明确缺失的功能，不应被描述成“后端边界”。
- 目标：
  - HIR lowering / block rewrite 不再依赖 `__scoop_task_spawn_int` / `__scoop_task_join_int` 和 `wrap_task_spawn_int_call` 这类 `_int` 专用入口。
  - `sysroot/core.scoop` 与 `sysroot/task.scoop` 中供 `spawn` / `join` / `await` 路径使用的 internal glue 不再只暴露 `Task<Int>` / `Continuation<Int>` / `Executor.await(Task<Int>)`。
  - 语法糖 desugar 后的 HIR 仍能保留任务结果类型，为后续 LLVM / runtime 泛型化提供稳定输入。
- 验收：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - 新增 HIR fixtures：`spawn { "x" }`、`spawn { Struct(...) }`、`join task` 的泛型结果类型保持不被擦除。
- 依赖：T2101

### T2103 [TODO] 并发：LLVM codegen 去 `scoop_task_*_int` 专用路径
- 描述：当前 LLVM 侧对 `spawn/join` 的支持仍硬编码在 `scoop_task_spawn_int` / `scoop_task_join_int`，并显式要求 `CgTy::Int` / `i64` 值。只要这条路径不改，`Task<T>` 就仍然只是假类型，不能执行 `Int` 之外的结果类型。
- 目标：
  - codegen dispatch 不再只识别 `__scoop_task_spawn_int` / `__scoop_task_join_int`，而是支持与 `Task<T>` 对齐的统一 task intrinsic / helper 路径。
  - `spawn` 结果保存、`join` 结果取回可覆盖 scalar / ref / aggregate / 泛型实例，不再把结果压扁回 `i64`。
  - task payload transport 与 continuation payload 方案保持一致，避免维护独立的 task-only ABI。
- 验收：
  - 新增 run-pass fixtures：`spawn` 返回 `String`、tuple/struct、class ref、嵌套泛型值。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2102

### T2104 [TODO] 并发：runtime executor / `Task<T>` 完成回调泛型化
- 描述：`runtime/c/scoop_task_executor.c` 当前仍是 `ScoopTaskU64` + `result_u64` + `on_complete_resume_u64` 模型，sysroot `task.scoop` 也仍把 `taskCreate` / `await` / `map` / `andThen` 固定在 `Task<Int>`。这不是“实现细节”，而是当前明确缺失的运行期功能。
- 目标：
  - runtime task 状态机、executor job、completion waiter 不再只支持 `u64` 结果与 `resume_u64`，而是支持与编译器 ABI 对齐的泛型 payload。
  - `Task<T>.onComplete`、`Executor.await`、`map`、`andThen` 等 glue 不再固定在 `Task<Int>` / `Continuation<Int>`。
  - ref / aggregate payload 在 pinning、GC stress、跨线程或跨 executor 恢复时语义稳定。
- 验收：
  - 新增 `crates/scoop_runtime/tests/*` 或等价 runtime 测试：泛型 task result、onComplete 恢复、ref payload rooting。
  - 至少一组 `SCOOP_GC_STRESS=1` run-pass 覆盖 `Task<String>` 或 `Task<StructWithRef>`。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2103

### T2105 [TODO] 并发：结构化并发回归矩阵与语义锁定
- 描述：`Task<T>` 泛型化后，需要用真实并发场景锁定语义边界，避免回归只覆盖“单任务 + Int 返回值”的最小路径。
- 目标：
  - 覆盖 nested `spawn` / `join`、控制流中的 `join`、多任务交错与 join 顺序等场景。
  - 验证 `Task<T>` 在错误传播、取消前置准备、GC 压力下的最小语义边界；暂不扩展到下一阶段 executor / stdlib API。
  - 把当前阶段明确不支持的并发组合写成稳定诊断或注释化限制，避免语义漂移。
- 验收：
  - 新增 run-pass fixtures：nested spawn/join、多任务交错、控制流 join。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2104

## T22：Lambda 推断与调用语义补齐

### T2201 [TODO] Lambda：expected function type 向任意参数个数传播
- 描述：当前 lambda 的 expected-type 向下传播只覆盖 0/1/2 参数；一旦没有 expected function type，未标注类型的参数会直接报错。需要先把最常见的上下文推断补齐，再把真正不可推断的场景留给稳定诊断。
- 目标：
  - expected function type 的传播覆盖任意参数个数，而不是只支持 0/1/2 参数 lambda。
  - 变量初始化、返回语境、调用实参、集合/构造器上下文等常见入口都能把 expected type 传给 lambda。
  - 对确实无法推断的场景保留清晰、稳定的错误信息，而不是依赖零散 early error。
- 验收：
  - 新增 typecheck fixtures：3+ 参数 lambda、多上下文 expected-type 传播、无法推断时的诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2202 [TODO] Lambda：receiver lambda 体内 `this` 与成员解析
- 描述：receiver function type 已经进入类型系统，但 lambda body 里当前还不会自动注入 `this`，导致“类型可表达、语义不可用”的断层。
- 目标：
  - receiver lambda 进入 typecheck / lowering 时自动建立 `this` 绑定与成员查找环境。
  - receiver lambda 中的成员访问、扩展调用、闭包捕获与普通 lambda 保持一致的局部作用域规则。
  - 相关 HIR / codegen 不再把 receiver 仅当作普通首参处理，避免 `this` 语义在后续阶段丢失。
- 验收：
  - 新增 typecheck / run-pass fixtures：receiver lambda 直接访问 `this`、调用成员、捕获外层局部。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2201

### T2203 [TODO] 调用语义：统一函数值 / funptr / ctor delegation 的实参匹配
- 描述：当前函数值调用、函数指针调用、`super(...)` / `this(...)` 构造器委托调用仍各自带有命名实参或 receiver function type 的早期门禁，调用规则没有真正统一。
- 目标：
  - 函数值调用支持命名实参，参数匹配规则与普通函数调用保持一致。
  - 函数指针调用支持命名实参，并解除对 receiver function type 的不必要早期拒绝，或在更合理的阶段统一降格/诊断。
  - `super(...)` / `this(...)` 构造器委托调用改用同一套实参匹配逻辑，不再只允许位置参数。
- 验收：
  - 新增 typecheck / run-pass fixtures：函数值命名实参、funptr 命名实参、ctor delegation 命名实参。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2201

## T23：泛型约束 / Pattern / 值类型能力补齐

### T2301 [TODO] 泛型：`where` nominal bound 支持类型实参
- 描述：当前 `where` 约束一旦写成带类型实参的 nominal bound（例如 `where T: Box<Int>`）就会被直接拒绝。需要把这类 bound 贯通到解析、解析后表示、检查与诊断。
- 目标：
  - 支持带类型实参的 nominal `where` bound，并正确解析到已实例化的 bound type。
  - 实例化处 bound 检查、函数体内成员分发、错误消息都基于实例化后的 bound，而不是回退到未参数化 nominal type。
  - 对不满足或不可解析的 bound 给出稳定诊断。
- 验收：
  - 新增 typecheck fixtures：正例 `where T: Box<Int>`、反例 `where T: Box<String>`、体内通过 bound 调方法。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2302 [TODO] Pattern：顶层 `val` 支持 pattern binding
- 描述：局部 destructuring 已逐步落地，但顶层 `val (a, b) = ...` 仍在声明头检查阶段被直接拒绝，导致同一套 pattern 语法在顶层与局部语义不一致。
- 目标：
  - 顶层 tuple / struct / enum destructuring 复用既有 pattern binding 规则，而不是单独保留“顶层只允许标识符”的限制。
  - 顶层符号安装、初始化顺序、多文件可见性与循环引用诊断保持稳定。
  - 对当前仍不支持的递归或歧义 pattern 给出明确报错，而不是统一 early reject。
- 验收：
  - 新增多文件 fixtures：顶层 tuple/struct 解构、跨文件引用、非法 pattern 诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2303 [TODO] 值类型：`struct` 字段支持 `var` 与默认值
- 描述：当前 `struct` 字段同时禁止 `var` 与默认值，值类型声明能力明显弱于目标语言语义。需要先收口字段模型，再决定更新与构造路径如何共享实现。
- 目标：
  - `struct` 声明支持 `var` 字段与默认值，声明头、构造参数、布局与初始化规则保持一致。
  - 默认值在构造调用、`with` 更新与常量/编译期路径中有统一语义，不引入额外特判。
  - 字段可变性与值语义冲突处给出明确约束和诊断。
- 验收：
  - 新增 run-pass fixtures：带默认值的 `struct` 构造、省略默认参数、`var` 字段更新。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：无

### T2304 [TODO] 值类型：`with` 更新扩展到更完整的值类型语义
- 描述：当前 `with` 更新只支持 `struct` 且显式拒绝嵌套字段路径更新，无法覆盖更接近 record-style 的值对象写法。
- 目标：
  - `with` 的 base 类型不再局限于当前最小 `struct` 子集，而是对齐本轮支持的值类型模型。
  - 嵌套字段路径更新 lower 成稳定的 copy-update 链，而不是在 typecheck 阶段直接拒绝。
  - 诊断能够区分“字段不存在 / 字段不可更新 / 类型不匹配 / base 非值类型”等不同错误。
- 验收：
  - 新增 HIR / run-pass fixtures：单层 `with`、嵌套路径 `with`、非法更新诊断。
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：T2303

### T2305 [TODO] Pattern：`when` 的 or-pattern 支持共享 binder
- 描述：当前 or-pattern 已能做简单判别，但一旦在 `A(x) | B(x)` 里引入 binder 就会被直接拒绝。需要把 binder 集一致性与类型合流规则补齐。
- 目标：
  - 当各分支 binder 集、名称与类型兼容时，允许 or-pattern 引入共享 binder。
  - `when` arm 的局部环境合并稳定，不再依赖“or-pattern 不能绑定名字”的早期限制。
  - 对 binder 数量、名称或类型不一致的分支给出具体诊断。
- 验收：
  - 新增 typecheck / run-pass fixtures：合法 binder or-pattern、非法 binder mismatch。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T2302

## T24：`const fun` 与 MIR 完整化

### T2401 [TODO] `const fun`：放宽纯签名门禁并后置 effect 验证
- 描述：当前声明头检查会直接拒绝 `const fun` 上的非 `Pure` effect row 与任何 `eff` 参数，使编译期函数模型停留在最保守的纯函数子集。
- 目标：
  - `const fun` 的声明层不再 blanket reject 非 `Pure` effect row / `eff` 参数，而是允许表达后再在语义阶段判定是否可在编译期执行。
  - const evaluator / 调用检查能区分“编译期可执行”“语义可声明但当前未实现”“运行期 effect 不允许进入 const”三类情况。
  - 相关诊断后置到更合理的阶段，并保留清晰的 unsupported reason。
- 验收：
  - 新增 const fixtures：effect row / `eff` 参数的正反例与诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2402 [TODO] MIR：常见表达式 lowering 去 `Todo`
- 描述：MIR 路径里，struct literal、tuple literal、interpolated string、member access、call、cast、type check 等表达式仍大量直接降成 `Todo`，当前更像“结构回归视图”而不是可依赖的中端表示。
- 目标：
  - struct/tuple literal、interpolated string、member access、call、cast、type check 等常见表达式都 lower 成真实 MIR，而不是 `Todo`。
  - 新增 dump-mir fixtures 覆盖每一类表达式，确保结构稳定可回归。
  - 已触达路径不再残留 `Todo("...")` 占位。
- 验收：
  - 新增 MIR fixtures：struct/tuple/interpolation/member/call/cast/type-check。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2403 [TODO] MIR：`perform` / `handle` / 控制流 lowering 去占位
- 描述：effect 相关语义与部分控制流在 MIR 中仍是非常粗糙的占位结构。如果后续要把 MIR 当作验证、优化或解释执行的基础，这一块必须先去掉“看起来有节点，实际不可用”的假完整性。
- 目标：
  - `perform` / `handle` 在 MIR 中有最小但真实的结构化表示，可表达 handler、resume/continue 边界与 effect 控制流。
  - MIR 中的相关控制流节点与前端 / LLVM 语义保持可对照，不再退回统一 `Todo`。
  - 不破坏现有 LLVM 主路径；MIR 的增强先服务于可验证性与后续优化入口。
- 验收：
  - 新增 dump-mir fixtures：`perform`、`handle`、嵌套控制流与 effect 组合。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：T2003、T2402

## T25：低优先级（Annotation / FFI ABI）

### T2501 [TODO] Annotation：annotation class 从 data-only 子集扩到 richer model
- 描述：annotation class 当前只接受“主构造参数承载数据”的最小子集，不支持继承 / 实现接口，也不支持类型体。该能力不阻塞当前主线，因此放在本轮末尾。
- 目标：
  - annotation class 支持更完整的声明模型，包括 supertypes / interfaces 与类型体保留。
  - typecheck / HIR 能保留 richer annotation 元信息，避免在前端直接截断。
  - 对仍未实现的 runtime 或反射相关能力给出明确边界，而不是继续用 data-only 子集冒充完整支持。
- 验收：
  - 新增 parser / typecheck / HIR fixtures：带 supertype、带 body 的 annotation class 及相关诊断。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 依赖：无

### T2502 [TODO] FFI：`@CallingConvention` 与 extern side table 扩展到非 C ABI
- 描述：当前 extern side table 明确只支持 C ABI，`@CallingConvention` 也只接受 `"c"` / `"cdecl"`。这条线属于系统互操作增强，不是当前阶段主线，因此放在所有语言特性任务之后。
- 目标：
  - `@CallingConvention` 接受除 C ABI 之外的目标 calling convention，并在不支持的 target 上给出明确 gate/诊断。
  - extern side table / HIR / LLVM codegen 为符号保存 calling convention 信息，不再写死为 C ABI。
  - 至少用 compile-only / emit-llvm fixtures 锁定 calling convention 的前端与后端映射。
- 验收：
  - 新增 fixtures：非 C ABI extern 声明、目标不支持时的诊断、`--emit-llvm` calling convention 检查。
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
- 依赖：无
