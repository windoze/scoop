# Effect Refactor Boundary Inventory

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 任务来源：`P0-T03`

本文件是 P0-P6 的执行约束，不是讨论草稿。后续任务若要改变“共享 / 复制 / 后续再判定”的结论，必须先更新本文件，再继续实现。

## 判定规则

1. `共享`
同一份实现可以同时服务 `legacy` 与 `refactor`；必须提供单一 API。除最上层 CLI / session / dispatcher 壳层外，模块实现本身不得读取 pipeline selector，也不得带线路特化参数。
2. `复制`
当前模块已经承载 effect / continuation 业务语义，或者其对下游产物的 contract 会在 refactor 主线上独立演化。后续任务必须在 refactor 命名空间创建新实现，不得在原模块里加 `if legacy/refactor` 分支。
3. `后续再判定`
当前还无法安全定性。进入该模块前，必须先把“不确定原因”和最终决定回写到本文件，不能一边实现一边临时判断。

## 当前唯一允许分流的壳层

当前仓库里，只有下列位置允许读取 pipeline selector：

- `crates/scoop/src/cli.rs`
- `crates/scoop/src/commands/**`
- `crates/scoopc/src/driver_cli.rs`
- `crates/scoopc/src/bin/scoopc.rs`
- `crates/scoopc/src/session/`
- `crates/scoopc/src/effect_refactor_pipeline/`

除这些壳层外，其余模块都不得新增 selector 读取点。

## 共享条目

### `crates/scoop/src/cli.rs`、`crates/scoop/src/commands/**`、`crates/scoopc/src/driver_cli.rs`、`crates/scoopc/src/bin/scoopc.rs`

状态：`共享`

单一 API：`EffectPipelineMode -> SessionOptions::new(...) -> Session::with_options(...) -> scoopc::effect_refactor_pipeline::*`

理由：这些文件只负责 CLI 解析、I/O、构造 `SessionOptions`、再调用 `effect_refactor_pipeline` 的 stage wrapper。它们是 P0 明确允许存在的顶层路由壳层，不拥有 AST / HIR / MIR / LLVM 的业务语义。

禁止：

- 在命令模块里直接调用 legacy HIR / MIR / LLVM 业务入口绕过 dispatcher。
- 再引入环境变量、线程局部或测试专用开关去偷偷切主线。
- 在 driver 层新增第二套与 `SessionOptions` 不一致的 pipeline 语义。

### `crates/scoopc/src/session/`

状态：`共享`

单一 API：`SessionOptions`、`Session::with_options(...)`、`Session::effect_pipeline_mode()`

理由：`session` 只负责收口全局编译配置、sysroot 与顶层入口能力；当前 `EffectPipelineMode` 只是一个顶层 config bit，不携带 HIR / MIR / LLVM 业务实现。

禁止：

- 在 `Session` 里加入按阶段拆开的 pipeline 布尔值。
- 让 `session` 直接拥有 typed HIR / MIR / lowering side table。
- 让低层业务代码回头从环境变量读取 pipeline mode，绕过 `SessionOptions`。

### `crates/scoopc/src/effect_refactor_pipeline/`

状态：`共享`

单一 API：`dispatcher_for_session(...)`、`enter_ast_stage(...)`、`enter_typed_hir_stage(...)`、`enter_direct_style_mir_stage(...)`、`enter_effect_facts_stage(...)`、`enter_late_lowering_stage(...)`、`enter_llvm_codegen_stage(...)`

理由：这是 P0 专门建立的 stage-boundary dispatcher 壳层；它允许读取 selector，但职责仅限于“选择 stage entry 并整体委托”，不允许承载具体 lowering / analysis / codegen 逻辑。

禁止：

- 在这里内联实现 typed HIR / MIR / facts / LLVM 业务逻辑。
- 让命令层或其它模块跳过这些 wrapper，直接回 legacy 入口。
- 在 `legacy.rs` / `refactor.rs` 之外扩散新的 selector 读取点。

### `crates/scoopc/src/parser/`

状态：`共享`

单一 API：`crate::parser::parse_file(...)`

理由：P1 的设计基线要求 parser 继续保持普通调用语法和普通 AST 形状；`Continuation.resume(...)` 与单一 `Unit` 参数 sugar 都不应在 parser/AST 阶段做 type-dependent desugar。

禁止：

- 新增 `ResumeExpr` 之类仅服务 refactor 主线的 AST 节点。
- 在 parser 中读取 pipeline selector。
- 在 parser 阶段做 `resume()` 或 `f()` 到 typed contract 的特判 desugar。

### `crates/scoopc/src/source.rs`、`crates/scoopc/src/span.rs`

状态：`共享`

单一 API：`SourceFile`、`SourceMap`、`SourceId`、`Span`

理由：这些文件只承载源文本、位置信息和跨文件映射，是完全中立的底层基础设施；它们不拥有任何 effect / continuation 语义。

禁止：

- 在 `SourceFile` / `SourceMap` / `Span` 中缓存 pipeline 或 lowering 侧专用语义位。
- 给 span/source API 增加 legacy/refactor 专用参数。

### `crates/scoopc/src/sysroot/`

状态：`共享`

单一 API：`Sysroot::load_from(...)`、`Sysroot::default_path()`、`collect_compilable_sysroot_files(...)`

理由：sysroot loader 只负责把标准库签名和需要参与编译的 support sources 加载进来。新旧主线都应读取同一份 sysroot 资产，而不是各自维护隐藏的内建声明树。

禁止：

- 为 refactor 单独引入另一套 sysroot 搜索路径。
- 在 loader 里按 pipeline mode 注入不同签名。
- 让 effect/continuation 语义差异藏进“只在某条主线加载的 sysroot 文件”。

### `crates/scoopc/src/target/`

状态：`共享`

单一 API：`TargetPlatform`

理由：target 能力判断是平台事实，不是 pipeline 事实；P1-P6 的新旧主线都必须服从同一 target capability gate。

禁止：

- 在 target 层引入 effect pipeline 特化 gate。
- 把 lowering 选择塞进 `TargetPlatform`。

### `crates/scoopc/src/ty/`

状态：`共享`

单一 API：`TypeStore`、`TypeId`、`TypeKind`、`ty::layout`

理由：`ty/` 提供的是编译器内部统一类型表示与布局基础设施。即使 P2 会引入 `Continuation<ResumeTuple, Answer, Out>` 的 typed contract，它也应该表现为普通的编译器拥有接口与 side table 信息，而不是让 `TypeStore` 带上 pipeline-specific lowering 逻辑。

禁止：

- 在 `TypeKind` 或 `layout` 中直接编码 legacy/refactor 两套 effect lowering 形状。
- 把 `Step` / continuation object / runtime ABI slot 编号提前塞进通用类型系统层。

### `runtime/c/scoop_runtime_api.h`、`crates/scoop_runtime/src/abi_exports_allowlist.rs`、`crates/scoop_runtime/build.rs`

状态：`共享`

单一 API：runtime 导出符号 allowlist、对应审计测试、以及统一的 runtime 构建脚本

理由：这些文件本身是 declarative ABI 审计与构建胶水，不承载某条 pipeline 的业务语义。无论 legacy 还是 refactor，只要运行时 ABI 对外可见，就都必须经过同一份 allowlist 与同一条构建路径审计。

禁止：

- 绕过 `scoop_runtime_api.h` 新增未登记导出。
- 在 build glue 里按 pipeline 偷偷选择另一套 runtime 库。
- 用条件编译把“仅某条主线可见”的导出藏起来而不回写 allowlist。

### `runtime/c/scoop_root_frame.h`、`runtime/c/scoop_stackmap.h`、`runtime/c/scoop_stackmap.c`

状态：`共享`

单一 API：explicit root frame 枚举 helper 与 stackmap registry / lookup / root-slot visitor

理由：这些 helper 负责的是 GC roots/stackmap 的底层运行时约束。它们应当对“谁调用我”保持中立，只消费显式 frame 描述或 stackmap record，而不拥有 handler-stack 或 continuation 语义判断。

禁止：

- 在 root-frame/stackmap helper 中编码 legacy handler stack、legacy continuation object 布局或 selector 分支。
- 把 refactor 专用 state-machine 语义偷塞进这些底层 ABI helper，而不先通过显式 contract 下沉。

## 复制条目

### `crates/scoopc/src/hir/`

状态：`复制`

从哪个阶段开始分叉：`P1-T03` 建立 AST -> HIR handoff contract 时开始固定新入口；到 `P2-T01` 必须形成 refactor 专属 typed HIR stage。

为什么当前旧实现不能中立共享：当前目录同时承载 `lower_for_dump`、编译单元 lowering、面向现有 MIR/LLVM 的兼容数据形状和 typed 入口。P1/P2 的 refactor 目标要求新的 HIR handoff 与 typed side table 不再借壳 legacy `lower_for_dump`；若继续在原目录内混写，只能靠 selector 分支维持两套 contract。

后续应从哪个入口继续推进：建议在 `crates/scoopc/src/effect_refactor_pipeline/refactor/hir/` 或等价的 refactor-owned 模块下建立新实现，再由 `enter_typed_hir_stage(...)` 接入。

补充约束：若 `hir/` 中有少量 helper 未来证明可以共享，必须先抽到新的中立模块，再同时供两边调用；不能直接在现有 `hir/` 目录中混放双主线实现。

### `crates/scoopc/src/typecheck/`

状态：`复制`

从哪个阶段开始分叉：`P2-T01` / `P2-T02`

为什么当前旧实现不能中立共享：P2 要把 `Continuation<ResumeTuple, Answer, Out>`、`resume()` 的 `Unit` sugar、runtime error 作为普通 `Raise<RuntimeError>` 传播等 typed 语义固定下来。这些都是 refactor 主线要独立冻结的 contract；若直接在现有 `typecheck/` 里加 pipeline 分支，会把 P2 的 typed 语义和 legacy 规则混在一起。

后续应从哪个入口继续推进：建议在 `crates/scoopc/src/effect_refactor_pipeline/refactor/typecheck/` 或等价的 `typed_hir` 子树中落地，再由 refactor typed stage 调用。

补充约束：只有真正不含 pipeline 语义的诊断格式化、纯类型工具函数，才允许后续抽成中立 helper。

### `crates/scoopc/src/mir/`

状态：`复制`

从哪个阶段开始分叉：`P3-T01`

为什么当前旧实现不能中立共享：当前 `mir/` 目录中的 `lower_for_dump`、materialization、summary/pass view 仍围绕 legacy typed/HIR compatibility 组织。P3 需要 production-grade 的 direct-style MIR，并显式下沉 `Call / Perform / Resume / Handle` 语义，禁止继续依赖 HIR fallback、span/name 猜测或旧 dump 入口。

后续应从哪个入口继续推进：建议在 `crates/scoopc/src/effect_refactor_pipeline/refactor/mir/` 建立 direct-style MIR 实现，并由 `enter_direct_style_mir_stage(...)` 统一接入。

补充约束：现有 `mir/` 目录在 P3 之后只允许保留 legacy 主线所需维护，不允许新增 refactor 业务逻辑分支。

### `crates/scoopc/src/effect/`

状态：`复制`

从哪个阶段开始分叉：`P4-T01`（effect facts）与 `P5-T01`（late lowering）

为什么当前旧实现不能中立共享：当前 `effect/` 持有 legacy effect analysis / state-machine planning 的业务语义，而计划基线明确要求 refactor facts 不能混进 legacy `effect` / `summary` / `ProgramFacts`。P4/P5 还要引入新的 `MaterializedEffectFacts`、`resolved_outward_cases`、`ImplPlan` 与整函数 late lowering contract，这些都不能继续寄生在现有 effect 中间层里。

后续应从哪个入口继续推进：建议分别在 `crates/scoopc/src/effect_refactor_pipeline/refactor/effect_facts/` 与 `.../late_lowering/` 下推进，再由对应 stage wrapper 接入。

补充约束：若现有 `effect/state_machine` 中某段逻辑未来证明只是纯 backend-agnostic helper，也必须在 refactor contract 稳定后抽成新中立模块，不能在原目录继续混写新旧实现。

### `crates/scoopc/src/llvm/`

状态：`复制`

从哪个阶段开始分叉：`P6-T01`

为什么当前旧实现不能中立共享：当前 LLVM backend 仍消费 legacy lowered HIR/materialized pass view，并绑定了现有 effect/runtime ABI 假设。P6 的 refactor 目标是“只翻译 P5 late-lowered representation 到 LLVM”，不能在旧 backend 里继续做第二次高层 lowering 或通过 selector 夹带两套 codegen 逻辑。

后续应从哪个入口继续推进：建议在 `crates/scoopc/src/effect_refactor_pipeline/refactor/llvm/` 新建 refactor backend，再由 `enter_llvm_codegen_stage(...)` 接入。

补充约束：任何未来想共享的 backend helper，都必须先证明其 API 不感知 legacy/refactor 的 Step/frame/continuation contract，再单独抽模块；否则默认复制。

### `runtime/c/scoop_runtime.c` 中的 `scoop_effect_*`、`scoop_continuation_*`、`scoop_callee_suspend_state_*`，以及 `runtime/c/scoop_runtime_api.h` 中对应的 effect/continuation 导出

状态：`复制`

从哪个阶段开始分叉：`P6-T04`

为什么当前旧实现不能中立共享：现有 runtime 明确编码了 legacy handler-stack、`EffectOutcome` transport、captured handler stack snapshot、以及 `scoop_continuation_resume_install_legacy_replay_state(...)` 一类 legacy replay 语义。这正是本轮 refactor 要替换的运行时合同；如果继续把 refactor 逻辑追加进同一实现文件，只会重建“同文件双主线”的禁区。

后续应从哪个入口继续推进：在真正进入 `P6-T04` 前，先把 refactor runtime 侧接口拆到新的 `runtime/c/effect_refactor_*.c` / `runtime/c/effect_refactor/` 命名空间，或等价的独立文件树，再把新增导出显式纳入 `scoop_runtime_api.h` allowlist。

补充约束：在完成这一步之前，`runtime/c/scoop_runtime.c` 只允许修 legacy ABI 自身 bug；不允许加入 selector 分支，也不允许先塞“临时兼容层”来模拟 refactor runtime。

## 当前没有“后续再判定”条目

本次清单已对 P0-T03 要求覆盖的主要子系统全部给出结论。若未来进入未列出的关键模块，或某个新模块无法直接归入“共享 / 复制”，必须先在本文件新增条目，再继续实现。

## 搜索守护规则

1. selector 相关命中允许集中在 CLI / session / dispatcher / 测试辅助。
2. 下列共享中立模块必须保持 selector 搜索为 0 命中：
   - `crates/scoopc/src/parser/`
   - `crates/scoopc/src/source.rs`
   - `crates/scoopc/src/span.rs`
   - `crates/scoopc/src/sysroot/`
   - `crates/scoopc/src/target/`
   - `crates/scoopc/src/ty/`
   - `runtime/c/scoop_root_frame.h`
   - `runtime/c/scoop_stackmap.h`
   - `runtime/c/scoop_stackmap.c`
3. 被标记为 `复制` 的目录也不得继续在原地增长新的 selector 分支；refactor 代码应落到新模块，而不是把搜索命中扩散进旧目录。

## 阶段推进对照

1. `P1`：parser、source/span、sysroot、target、ty 继续共享；开始建立 refactor-owned HIR handoff 入口。
2. `P2`：typecheck 与 typed HIR 进入复制路线；`Continuation` contract 与 `Unit` sugar 在 refactor typed 路径固定。
3. `P3`：MIR 进入复制路线；新 direct-style MIR 不再借壳 legacy `lower_for_dump`。
4. `P4-P5`：effect facts 与 late lowering 进入复制路线；禁止把新 facts 混回 legacy `effect`。
5. `P6`：LLVM backend 与 effect/continuation runtime slice 进入复制路线；底层 root-frame/stackmap/ABI 审计继续共享。
