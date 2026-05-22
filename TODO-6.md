# TODO-6：Global init + LLVM backend cleanup + final verification

> 生成时间：2026-05-21
> 细化时间：2026-05-22
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P6-P8
> 索引：[`TODO.md`](./TODO.md)
> 当前状态：`P7-T04-a` 已完成。在 `sysroot_atomic_basic` 触发的 codegen panic 复盘中识别出 codegen 阶段类型纪律不足（`TypeId` 同时承载已/未单态化两种语义、`class_init_layout` 用裸字符串 key 静默兜底），`P7-T04-b` 实施前必须先修复这一根本设计问题。新增前置任务 `P7-T04-b-1..4`（含各自 R）：依次引入 `MonoTypeId`、拆分 `ClassInit` 为 `GenericClassDecl` / `MonoClassInit`、收回 layout key 字符串形态为 `ClassInstanceKey`、把 codegen 全面切到 `MonoTypeId` 并删除 `expect_cg_ty_of` 与所有静默兜底；前一轮未提交的 `P7-T04-b` 实现已 revert。`P7-T04-b` 依赖随之改为 `P7-T04-b-4R`。`P7-T04-b-1` 已完成（`MonoTypeId` / `MonoTypeKind` / `as_mono` / `kind_mono` 纯增量发布）；下一步执行 `P7-T04-b-1R`。

## 范围

- 闭合 global object、top-level `val`、annotated top-level `var` 的初始化根分类。
- 分离 object once 与 top-level eager init 语义。
- 建立 per-cone init routine 与 final entry init order contract。
- 让 `@Global` / `@ThreadLocal` storage policy 全链路闭合。
- 清理 LLVM backend，使其只依赖 `LIR + LIR facts + base context`。
- 做最终全仓验证，为未来 C backend 固定干净输入边界。

## 进入门禁

- P4/P5 已完成：effect facts stage 不修改 MIR 输出本体，`EffectFactsStageOutput = { effect_facts }`，`LirStageOutput = { lir, lir_facts }`，且 LIR opt family 只消费 LIR-owned 输入。
- `scoopc_lir_facts` 已发布 P5-owned backend-neutral callable ABI、dynamic invoke、dispatch owner/slot、continuation/resume publication 与 LIR opt metadata；TODO-6 不应重新让 LLVM 从 HIR/raw MIR/effect facts 推导这些合同。
- TODO-6 从剩余边界开始：global init/storage/entry init order、LLVM HIR scaffold、`LlvmCodegenStageOutput` / `StageEmitInput` P5 wrapper handoff、crate-private MIR pass-view residual、LLVM physical ABI/layout、backend reachability、多 `TypeStore` 桥接和最终全仓验证。

## 细化原则

- 每个实现小阶段后必须插入独立 review 任务。
- P6 global init 与 P7 backend cleanup 分开推进，禁止在 codegen 里补前端语义。
- 如果 LLVM backend 仍需要 HIR/raw MIR/effect facts，必须先补齐 LIR/LIR facts 或 base context owner，不能在 backend 增加新的回看路径。
- 最终验收必须搜索确认：无旧 comptime surface、无 HIR/codegen 层去虚化、无 stage output 嵌套上游整包。

## TODO-6-INIT 审计基线

### Global init / storage 当前入口

| 层级 | 当前入口 | 现状与分歧点 |
| --- | --- | --- |
| HIR side table | `crates/scoopc/src/hir/mod.rs` 的 `ExternGlobal`、`TopLevelVar`、`TopLevelImmutableValue`、`ObjectInit` | 实际 initializer body 仍在 `LoweredHir` side table；P7 前 LLVM 仍读取这些 scaffold，不能把它们当长期 facts。 |
| HIR lowering | `crates/scoopc/src/hir/lower/main/impl_lowering.rs` | top-level `val`、top-level `var`、`@Extern` 与 object init step 在这里分类；`@Global` / `@ThreadLocal` 通过 FQN 解析成 `TopLevelVarStorage`。 |
| HIR/typecheck gate | `crates/scoopc/src/typecheck/annotations.rs` | `@Extern` 顶层变量禁止 initializer；非 extern top-level `var` 必须显式 `@Global` / `@ThreadLocal` 且满足当前 GC-free gate。 |
| `hir_facts` | `crates/scoopc_hir_facts/src/globals.rs`、`crates/scoopc_hir_facts/src/source_sites.rs`、`crates/scoopc/src/pipeline/hir_stage.rs` | 已表达 `TopLevelVal` / `TopLevelVar` / `ObjectSingleton`、storage policy、monomorphic gate、top-level init root contract 与 dependency roots；不承载 initializer body。 |
| MIR / `mir_facts` | `crates/scoopc/src/mir/lower/mir_lowering_facts.rs`、`crates/scoopc/src/mir/lower/entry.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc_mir_facts/src/roots.rs`、`crates/scoopc/src/pipeline/mir_stage.rs` | 已有 `InitializerRoot` / `ExternGlobalRoot` inventory、MIR `TopLevelRef` / `StoreTopLevelVar`；`mir_facts` 目前只发布 dependency count 等摘要，dependency 身份仍主要在 MIR item。 |
| LIR / `lir_facts` | `crates/scoopc_lir_facts/src/lib.rs`、`crates/scoopc/src/pipeline/lir_facts_builder.rs`、`crates/scoopc/src/pipeline/effect_lowering_stage.rs` | `LirFacts` 目前覆盖 callable/effect-control/dispatch/resume/opt metadata，尚未发布 global init/storage/per-cone/final-entry facts；`LirStageContext` 仍保留 LLVM residual pass view。 |
| LLVM globals | `crates/scoopc/src/llvm/codegen/mod.rs`、`crates/scoopc/src/llvm/codegen/main/globals.rs`、`crates/scoopc/src/llvm/codegen/main/immut_value.rs`、`crates/scoopc/src/llvm/codegen/object_init.rs`、`crates/scoopc/src/llvm/codegen/main/cone_init.rs`、`crates/scoopc/src/llvm/emit.rs` | LLVM 仍读取 HIR side tables；top-level `val` 走 lazy once first-access；object once 与 top-level `val` 共享 runtime once；per-cone init routine 当前基本为空壳，只在 final entry 中被调用。 |

### Global init / storage 语义分歧点

1. top-level `val` 仍是 lazy first-access，目标语义要求所有 linked cones 的 top-level `val` 在用户 `main` 前 eager 初始化。
2. object once 与 top-level eager init 仍共享旧 once/runtime helper 路径，目标语义要求 once 只服务 object singleton。
3. per-cone init routine 尚未按 LIR facts 执行本 cone roots；final entry 虽有调用位置，但没有闭合真实 init order。
4. `@Global` / `@ThreadLocal` 的 HIR/MIR facts 已存在，但 LLVM 仍自己决定物理 storage 与初始化方式；非 entry thread TLS 初始化策略还需要显式收口。
5. HIR/MIR 已记录 dependency roots，但 backend 当前不按 dependency contract 决定 init order，且 `mir_facts` 对 dependency 身份的发布不够直接。
6. top-level `var` initializer 当前偏向 LLVM const/static initializer 路径，非 const runtime initializer 与 eager init routine 的关系未闭合。
7. `LirFacts` 尚无 global init/storage contract，导致 LLVM 无法只凭 `LIR + lir_facts + base context` 完成 global init。
8. `@Extern` global root 在 source-site/MIR root 层有合同，但 LLVM 仍保留 HIR `extern_globals` scaffold 读取。

### LLVM backend 当前上游输入残余

| 位置 | 当前读取 | TODO-6 归属 |
| --- | --- | --- |
| `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` | `LlvmCodegenStageOutput` 保存 `LoweredHir` scaffold、`HirFacts`、primary/ABI visibility `EffectLoweredStageOutput`；stage 会从 `CodegenLoweringOutput` 重建 HIR/MIR/P4/P5 handoff。 | P7 删除 P5 wrapper handoff 与 HIR scaffold 长期输出。 |
| `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` | `precheck_invalid_integer_literals` 直接遍历 HIR item/fun/expr/pat 与 `TypeStore`。 | P7 把 backend 前置诊断迁到 HIR 屏障或 LIR/base context，不保留 backend HIR traversal。 |
| `crates/scoopc/src/llvm/emit.rs` | `StageEmitInput` 接收 HIR scaffold 与两份 `EffectLoweredStageOutput`；`LoweredCodegenEntry` 混合 LIR、LIR facts、HIR、pass view、TypeStore。 | P7 收窄 LLVM 入口输入。 |
| `crates/scoopc/src/llvm/emit.rs` | module 构建读取 HIR root/fun index、global side tables、entry main HIR body。 | P6/P7 迁移 global/entry 合同到 LIR facts 与 base context。 |
| `crates/scoopc/src/llvm/reachability.rs` | reachability 同时读取 HIR indexes/body、`MaterializedMirPassView`、raw MIR bodies/calls，并保留 backend devirtualization 推断。 | P7 改为 LIR/LIR facts reachability，删除 codegen 层 devirtualization。 |
| `crates/scoopc/src/llvm/codegen/mod.rs` | `CompilationUnitCodegenCx` 长期持有 HIR indexes、HIR facts、`MaterializedMirPassView`、LIR program、LIR facts、`TypeStore`。 | P7 收口为 backend-private LIR codegen context。 |
| `crates/scoopc/src/llvm/codegen/main/*.rs` | function/expr/stmt/class ctor fallback 直接 lower HIR。 | P7 删除 HIR fallback body emission。 |
| `crates/scoopc/src/llvm/codegen/mir_body/*.rs` | raw MIR body lowering 直接读取 MIR `Body`、`Statement`、`Rvalue`、`CallKind`、MIR `TypeStore`，并用 LIR signature 补丁。 | P7 删除 crate-private MIR pass-view body emission residual。 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/body/*.rs` | 以 LIR state graph 为主，但仍从 pass view 拉 MIR function/body/local/type 信息。 | P7 让 effect-lowered body emission 只依赖 LIR body/source-slice contract 与 base type context。 |
| `crates/scoopc/src/llvm/codegen/effect_lowered/layout/*.rs` | physical ABI layout 主要消费 `LateLoweredProgram + LirFacts + TypeStore`，但 carrier/ABI/type layout 仍读取 HIR vtable/itable/class init/enum layout 与多 `TypeStore` bridge。 | P7 保留 backend-private physical layout，删除 HIR/effect/raw MIR facts 输入与多 TypeStore 模糊桥接。 |

## [DONE] TODO-6-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P6-P8、`PIPELINE_REFACTOR.md` 和当前 global init、storage policy、LLVM backend 输入依赖的真实边界；
  - 生成本任务包的详细任务列表，覆盖 global init model、LLVM backend cleanup 和 final verification；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务扩展 `TODO-6-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出当前 object once、top-level `val`、top-level `var`、`@Global`、`@ThreadLocal` 的 HIR/MIR/LIR/codegen 入口和语义分歧点。
  2. 列出 LLVM backend 当前直接读取 HIR、raw MIR、effect facts 或上游整包 stage output 的位置。
  3. 把 P6、P7、P8 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 global init contract 或 backend 输入边界约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-6.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-6.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 P6/P7/P8 的拆分依据、最终验收命令和未来 C backend 输入边界风险。
- 完成记录：
  - 拆分依据：P6 先闭合 global init/storage/entry order，因为 P7 要删除 LLVM HIR scaffold 必须先有 LIR/LIR facts 承载这些合同；P7 再按入口/global、reachability/body、stage wrapper、physical ABI/TypeStore residual 收口 LLVM；P8 最后做全仓 residual 搜索、文档同步和完整验证。
  - 审计结论：当前 HIR/MIR facts 已有 root 分类、storage policy 与部分 dependency 合同，但 LIR facts 尚无 global init/storage surface；LLVM 仍从 HIR side tables、raw MIR pass view、HIR facts 与多 TypeStore bridge 补语义，top-level `val` 仍 lazy first-access，per-cone init routine 仍未执行真实 eager init。
  - 最终验收命令基线：`cargo fmt`；`cargo run -p scoop_tools -- dependency-gate`；`cargo test --all --all-targets`；`cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`；并配套 residual 搜索 `rg -n "comptime|const_eval|devirtual|hir_compat_scaffold|llvm_residual_pass_view|materialized_pass_view|EffectLoweredStageOutput|EffectFactsStageOutput.*MirStageOutput|LirStageOutput.*MirStageOutput"`。
  - 未来 C backend 风险：如果 P6/P7 后 LLVM 仍需要读取 HIR/raw MIR/effect facts 或 backend-specific TypeStore bridge，未来 C backend 会复制同一耦合；因此 P7/P8 完成条件必须冻结为共享 `LIR + LIR facts + base context` 输入边界，而不是只让 LLVM 局部通过。

## [DONE] P6-T01：发布 global init 与 storage LIR facts contract

- 目标：
  - 在 `scoopc_lir_facts` 中发布 backend-neutral global init/storage contract；
  - 让 per-cone init routine、final entry init order、object once、top-level eager init 和 storage policy 有 LIR-owned 查询面。
- 必须修改的主要位置：
  - `crates/scoopc_lir_facts/`
  - `crates/scoopc/src/pipeline/lir_facts_builder.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/effect_lowered/`
  - 相关 dump/verifier/tests/fixtures
- 必须实现的内容：
  1. 新增 LIR facts 数据结构表达 global root identity、root kind、storage policy、initializer presence、dependency root identity、object once contract、top-level eager init contract、per-cone init routine contract 与 final entry init order contract。
  2. 从 P3/P5 显式输入构造这些 facts；禁止通过 P4 output 或 LLVM backend 回看 HIR/raw MIR 来补合同。
  3. `LirFacts` verifier 必须检查 root identity 唯一、dependency 指向已发布 root、object once 与 top-level eager init 分类互斥、`@Global` / `@ThreadLocal` storage policy 完整。
  4. stable dump 中展示 global init/storage facts，便于后续 P6/P7 fixture 回归。
  5. 如果发现 MIR facts 只发布 dependency count 不足以构造 LIR contract，必须在本任务内补齐正确 owner，而不是让 LLVM 使用旧 dependency 旁路。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_lir_facts`
  3. `cargo test -p scoopc --no-default-features lir_facts_builder`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  5. `git diff --check`
- 完成条件：
  - `LirFacts` 明确发布 global init/storage/final-entry 合同；
  - 后续 LLVM 可从 `LIR + lir_facts + base context` 查询 global init 所需的 backend-neutral 信息；
  - 没有新增 HIR/raw MIR/effect facts 的 backend 公共查询面。
- 依赖：TODO-6-INIT
- 完成记录：
  - 新增 MIR-owned initializer dependency facts，避免 LIR/LLVM 只能从 dependency count 或 raw MIR item 回推 dependency identity。
  - `scoopc_lir_facts` 新增 global init/storage fact group，发布 global root、root kind、storage policy、initializer presence、dependency identity、object once、top-level eager init、per-cone init routine 与 final entry init order contract。
  - LIR facts verifier 覆盖 global root count、dependency target、object once/top-level eager init 互斥、mutable/extern storage policy、cone routine 与 final entry routine 引用完整性。
  - `lir_facts_builder` 从 `MirFacts` 构造 global init/storage facts；stable dump 在存在 global roots 时展示 global init/storage section，后续 P6/P7 可从 `LIR + lir_facts + base context` 查询这些 backend-neutral 信息。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features lir_facts_builder`；`cargo test -p scoopc_mir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`git diff --check`；`cargo clippy --all-targets -- -D warnings`。

## [DONE] P6-T01R：Review global init/storage LIR facts contract

- 参考：P6-T01。
- 重点：
  - `LirFacts` 是否真正表达 P6 所需的 global init/storage 合同；
  - facts owner 是否正确，是否引入新的上游整包嵌套或 backend 回看；
  - verifier/dump/test 是否能暴露 dependency、root kind 和 storage policy drift。
- 验证：
  - 重新运行 P6-T01 的所有验证；
  - 额外搜索 `LirFacts` / `scoopc_lir_facts` 是否引用 stage crate、`MaterializedMir`、`EffectFactsStageOutput` 或 LLVM 类型。
- 完成条件：
  - review 结论明确写出：global init/storage LIR facts contract 可以支撑 P6-T02/P6-T03，或列出阻塞项并在本 review 内修复。
- 依赖：P6-T01
- 完成记录：
  - Review 结论：`LirFacts` 已发布 global root、root kind、storage policy、initializer presence、dependency identity、object once、top-level eager init、per-cone init routine 与 final entry init order 合同，可以作为 P6-T02/P6-T03 的输入边界继续推进。
  - 本 review 修复了 verifier 漏洞：现在会拒绝 top-level eager/object-once contract 与 root 的 initializer/storage drift、extern global contract 缺失或 initializer_absent drift、eager root 未进入或重复进入 cone init routine、routine/root cone 不一致、final entry routine 遗漏/重复，以及 eager dependency init order drift。
  - owner/依赖复审：`scoopc_lir_facts` 仍只依赖 `scoopc_ids`、`scoopc_project_model`、`scoopc_types`，额外搜索未发现 stage crate、`MaterializedMir`、`EffectFactsStageOutput` 或 LLVM 类型引用；global init/storage facts 从 MIR facts/LIR stage 显式输入构造，没有新增 backend 回看查询面。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features lir_facts_builder`；`cargo test -p scoopc_mir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；额外 residual 搜索 `MaterializedMir|EffectFactsStageOutput|inkwell|llvm|crate::mir|crate::effect|crate::hir|scoopc::|scoopc_mir|scoopc_effect|scoopc_hir` 于 `crates/scoopc_lir_facts` 无命中；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [DONE] P6-T02：实现 per-cone eager top-level init 与 final entry order

- 目标：
  - 让 top-level `val` 和 annotated top-level `var` 在用户 `main` body 前按 source-cone DAG / per-cone root contract eager 初始化；
  - 删除 top-level `val` lazy first-access 语义。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/codegen/main/cone_init.rs`
  - `crates/scoopc/src/llvm/codegen/main/immut_value.rs`
  - `crates/scoopc/src/llvm/codegen/main/globals.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/effect_lowered/` 与 LIR global init lowering 相关位置
  - global init 相关 fixtures
- 必须实现的内容：
  1. per-cone init routine 必须根据 P6-T01 的 LIR facts 初始化本 cone roots，不递归替依赖 cone 干活。
  2. final system entry 必须在 runtime init 后、用户 `main` body 前按 linked source-cone DAG 顺序调用 cone init routines。
  3. top-level `val` 访问路径改为读取 eager 初始化后的 backing storage，不再触发 lazy init function / once guard。
  4. annotated top-level `var` initializer 进入 cone init routine；静态数据 initializer helper 只能作为 backend 物理化优化，不能改变 eager init 语义。
  5. 初始化 dependency contract 必须由 facts/IR 决定，不能依赖源码文件编译顺序或 LLVM 遍历偶然顺序。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features global_init`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `git diff --check`
- 完成条件：
  - top-level `val` / annotated top-level `var` 不再 lazy first-access；
  - 所有 linked cones 的 top-level eager init 在 `main` 前完成；
  - cone init routine 真实执行本 cone roots，且 entry order 有测试覆盖。
- 依赖：P6-T01R
- 完成记录：
  - LLVM cone init plan 改为消费 `LirFacts.global_init.final_entry_order` / `cone_init_routines`，不再从 HIR source map 或 side table 临时推导 routine/root 顺序；LIR facts builder 同步按 eager root dependency 为 final entry routines 做跨 cone 拓扑排序。
  - per-cone init routine 现在执行本 routine 发布的 top-level eager roots：top-level `val` 通过 eager init helper 在 main 前初始化，annotated top-level `var` 在 routine 内求值并写入 backing storage；top-level var storage 默认零初始化，initializer 不再要求 LLVM static const 路径。
  - top-level `val` 普通访问和 hidden bridge 不再 first-access 调用 init helper，只保留 initialized check 并读取 eager backing storage；递归初始化错误仍由 guard check 稳定失败。
  - MIR root facts 为 initializer/extern global root 使用 source path 对应的 source-cone stable key，避免 LIR global init facts 把所有 roots 归到单一 compilation cone。
  - 新增 LLVM IR 单测覆盖 cone routine 执行 eager roots 和普通 top-level `val` 访问不再 lazy-init；新增 `tests/fixtures/run-pass/global_init/eager_val_and_global_var_before_main.scoop`，并按 eager 语义更新既有 top-level `val` stdout golden。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features global_init`；`cargo test -p scoopc global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoopc pipeline_user_visible_failure_policy_tracks_internal_bug_sentinels`；`git diff --check`。
  - 已运行完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：P6-T02 相关 fixtures 均通过，但全量 run-pass 仍有 7 个既有失败，分别来自 unresolved generic `scoop.core.println`、`scoop.runtime.test.*` import 或非本任务路径；未将这些历史问题作为 P6-T02 前置项。

## [DONE] P6-T02R：Review per-cone eager top-level init 与 final entry order

- 参考：P6-T02。
- 重点：
  - top-level eager init 是否覆盖 `val`、`@Global var`、entry-thread `@ThreadLocal var`；
  - final entry init order 是否只由 source-cone DAG 与 LIR facts 决定；
  - 是否删除或隔离了 top-level `val` lazy once path。
- 验证：
  - 重新运行 P6-T02 的所有验证；
  - 额外搜索 `immut_value` / global init code 中是否仍存在 top-level `val` first-access once 初始化路径。
- 完成条件：
  - review 结论明确写出 eager top-level init 与 final entry order 已闭合，或列出阻塞项并在本 review 内修复。
- 依赖：P6-T02
- 完成记录：
  - Review 结论：eager top-level init 与 final entry order 已闭合到 P6-T02R 要求；top-level `val` 普通访问不再 first-access lazy init，`@Global var` 与 entry-thread `@ThreadLocal var` initializer 均由 per-cone eager init routine 在用户 `main` 前执行。
  - 本 review 修复了两个阻塞缺口：MIR/LIR global init facts 现在携带 dependency-before-consumer source-cone topo order，LIR verifier 会拒绝 final entry routine source-cone 顺序倒置；HIR top-level initializer dependency 收集会递归扫描直接调用的顶层/成员函数体，避免经 helper 间接读取 top-level root 时漏排 eager init dependency。
  - 新增回归覆盖：`eager_threadlocal_var_before_main.scoop` 验证 entry-thread TLS initializer 在 `main` 前运行；`indirect_eager_dependency_via_function.scoop` 验证经函数间接读取的 eager dependency 会先初始化；`scoopc_lir_facts` verifier 单测覆盖 final-entry source order drift。
  - 残余搜索结论：`immut_value` / main global init 代码中，`ensure_top_level_immutable_value_init_function_defined` 只由 `cone_init.rs` 的 eager routine 调用；普通 top-level `val` 访问路径未再调用 lazy init helper。`scoop_once_begin/end` 仍存在于 top-level `val` init helper body 与 object once 路径，P6-T03 将继续分离 object once 与 storage policy。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc_mir_facts`；`cargo test -p scoopc --no-default-features global_init`；`cargo test -p scoopc --no-default-features hir_top_level_init_publishes_storage_and_extern_roots`；`cargo test -p scoopc --no-default-features lir_facts_builder`；`cargo test -p scoopc global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 已重跑 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`：新增/相关 global-init、top-level-val、top-level-var fixtures 均通过；全 run-pass 仍有 7 个既有非本任务失败（array literal inference、mutable array ref/composite、std process args、stdlib string、sysroot atomic 路径），未作为 P6-T02R 前置项。

## [DONE] P6-T03：分离 object once 与 `@Global` / `@ThreadLocal` storage policy

- 目标：
  - object singleton 保留独立 once 语义；
  - top-level eager init 不再共享 object once runtime path；
  - `@Global` / `@ThreadLocal` storage policy 在 HIR/MIR/LIR/codegen/runtime 语义上闭合。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/codegen/object_init.rs`
  - `crates/scoopc/src/llvm/codegen/main/globals.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body/member.rs`
  - `runtime/c/scoop_once.c`
  - HIR/typecheck storage policy gate 与相关 fixtures
- 必须实现的内容：
  1. object once guard/runtime helper 只服务 object singleton，不再被 top-level `val` eager init 复用。
  2. object 初始化中的递归/半初始化 contract 与普通 self access 保持独立测试覆盖。
  3. `@Global var` 使用 global storage，并由 cone init routine 初始化。
  4. entry thread 的 `@ThreadLocal var` 使用 TLS storage，并由 cone init routine 初始化。
  5. 非 entry thread TLS 初始化策略必须显式实现；若本轮语义选择限制该能力，必须在 HIR/typecheck 屏障稳定拒绝并有 fixture，不能留给 LLVM/runtime 猜测。
  6. `@Extern` global / thread-local root 必须继续遵守 storage policy、unsafe 和 no-initializer 合同。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features storage_policy`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`
  5. `cargo test -p scoop_runtime --all-targets`
  6. `git diff --check`
- 完成条件：
  - object once 与 top-level eager init 不共享语义路径；
  - `@Global` / `@ThreadLocal` storage policy 从 frontend facts 到 codegen/runtime 有一致实现；
  - unsupported TLS 场景不再以后端 panic 或未定义行为暴露。
- 依赖：P6-T02R
- 完成记录：
  - top-level `val` eager init helper 不再调用 runtime `scoop_once_begin/end`，改用编译器私有 guard state machine；普通访问仍要求 guard 已 initialized，递归 eager init 继续稳定以退出码 1 失败。
  - object singleton init 继续独占 runtime once 路径；更新 runtime/codegen 注释并新增 LLVM 单测确认 top-level eager init 不含 `scoop_once`，object singleton init 仍保留 `scoop_once`。
  - `@Global var` 继续使用普通 LLVM global 并由 per-cone init routine 初始化；`@ThreadLocal var` 继续使用 LLVM TLS storage，entry thread 由 final entry cone init 初始化，worker thread 通过新生成的 `scoop_thread_init_current` hook 在 `scoop.thread` native trampoline 进入用户 closure 前初始化本线程 TLS roots。
  - HIR/typecheck storage policy gate 新增 `@ThreadLocal` 与 `@Global` 互斥诊断，避免 frontend facts/codegen 对冲突策略静默择一；`@Extern` global/TLS 无 initializer 与 storage policy 合同保持不变。
  - 新增 fixtures 覆盖 worker thread TLS 初始化、object once 普通访问、object recursive half-init contract，以及 storage policy 冲突诊断。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features storage_policy`；`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo test -p scoop_runtime --all-targets`；`cargo test -p scoopc llvm_top_level_eager_init_does_not_call_object_once_runtime`；`cargo test -p scoopc llvm_object_singleton_init_keeps_object_once_runtime`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 备注：首次 `cargo test -p scoop_runtime --all-targets` 使用 20 分钟工具 timeout 在 `stackmap_registry` 附近被中止；随后单独运行 `cargo test -p scoop_runtime --test stackmap_registry -- --nocapture` 和更长 timeout 的完整 runtime suite 均通过，未发现单个 runtime test 卡住。

## [DONE] P6-T03R：Review object once 与 storage policy 分离结果

- 参考：P6-T03。
- 重点：
  - object once 是否只服务 object；
  - top-level eager init 是否不再复用 once first-access；
  - `@Global` / `@ThreadLocal` / `@Extern` storage policy 是否在 HIR/MIR/LIR/codegen/runtime 一致。
- 验证：
  - 重新运行 P6-T03 的所有验证；
  - 额外搜索 `scoop_once` 调用点，确认 top-level eager init 没有继续走 object once helper。
- 完成条件：
  - review 结论明确写出 object once 与 storage policy 已闭合，或列出阻塞项并在本 review 内修复。
- 依赖：P6-T03
- 完成记录：
  - Review 结论：object once 与 storage policy 分离结果已闭合到 P6-T03R 要求；object singleton 继续独占 runtime `scoop_once_begin/end` 路径，top-level eager init 不再复用 once first-access，`@Global` / `@ThreadLocal` / `@Extern` storage policy 在 HIR/MIR/LIR/codegen/runtime 路径保持一致。
  - 复审结果：`declare_runtime_once_begin/end` 的实际 codegen 调用点只在 `object_init.rs`，top-level `val` eager init 通过 `immut_value.rs` 的 compiler-private guard state machine 和 `cone_init.rs` 的 eager routine 执行；普通 top-level `val` 访问只做 initialized check 后读取 backing storage。
  - Storage policy 复审：非 extern 顶层 `var` 仍由 typecheck 要求显式 `@Global` / `@ThreadLocal` 且拒绝二者冲突；HIR/MIR/LIR facts 保留 resolved storage policy；LLVM 对 `@Global` 使用普通 global，对 `@ThreadLocal` 使用 TLS storage；entry thread 经 final entry cone init 初始化 TLS roots，worker thread 经 `scoop_thread_init_current` hook 初始化当前线程 TLS roots；`@Extern` global/TLS 继续保持 no-initializer、unsafe 与 storage contract。
  - 额外 residual 搜索：`scoop_once_begin|scoop_once_end` 在 `crates/scoopc/src/llvm/codegen` 只剩 runtime symbol/declaration 注释、常量与 object guard 注释；`top_level_val_eager_init|top_level_val_init_guard|declare_runtime_once_begin` 搜索确认 top-level eager path 只调用 compiler-private init helper，runtime once declaration 只被 object init 使用。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features storage_policy`；`cargo test -p scoopc llvm_top_level_eager_init_does_not_call_object_once_runtime`；`cargo test -p scoopc llvm_object_singleton_init_keeps_object_once_runtime`；`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo test -p scoop_runtime --all-targets`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [DONE] P6-T04：P6 全包清场、文档同步与依赖审计

- 目标：
  - 对 P6 global init/storage/entry order 做最终清场；
  - 同步文档、README、fixtures 和 TODO 状态；
  - 为 P7 删除 LLVM HIR/raw MIR residual 提供稳定输入边界。
- 必须修改的主要位置：
  - `TODO.md`
  - `TODO-6.md`
  - `PIPELINE-CLEANUP.md`
  - `PIPELINE_REFACTOR.md`（仅在设计边界实际变化时）
  - `README.md`
  - global init/storage fixtures
- 必须实现的内容：
  1. 搜索确认 top-level `val` 不再 lazy first-access，per-cone init routine 不再为空壳。
  2. 搜索确认 object once 与 top-level eager init 语义路径已分离。
  3. 搜索确认 LIR facts 已发布 global init/storage/final-entry contract，LLVM 不再为 P6 语义创建新的 HIR fallback。
  4. 更新文档，明确 P7 仍需清理的 backend residual 只剩 HIR scaffold、raw MIR pass view、reachability、physical ABI/layout 与 TypeStore bridge。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_lir_facts`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `cargo clippy --all-targets -- -D warnings`
  6. `git diff --check`
- 完成条件：
  - P6 global init/storage/entry order 语义闭合；
  - `TODO.md` 与本文件状态同步；
  - P7 可以从明确的 `LIR + lir_facts + base context` global-init 输入开始清理 LLVM。
- 依赖：P6-T03R
- 完成记录：
  - 清场结论：P6 global init/storage/entry order 语义已闭合。top-level `val` 普通访问只做 initialized check 后读取 eager backing storage；`ensure_top_level_immutable_value_init_function_defined` 只由 `cone_init.rs` 的 per-cone eager routine 调用；per-cone routine 从 `LirFacts.global_init.final_entry_order` / `cone_init_routines` 构造并执行本 cone eager roots，不再是空壳。
  - object once 分离结论：`declare_runtime_once_begin/end` 的实际 codegen 调用点只剩 `object_init.rs`，top-level eager init 使用 compiler-private guard state machine，不再复用 runtime object once helper。
  - LIR facts 合同结论：`scoopc_lir_facts` 已发布 global root、storage policy、object once、top-level eager init、per-cone routine 与 final entry order contracts；`lir_facts_builder` 从 MIR facts 构造这些合同，verifier 覆盖 root/dependency/storage/routine/order drift，dependency gate 确认 fact crate 仍只依赖基础 crate。
  - 本任务修复了 P6 清场验证暴露的真实阻塞：`scoop.thread` native object 会引用 `scoop_thread_init_current`，而没有 TLS roots 的程序此前不会定义该 hook，导致线程相关 fixtures 链接失败。现在 LLVM 始终定义 `scoop_thread_init_current`，无 TLS roots 时生成 no-op body，有 TLS roots 时仍执行当前线程 TLS roots；`std_thread_basic`、delegated-property 并发 fixtures 与 GC cross-thread continuation fixture 因此恢复通过。
  - 文档同步：`README.md` 与 `PIPELINE-CLEANUP.md` 已更新为 P6 完成状态，明确 P7 剩余 residual 只应覆盖 HIR scaffold、raw MIR/pass-view body emission、reachability、physical ABI/layout、entry/global HIR side-table 过渡读取与多 `TypeStore` bridge。`PIPELINE_REFACTOR.md` 未修改，因为设计边界没有变化。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo test -p scoopc --no-default-features storage_policy`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 run-pass：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑，P6/global-init/thread-init 相关 fixtures 全部通过；全量仍有 7 个既有非 P6 失败（`array_lit_infer_unannotated_and_nested_basic.scoop`、`array_lit_infer_string_char_float_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`），与 P6-T02R / P5 记录的历史 baseline 一致，未作为 P6-T04 前置项。

## [DONE] P6-T04R：Review P6 全包完成度

- 参考：P6-T04。
- 重点：
  - P6 是否满足 `PLAN.md` 中 object once、top-level eager init、per-cone init routine、final entry order 与 storage policy 完成标准；
  - P7 residual 是否没有被错误描述为 P6 已完成范围。
- 验证：
  - 重新运行 P6-T04 的所有验证；
  - 额外搜索 global init/storage 相关 HIR scaffold 在 LLVM 中的剩余读取点，并确认它们已明确留给 P7。
- 完成条件：
  - review 结论明确写出 P6 完成、P7 可以开始，或列出阻塞项并在本 review 内修复。
- 依赖：P6-T04
- 完成记录：
  - Review 结论：P6 global init model 已满足 `PLAN.md` 中 object once、top-level eager init、per-cone init routine、final entry order 与 storage policy 完成标准；P7 可以从已明确的 `LIR + lir_facts + base context` global-init 输入边界开始清理 LLVM backend residual。
  - P6 完成度复审：`cone_init.rs` 按 `LirFacts.global_init.final_entry_order` / `cone_init_routines` 构造并执行 eager roots；top-level `val` 普通访问只做 initialized guard 检查并读取 backing storage；`@Global` / `@ThreadLocal` top-level `var` 由 cone/thread init routine 初始化；`scoop_thread_init_current` 在无 TLS roots 时也会生成 no-op hook，避免线程 runtime 链接漂移。
  - object once 复审：`scoop_once_begin/end` 的实际 codegen 调用点只在 `object_init.rs`，top-level eager init 继续使用 compiler-private guard state machine，没有复用 object once runtime helper。
  - P7 residual 复审：LLVM 中 `top_level_vars`、`top_level_immutable_values`、`object_inits`、`extern_globals` 等 HIR scaffold 读取仍存在于 `emit.rs`、`codegen/mod.rs`、`codegen/main/*`、`reachability.rs`、`mir_body/*` 与 `effect_lowered/*` 等路径；这些均已在 `P7-T01` / `P7-T02` / `P7-T03` / `P7-T04` 作为 backend cleanup residual 登记，未被误描述为 P6 已完成范围。
  - 额外 residual 搜索：`ensure_top_level_immutable_value_init_function_defined` 只由 `cone_init.rs` 的 eager root emission 调用；`scoop_once_begin|scoop_once_end|declare_runtime_once_begin|declare_runtime_once_end` 在 LLVM codegen 中除 runtime symbol/declaration/comment 外，只剩 `object_init.rs` 的 object singleton init 调用。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 run-pass：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑，global-init/thread-init 相关 fixtures 全部通过；全量仍有 7 个既有非 P6 失败（`array_lit_infer_string_char_float_basic.scoop`、`array_lit_infer_unannotated_and_nested_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`），与 P6-T04 baseline 一致，未作为 P6-T04R 前置项。

## [DONE] P7-T01：迁移 LLVM entry/global 查询到 LIR facts

- 目标：
  - 删除 LLVM entry/global 初始化路径对 HIR side tables 的语义依赖；
  - 让 entry selection、global root inventory、extern global contract 和 cone init 调用只读取 LIR/LIR facts/base context。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/main/cone_init.rs`
  - `crates/scoopc/src/llvm/codegen/main/globals.rs`
  - `crates/scoopc/src/llvm/codegen/main/immut_value.rs`
  - `crates/scoopc/src/llvm/codegen/object_init.rs`
  - `crates/scoopc_lir_facts/`
- 必须实现的内容：
  1. `select_entry_main` / entry argument classification 不再遍历 `LoweredHir.file.items`，改用 LIR callable facts 或明确 base context。
  2. extern global、top-level var、top-level val、object singleton 的 LLVM declaration/definition inventory 改读 LIR global init/storage facts。
  3. 移除或降级 `CompilationUnitCodegenCx` 中仅为 entry/global 查询保留的 HIR side table 字段。
  4. 如果发现 LIR facts 缺少 entry/global 所需信息，必须补 facts owner，而不是保留 HIR scaffold fallback。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm_entry_global`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `git diff --check`
- 完成条件：
  - LLVM entry/global 语义查询不再依赖 HIR side tables；
  - global init/storage 的 backend 物理化只消费 LIR facts 与 base/type context；
  - 没有新增 backend-private HIR fallback。
- 依赖：P6-T04R
- 完成记录：
  - `scoopc_lir_facts` 补齐 LLVM entry/global 查询需要的 backend-neutral owner：`LirCallableFacts` 现在发布 source-level callable kind、source 参数类型和返回类型，`LirExternGlobalFacts` 发布 extern linkage；verifier 会拒绝 callable source signature 与 ABI contract 漂移。
  - LLVM entry selection / `main(args: Array<String>)` 参数分类改为读取 LIR callable facts + TypeStore，不再遍历 `LoweredHir.file.items` 来决定入口语义；HIR `FunDecl` 只作为当前 P7-T03 前仍存在的 body emission scaffold 按已选 FQN 取回。
  - extern global、top-level `var`、top-level `val`、object singleton 的 LLVM declaration/definition inventory 查询改为读取 `LirFacts.global_init`；`CompilationUnitCodegenCx` 已移除 HIR `extern_globals` side table 输入，top-level/object HIR side tables 仅保留 initializer/body emission scaffold residual。
  - 新增 LLVM entry/global 回归测试覆盖 LIR callable facts 驱动的 argv entry classification，以及 extern global symbol declaration/load-store 由 LIR global facts physicalize。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features llvm_entry_global`（无匹配 LLVM 测试但编译通过）；`cargo test -p scoopc llvm_entry_global`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑：entry/global、extern global、global-init 相关 fixtures 均通过；全量仍有 7 个既有非 P7-T01 失败（`array_lit_infer_unannotated_and_nested_basic.scoop`、`array_lit_infer_string_char_float_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`），与 P6-T04R baseline 一致，未作为本任务前置项。

## [DONE] P7-T01R：Review LLVM entry/global LIR facts 迁移结果

- 参考：P7-T01。
- 重点：
  - LLVM entry/global 查询是否已切到 LIR facts；
  - P6 global init contract 是否未被 LLVM 重新推导；
  - HIR side table residual 是否只剩后续 body/reachability/physical layout 范围。
- 验证：
  - 重新运行 P7-T01 的所有验证；
  - 额外搜索 `emit.rs` / `codegen/main` 中对 `top_level_vars`、`top_level_immutable_values`、`object_inits`、`extern_globals` 的 HIR scaffold 读取。
- 完成条件：
  - review 结论明确写出 entry/global backend 输入边界已迁移，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T01
- 完成记录：
  - Review 结论：LLVM entry/global backend 输入边界已迁移到 `LIR + lir_facts + base/type context`；entry main selection / `main(args: Array<String>)` 参数分类从 `LirCallableFacts` 与 `TypeStore` 查询，不再遍历 `LoweredHir.file.items` 做入口语义判定。
  - global inventory 复审：per-cone init plan 由 `LirFacts.global_init.final_entry_order` / `cone_init_routines` / `roots` 驱动；top-level `var`、top-level `val`、object singleton 与 `@Extern` global 的 LLVM storage/key/declaration 入口均先校验或读取 `LirGlobalRootFacts`，没有新增 backend-private HIR fallback。
  - residual 搜索结论：`emit.rs` 中的 `top_level_vars` / `top_level_immutable_values` / `object_inits` 传入仍用于 body/initializer emission scaffold，`extern_globals` 只剩 `reachability` 输入；`codegen/main` 中未发现 `extern_globals` 读取，剩余 `top_level_vars` / `top_level_immutable_values` 读取均在 LIR facts 选中 root 后取 initializer/body scaffold，归属 P7-T02/P7-T03 后续清理范围。
  - LIR facts 复审：`scoopc_lir_facts` verifier 覆盖 callable source signature 与 ABI contract 漂移、global root/storage/extern contract drift、eager root/routine/final-entry order drift；生产 `lir_facts_builder` 构造后执行 `facts.verify()`，未发现 P7-T01R 阻塞项。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features llvm_entry_global`（无匹配测试但编译通过）；`cargo test -p scoopc llvm_entry_global`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 run-pass：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已重跑，entry/global、extern global、global-init 相关 fixtures 通过；全量仍有 7 个既有非 P7-T01R 失败（`array_lit_infer_unannotated_and_nested_basic.scoop`、`array_lit_infer_string_char_float_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`），与 P7-T01 / P6 baseline 一致，未作为本 review 前置项。

## [DONE] P7-T02：删除 backend reachability HIR/MIR 回看与 codegen 去虚化 residual

- 目标：
  - 让 LLVM reachability 只基于 LIR/LIR facts 的 callable、dispatch、dynamic invoke 与 global init root；
  - 删除 reachability 层临时去虚化推断和 HIR/raw MIR 扫描。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/reachability.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/lookup.rs`
  - `crates/scoopc_lir_facts/`
  - reachability 相关 tests/fixtures
- 必须实现的内容：
  1. reachability seed 来自 final entry、global init routines、published LIR callables 和 runtime-required callables。
  2. ordinary/dynamic/dispatch edges 来自 LIR facts，不从 HIR body、raw MIR body或 pass-view candidate body 推断。
  3. 删除 backend reachability 中的 devirtualization fallback；去虚化 owner 仍只能是 MIR 或 LIR 窄 state-machine opt。
  4. reachability dump/test 能覆盖 object/global init root、dynamic invoke、dispatch slot 与 continuation/resume callable。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm::reachability`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `git diff --check`
- 完成条件：
  - `llvm/reachability.rs` 不再读取 HIR body、raw MIR body或 `MaterializedMirPassView`；
  - codegen 层不再做普通 dispatch devirtualization；
  - LIR/LIR facts reachability 足以覆盖 LLVM emission。
- 依赖：P7-T01R
- 完成记录：
  - `llvm/reachability.rs` 已替换为只消费 `LirFacts` 的 collector：入口 root、global init roots、已发布 LIR callables、runtime-required callables 作为 seeds；ordinary call、dynamic invoke、dispatch candidate edges 均从 `LirCallableFacts` / `LirDynamicInvokeContract` / `LirDispatchContract` 读取，不再导入或扫描 HIR body、raw MIR body、`MaterializedMirPassView`。
  - reachability 对 `KnownInstance` target 保持 required 语义，缺失时继续暴露为 backend handoff 错误；对 conservative `CandidateSet` / dispatch candidate 只 enqueue 已发布到 `LirFacts.callables` 的 callable，避免把未发布的宽候选（例如无关 `ToString` implementer）重新作为 backend HIR fallback 目标。
  - `emit.rs` 的 reachable set 改为从 LIR facts reachable FQN 映射到当前 P7-T03 前仍存在的 legacy body/declaration scaffold；删除了原先基于 HIR `fun_index` 的 monomorphized helper 追加逻辑。
  - 删除 `llvm/codegen/call/lowering.rs` 中接口调用的 codegen 层 dispatch 去虚化 fallback；普通 dispatch target refinement 只保留在 MIR/LIR-owned pass/facts 路径。
  - 新增 `llvm::reachability` 单测覆盖 LIR callable edges、global init root seed、dynamic invoke、dispatch target、unpublished conservative candidate 忽略，以及 published continuation/resume callable seed。
  - residual 搜索 `hir::|mir::|MaterializedMirPassView|devirtual` 于 `crates/scoopc/src/llvm/reachability.rs` 无命中；LLVM codegen 下 `devirtual|try_devirtualize|collect_known_receiver_subclasses|dispatch_devirtualization` 无命中。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features llvm::reachability`（LLVM feature-gated，无匹配测试但命令通过）；`cargo test -p scoopc llvm::reachability`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
  - 完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 已用 30 分钟 timeout 重跑：414/421 通过；剩余 7 个失败为既有非 P7-T02 baseline（`array_lit_infer_string_char_float_basic.scoop`、`array_lit_infer_unannotated_and_nested_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`）。

## [DONE] P7-T02-a：修复 run-pass fixture baseline 失败

- 目标：
  - 修复 P6/P7 期间反复登记的 7 个既有 run-pass fixture baseline 失败；
  - 让 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 不再保留这些非 P6/P7 baseline 失败。
- 已知失败 fixture：
  1. `tests/fixtures/run-pass/array_lit_infer_unannotated_and_nested_basic.scoop`
  2. `tests/fixtures/run-pass/array_lit_infer_string_char_float_basic.scoop`
  3. `tests/fixtures/run-pass/lang_mutable_array_new_string_ref.scoop`
  4. `tests/fixtures/run-pass/lang_mutable_array_new_struct_composite.scoop`
  5. `tests/fixtures/run-pass/std_process_args_exit_basic.scoop`
  6. `tests/fixtures/run-pass/stdlib_string_basic.scoop`
  7. `tests/fixtures/run-pass/sysroot_atomic_basic.scoop`
- 当前复现到的主要失败原因：
  1. 多数 fixture 在 LLVM 前端准备阶段失败：materialized MIR 中仍有 unresolved generic direct call target `scoop.core.println`。
  2. `lang_mutable_array_new_string_ref.scoop` 直接 `scoop run` 时会先暴露 `scoop.runtime.test.*` import 未解析；通过 fixture harness / `SYSROOT-DEPS` 注入后同样会落到 `scoop.core.println` 泛型 materialization 失败。
- 必须修改的主要位置：
  - `crates/scoopc/src/mir/materialize/`
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 或 LLVM frontend prepare 相关入口
  - `sysroot/lib/scoop.core/src/print.scoop` 及 generic `println<T>` materialization 所需 facts/实例化通道
  - `crates/scoop/src/fixtures/` 与 sysroot dependency 注入路径（仅当 `SYSROOT-DEPS` 未被正确传递时）
  - 上述 7 个 run-pass fixtures / golden（仅在语义输出确有变化时）
- 必须实现的内容：
  1. 定位为什么这些 call site 在 materialized MIR 中仍指向泛型 `scoop.core.println`，并修复实例化/替换/发布链路；不得在 LLVM backend 恢复 HIR fallback 或把 unresolved generic call 当作 codegen 特例兜底。
  2. 确认数组字面量、`MutableArray<T>`、`String` 方法、`main(args)` 与 Atomic sysroot API 相关路径都能获得 concrete callable target。
  3. 复查 `SYSROOT-DEPS: scoop.runtime.test` 是否在 run-pass harness 进入 `scoop run` 子进程时正确传递；若传递已正确，只需在完成记录说明直接 `scoop run` 与 fixture harness 行为差异。
  4. 如果修复需要新增 LIR/LIR facts 或 MIR materialization contract，必须放在正确 owner，不允许把 P7 backend cleanup 变成新的上游回看入口。
- 验证：
  1. `cargo fmt`
  2. 逐个运行上述 7 个 fixtures：`cargo run -p scoop -- test --fixtures <fixture>`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  4. `cargo clippy --all-targets -- -D warnings`
  5. `git diff --check`
- 完成条件：
  - 上述 7 个 fixtures 全部通过；
  - 全量 `tests/fixtures/run-pass` 不再保留这些 baseline 失败；
  - 修复没有新增 LLVM HIR/raw MIR fallback、codegen 去虚化 residual 或 stage output wrapper 回退。
- 依赖：P7-T02
- 完成记录：
  - 根因：MIR materializer 的 callee site binding lookup 在外层 call span 有精确 `println::<Int>` binding 时，仍会继续考察内层重叠 generic binding；`println(nested.size())` 中内层 `Array.size::<Array<Int>>` 可被过宽的 signature-shape remap 视为 `println::<Array<Int>>`，与外层精确 binding 冲突，导致 direct call target 保持泛型 `scoop.core.println`。
  - 修复：`lookup_site_instance_binding_for_callee_in` 先匹配并返回 exact call-site binding，只有没有 exact callee binding 时才回退到原有 overlapping lookup；没有新增 LLVM HIR/raw MIR fallback、codegen 去虚化 residual 或 stage output wrapper 回退。
  - 新增回归：`materialize_request_root_rewrites_nested_array_println_call_sites` 覆盖嵌套数组 `println(nested.size())` request-root body rewrite，确保 pass-visible main 不保留泛型 `scoop.core.println` direct call。
  - `SYSROOT-DEPS` 复查：`lang_mutable_array_new_string_ref.scoop` 在 fixture harness 下通过，说明 `SYSROOT-DEPS: scoop.runtime.test` 注入路径有效；直接 `scoop run` 不读取 fixture header，因此不会注入该 sysroot dependency。
  - 已知 7 个 baseline fixtures 已全部单独通过：`array_lit_infer_unannotated_and_nested_basic.scoop`、`array_lit_infer_string_char_float_basic.scoop`、`lang_mutable_array_new_string_ref.scoop`、`lang_mutable_array_new_struct_composite.scoop`、`std_process_args_exit_basic.scoop`、`stdlib_string_basic.scoop`、`sysroot_atomic_basic.scoop`。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc materialize_request_root_rewrites_nested_array_println_call_sites -- --nocapture`；逐个运行上述 7 个 fixtures；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421 passed）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [DONE] P7-T02R：Review backend reachability cleanup

- 参考：P7-T02。
- 重点：
  - reachability 是否只读 LIR/LIR facts/base context；
  - 是否删除 codegen/reachability devirtualization residual；
  - 是否没有把 MIR pass-view 换名藏入 reachability helper。
- 验证：
  - 重新运行 P7-T02 的所有验证；
  - 额外搜索 `llvm/reachability.rs` 中的 `hir::`、`mir::`、`MaterializedMirPassView`、`devirtual`。
- 完成条件：
  - review 结论明确写出 backend reachability cleanup 成立，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T02-a
- 完成记录：
  - Review 结论：backend reachability cleanup 成立；`llvm/reachability.rs` 的 collector 只消费 `LirFacts`、LIR callable/global-init/dynamic-invoke/dispatch contracts 和 stable callable keys，没有导入或扫描 HIR body、raw MIR body 或 `MaterializedMirPassView`。
  - seed/edge 复审：入口 root、global init roots、published LIR callables 与 runtime-required callables 作为 seeds；ordinary call、dynamic invoke 与 dispatch candidate edges 均从 `LirCallableFacts` / `LirDynamicInvokeContract` / `LirDispatchContract` 读取，unpublished conservative candidates 继续被忽略。
  - residual 复审：额外搜索 `llvm/reachability.rs` 中的 `hir::|mir::|MaterializedMirPassView|devirtual` 无命中；搜索 `crates/scoopc/src/llvm` 中的 `devirtual|try_devirtualize|collect_known_receiver_subclasses|dispatch_devirtualization` 无命中，未发现 codegen/reachability 普通 dispatch 去虚化残留。
  - 边界确认：LLVM 中仍存在的 `MaterializedMirPassView` / `pass_view` 命中位于 body emission、ABI layout 或 stage handoff 路径，已由后续 `P7-T03` / `P7-T04` 覆盖；本 review 未发现把 MIR pass-view 换名藏入 reachability helper 的情况。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features llvm::reachability`；`cargo test -p scoopc llvm::reachability`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421 passed）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [DONE] P7-T03：迁移 LLVM body emission 离开 raw MIR / HIR fallback

- 目标：
  - 让 LLVM body emission 使用 LIR body/source-slice/callable contracts；
  - 删除 `mir_body` raw MIR lowering 与 `main` HIR fallback lowering 的生产依赖。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/codegen/mir_body/`
  - `crates/scoopc/src/llvm/codegen/main/function.rs`
  - `crates/scoopc/src/llvm/codegen/main/expr.rs`
  - `crates/scoopc/src/llvm/codegen/main/stmt.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/body/`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc_lir_facts/`
- 必须实现的内容：
  1. effect-lowered body emission 不再从 pass view 拉 MIR function/body/local/type 信息；所需 source-slice/local/frame/signature 信息必须来自 LIR/LIR facts/base type context。
  2. 删除或测试专用化 `mir_body` production route；不允许 raw MIR body 作为 LLVM fallback。
  3. 删除 HIR expr/stmt/function/class ctor production fallback；静态源码错误不得在 backend 才暴露。
  4. 若 LIR 缺少 body emission 所需 contract，补到 P5/P6 owner 的 LIR facts，而不是保留 backend HIR/MIR 回看。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`
  3. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  4. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  5. `git diff --check`
- 完成条件：
  - LLVM production body emission 不再读取 HIR body 或 raw MIR body；
  - `llvm_residual_pass_view()` 不再用于 body emission；
  - LIR/LIR facts 是 body emission 的唯一 authoritative IR/query 输入。
- 依赖：P7-T02R
- 完成记录：
  - `LateLoweredCallable` 现在携带 stage-owned source callable/body contract；`effect_lowered` builder 在构造 LIR callable 时捕获 source body，LIR opt 重写会保留该 source contract，LLVM body emission 不再从 residual pass view 查询 callable body。
  - effect-lowered body emission 入口已移除 `MaterializedMirPassView` 参数：plain callable、effect-step callable、resume method、surface-resume owner trampoline/driver 等 production body path 均从 `LateLoweredProgram` / callable source body / `ProgramAbiQuery` / base `TypeStore` 获取 body、local、signature 和 source path。
  - `emit.rs` 删除 reachable HIR-body declaration/emission fallback：入口 root 由 LIR facts 选择，main wrapper 通过 LIR callable/source contract 获取返回类型和 span，不再要求 HIR `FunDecl` body scaffold。
  - plain/effect-neutral source-slice lowering 改用 LIR callable facts、plain call-site contract 与 published ABI query 解析 direct/dynamic/dispatch path；缺失 published callable contract 会作为 backend handoff 错误暴露，而不是回退到 HIR/raw MIR body emission。
  - 剩余 `MaterializedMirPassView` / `hir_compat_scaffold` / physical layout / TypeStore bridge residual 仍限于 stage handoff、layout 或测试辅助路径，归属后续 `P7-T04` / `P7-T04R`，本任务未新增 backend-private HIR fallback。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`；`cargo test -p scoopc llvm::codegen::effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421 passed）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [DONE] P7-T03R：Review LLVM body emission 迁移结果

- 参考：P7-T03。
- 重点：
  - production body emission 是否已离开 raw MIR/HIR；
  - remaining MIR/HIR imports 是否仅为测试 fixture 或已删除；
  - LIR facts 是否没有变成 MIR/HIR 结构的简单搬运。
- 验证：
  - 重新运行 P7-T03 的所有验证；
  - 额外搜索 `llvm/codegen` 中的 `hir::Expr`、`hir::Stmt`、`mir::Body`、`MaterializedMirPassView`、`llvm_residual_pass_view`。
- 完成条件：
  - review 结论明确写出 LLVM body emission 输入边界已收口，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T03
- 完成记录：
  - Review 结论：LLVM production body emission 输入边界按 P7-T03 收口成立；入口/main wrapper 和 callable body emission 由 `LateLoweredProgram`、LIR callable/source-body contract、`ProgramAbiQuery`、`LirFacts` 与 base `TypeStore` 驱动，不再从 `llvm_residual_pass_view()` / `MaterializedMirPassView` 查询 callable body，也不再走 HIR function/class-ctor body fallback 作为生产路径。
  - 本 review 修正了 `llvm/codegen/mir_body/mod.rs` 的过期模块注释：删除仍声称 production emit 从 `MaterializedMirPassView` / HIR-compatible boundary 进入的描述，明确该模块现在只是 LIR-owned source-slice body emission 复用的 helper，不能作为 backend body-discovery fallback。
  - 残留搜索分类：`llvm/codegen/effect_lowered` 中 `hir::Expr|hir::Stmt|mir::Body|MaterializedMirPassView|llvm_residual_pass_view` 仅命中 layout tests 对 `llvm_residual_pass_view()` 的测试输入；`MaterializedMirPassView` 在 `llvm/codegen` 的生产命中限于 `CompilationUnitCodegenCx` handoff 字段/accessor，属于后续 `P7-T04` 的 stage handoff residual；`hir::Expr|hir::Stmt|mir::Body` 的广义命中仍分布在 dead-code HIR helper、`mir_body` source-slice helper、named intrinsic/helper code 和测试构造中，未发现新的 HIR/raw-MIR body fallback 入口。
  - LIR facts 复审：`scoopc_lir_facts` 的 callable/body-slice/call-site/dynamic/dispatch/resume contract 使用 stable LIR keys、source-slice keys、ABI facts 和 control-body facts表达 backend-neutral 查询面，没有把 HIR body 或 raw MIR body 复制成新的 facts 结构。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421 passed）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`；额外搜索 `llvm/codegen` 中的 `hir::Expr|hir::Stmt|mir::Body|MaterializedMirPassView|llvm_residual_pass_view`。

## [DONE] P7-T04-a：发布 LLVM backend 收口所需的 LIR/base context 合同

- 目标：
  - 补齐 `P7-T04` 收窄 LLVM backend 输入边界所需的前置合同；
  - 让 LLVM 不再只能从 HIR side table、raw MIR pass view 或 effect facts residual 取得 initializer body、physical layout、callable identity 与 type bridge 信息。
- 阻塞原因：
  - 当前 `LirFacts.global_init` 只发布 root/storage/order 摘要，没有发布 top-level `val`、top-level `var`、object singleton initializer body/source-slice 或等价 LIR-owned init callable contract；LLVM 因此仍需要 `top_level_immutable_values`、`top_level_vars`、`object_inits` 这类 HIR side table 才能生成 eager/object init body。
  - physical ABI/layout 仍从 `class_inits`、`enum_layouts`、`class_vtables`、`interfaces`、`class_itables`、`fun_index` 等 HIR scaffold 读取 class/vtable/itable/enum/callable identity；`LirFacts` 尚未发布足以替代这些读取的 backend-neutral layout/type identity contract。
  - `LirStageOutput` 的 `types()` 仍经 `MaterializedEffectFacts` 间接取得 effect-owned `TypeStore`，并通过 `llvm_residual_pass_view()` 暴露 `MaterializedMirPassView`；TypeStore bridge 缺少单一 owner/verifier 与 cross-process stable wire-format 决策。
- 必须修改的主要位置：
  - `crates/scoopc_lir_facts/`
  - `crates/scoopc/src/pipeline/lir_facts_builder.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/effect_lowered/`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/`
  - 相关 dump/verifier/tests/fixtures
- 必须实现的内容：
  1. 在正确 owner 中发布 top-level eager init 与 object once 所需的 initializer source/body contract，或发布等价的 LIR-owned init callable contract；禁止让 `P7-T04` 继续从 HIR side table 取 initializer expr/block。
  2. 发布 physical ABI/layout 所需的 class field、enum variant/repr、vtable/itable slot、callable symbol identity 与 native callable signature contract；这些合同必须来自 LIR facts 或明确的 base/type context，不得继续依赖 `LoweredHir` 整包 scaffold。
  3. 将 LIR/backend 使用的 `TypeStore` 收口到单一 owner，并增加 verifier 校验 primary/ABI visibility type context 是否一致或可显式 remap。
  4. 对 `TypeId` / type identity 的 cross-process stable wire format 作出实现或显式推迟决策；若推迟，必须指定未来 owner 与不阻塞 `P7-T04` 的理由。
  5. 更新 stable dump 与 verifier，使 `P7-T04` 可用自动化检查发现 initializer/layout/type bridge contract drift。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_lir_facts`
  3. `cargo test -p scoopc --no-default-features lir_facts_builder`
  4. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  6. `git diff --check`
- 完成条件：
  - `P7-T04` 不再需要新增 HIR/raw MIR/effect facts 回看来收窄 stage handoff；
  - LIR facts 或 base/type context 已覆盖 LLVM physical ABI/layout 与 init body contract 所需逻辑输入；
  - TypeStore bridge 的唯一 owner、verifier 与 stable wire-format 处置已经明确。
- 依赖：P7-T03R
- 完成记录：
  - `scoopc_lir_facts` 新增 initializer source/body contract、physical layout facts、callable symbol/native/extern signature facts 与 TypeStore bridge/wire-format facts；verifier 现在检查 initializer body/root drift、layout/count drift、callable symbol签名漂移、TypeStore bridge 一致性和 wire-format owner。
  - `lir_facts_builder` 从 materialized MIR 的 data-only backend contracts 构造 LIR-owned init/layout/callable/type-context facts；top-level `val`、top-level `var`、object singleton roots 均发布 initializer source contract，physical layout 覆盖 class fields、enum repr/variants、vtable/itable/interface slots 和 callable symbols。
  - `MaterializedMir` 现在捕获 P7-T04-a 所需的 backend contract base context，后续 `P7-T04` 可从 `LIR + lir_facts + base/type context` 收窄 stage handoff，而无需新增 HIR/raw MIR/effect facts 回看。
  - TypeStore bridge 记录单一 owner 为 LIR stage base context；materialized/effect facts TypeStore 指纹一致时标记 `Identical`，否则要求显式 display-name remap。cross-process stable wire format 本轮显式推迟到 P8/per-cone build artifact serialization，因为 P7-T04 仍只消费同进程 `TypeStore` owner，不落盘持久化 `TypeId`。
  - stable dump 已展示 `physical_layout` 与 `type_context` sections；effect_lowered golden 已同步更新。
  - 验证通过：`cargo fmt`；`cargo test -p scoopc_lir_facts`；`cargo test -p scoopc --no-default-features lir_facts_builder`；`cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`；`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`；`cargo test -p scoopc llvm::codegen::effect_lowered::layout`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [DONE] P7-T04-b-1：引入 `MonoTypeId` 与 `MonoTypeKind` —— codegen 输入类型纪律基线

- 阻塞原因：
  - 当前 `cg_ty_of(TypeId) -> Option<CgTy>` 与 `expect_cg_ty_of(TypeId, &str) -> CgTy` 在签名层面承认输入 `TypeId` 可能仍含 `TypeKind::Param(_)`：codegen 内部不得不在每个出口反复 match `Param` 分支，并以 runtime warn + 上游 panic（`expect_cg_ty_of: type verifier accepted a non-codegen type while ...`）替代 type-state 检查。这本身就是 "non-codegen type 与 codegen type 共用同一 Rust 类型" 的设计缺陷；`sysroot_atomic_basic` 触发的 panic 只是其表象。
  - 后续 `P7-T04-b-2/3/4` 都以 "codegen 阶段的 TypeId 一定不含 Param" 作为类型层不变量，需要先有可被 Rust 类型系统检验的 token。
- 目标：
  - 在 `scoopc_types` 中新增 `MonoTypeId(TypeId)` newtype 与 `MonoTypeKind<'a>` 视图，结构与 `TypeKind` 同形但**无 `Param` 分支**；
  - 唯一构造路径：`TypeStore::as_mono(t: TypeId) -> Result<MonoTypeId, ParamLeak>`，必须递归校验 nominal `args`、function `params/return/effects`、tuple、option inner、union variants、StarProjection inner 等所有内嵌位置；
  - 提供 `TypeStore::kind_mono(MonoTypeId) -> MonoTypeKind`，使 children 出现位置一律暴露为 `MonoTypeId`（深度校验保证子位置已合法）；
  - 不修改任何现存调用点；既有 `cg_ty_of` / `expect_cg_ty_of` 暂保留以便后续任务分阶段迁移。
- 必须修改的主要位置：
  - `crates/scoopc_types/src/lib.rs`
  - `crates/scoopc_types/tests/`（或在同 crate 内补单元测试）
- 必须实现的内容：
  1. `MonoTypeId` 仅暴露为 `Copy + Clone + Eq + Hash + Debug` 的 newtype，**不**实现 `From<TypeId>` / `Into<TypeId>`；明确给出 `inner(self) -> TypeId` accessor 用于 hash-cons 比较与诊断输出。
  2. `as_mono` 实现采用迭代 worklist 或 visited set 防止递归类型导致栈溢出；首次发现 `Param` 时返回 `ParamLeak { offending: TypeId, leak_path: Vec<TypeKindLabel> }`，便于上游报告 "在哪个嵌套位置出现 Param"。
  3. `MonoTypeKind` 通过 `TypeStore::kind_mono` 构造时，对所有 inner TypeId 直接包成 `MonoTypeId`（无需重新 `as_mono`，因 `MonoTypeId` 的不变量已包含子树）；同时提供 `MonoTypeId::project_inner(self) -> impl Iterator<Item = MonoTypeId>` 之类辅助以便 codegen 后续遍历。
  4. 单元测试：
     - 单态化 `Int`、`String`、`Box<Int>`、`(Int, String)`、`Option<Bool>`、`Function((Int) -> String / IO)`、`Box<Box<Int>>` 等均通过 `as_mono`；
     - 包含 `Param` 的 nominal `args`、function `params/return/effects`、tuple element、option inner、union variant、star projection inner 等所有位置均被拒绝，且 `leak_path` 指向正确嵌套深度；
     - 递归类型不会无限递归；
     - 同一 `TypeId` 多次 `as_mono` 行为一致（幂等）。
  5. 显式文档说明 `MonoTypeId` 是**深度不变量**：持有 `MonoTypeId` 即意味着整棵类型树不含 `Param`；任何需要从 `TypeId` 跨越到 `MonoTypeId` 的路径都必须经过 `as_mono`。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_types`
  3. `cargo build -p scoopc`（确认旧 `TypeId` 调用路径未被破坏）
  4. `git diff --check`
- 完成条件：
  - `MonoTypeId` / `MonoTypeKind` / `as_mono` / `kind_mono` 已发布；
  - 单元测试覆盖所有 `Param` 嵌套位置；
  - 现存任何代码均未被修改（纯增量）。
- 依赖：P7-T04-a
- 完成记录：
  - 在 `crates/scoopc_types/src/lib.rs` 纯增量发布以下类型：`MonoTypeId(TypeId)` newtype（仅暴露 `inner()` accessor，无 `From<TypeId>` / `Into<TypeId>` / `unsafe`/`unchecked` 构造）、`ParamLeak { offending, leak_path }` 错误类型、`TypeKindLabel` 嵌套位置标签 enum（覆盖 10 个位置：`NominalArg` / `NominalEffect` / `UnionVariant` / `FunctionReceiver` / `FunctionParam` / `FunctionReturn` / `FunctionEffect` / `TupleElement` / `OptionInner` / `StarProjectionInner`）、`MonoTypeKind<'a>` 与并行 view（`MonoRefKind` / `MonoValueKind` / `MonoNominal` / `MonoFunction` / `MonoUnion` / `MonoEffectRow` / `MonoStarProjection`）。
  - `TypeStore::as_mono(TypeId) -> Result<MonoTypeId, ParamLeak>` 采用迭代 worklist + `HashSet<TypeId>` visited 防递归/防环；子节点按 reverse 顺序入栈以让 LIFO 弹出顺序与左到右深度优先一致，使同一 leak 输入产生稳定 `leak_path`。
  - `TypeStore::kind_mono(MonoTypeId) -> MonoTypeKind<'_>` 把 children 一律包装为 `MonoTypeId`，借出 `fqn: &'a str`，结构与 `TypeKind` 同形（去掉 `Param` 分支）。
  - 单元测试新增 19 个，覆盖：scalar/builtin 均通过、顶层 `Param` 拒绝（leak_path 为空）、所有 10 个嵌套位置的 `Param` 拒绝路径与对应 `leak_path` 断言、嵌套 `Box<Box<Box<Int>>>` 通过且不死循环、`kind_mono` 返回的 children 与原 `TypeKind` 的 inner `TypeId` 一一对应、`as_mono` 通过/拒绝路径均幂等。`cargo test -p scoopc_types` 全部 24 项通过。
  - 既有 `cg_ty_of` / `expect_cg_ty_of` 与所有调用点未修改；纯增量发布完成。
  - 验证：`cargo fmt`；`cargo test -p scoopc_types`（24 passed）；`cargo build -p scoopc`（旧 `TypeId` 调用路径仍编译通过）；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。

## [TODO] P7-T04-b-1R：Review `MonoTypeId` 类型纪律基线

- 参考：P7-T04-b-1。
- 重点：
  - `MonoTypeId` 是否真的不能从外部绕过 `as_mono` 构造（搜 `MonoTypeId(` 与 `pub fn ... -> MonoTypeId`）；
  - `as_mono` 是否覆盖所有 `TypeKind` / `RefTypeKind` / `ValueTypeKind` 子位置（包括 `EffectRow.terms`、`UnionType.variants`、`FunctionType.receiver` 等）；
  - `kind_mono` 子位置一致性是否被测试覆盖；
  - 没有任何 "fallback" 路径（例如遇到 `Param` 退化为某个默认 TypeId）。
- 验证：
  - 重新运行 P7-T04-b-1 的所有验证；
  - 额外搜索 `crates/scoopc_types/src/` 中是否存在静默把 `Param` 视为合法 codegen 类型的代码路径。
- 完成条件：
  - review 结论明确写出 `MonoTypeId` 不变量已被类型系统强制，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T04-b-1
- 完成记录：
  - 待填写。

## [TODO] P7-T04-b-2：拆分 `hir::ClassInit` 为 `GenericClassDecl` 与 `MonoClassInit`

- 阻塞原因：
  - 当前 `hir::ClassInit` 同时被 typecheck/HIR lowering 用作"泛型源声明视图"（字段/参数 TypeId 含 `Param`）与 codegen 用作"单态化实例视图"（字段/参数 TypeId 不含 `Param`），两种语义共用一个 Rust 结构，导致 `class_init_layout(class_fqn: &str)` 拿到的 `ClassInit` 究竟属于哪种语义无法在类型层判定，是 `Param` 泄漏到 `llvm_class_payload_type` 的根本原因。
  - 后续 `P7-T04-b-3` 引入 `ClassInstanceKey` 时，`class_inits` 表的值必须只承载单态化条目，否则 key 类型化无法消除 silent fallback。
- 目标：
  - 把 `hir::ClassInit` 拆为两类语义独立的 Rust 类型：`GenericClassDecl`（泛型源声明视图，字段为 `TypeId`）与 `MonoClassInit`（单态化实例视图，字段为 `MonoTypeId`），二者不可隐式互转；
  - codegen 视野中的 `ClassInitIndex` 只持有 `MonoClassInit`；任何残留 `Param` 在构造 `MonoClassInit` 时即被 `as_mono` 拒绝，升级为 monomorph driver bug。
- 必须修改的主要位置：
  - `crates/scoopc/src/hir/mod.rs`（拆分 `ClassInit` / `ClassField` / `ClassCtorParam` / `ClassInitStep` / `ClassCtorDelegation` 等结构）
  - `crates/scoopc/src/hir/lower/`（types/util/main 等所有产出 ClassInit 的位置；区分 `GenericClassDecl` 与 `MonoClassInit` 输出）
  - `crates/scoopc/src/hir/lower/util/generic_layouts.rs`（`collect_generic_class_instantiation_inits` 改为产出 `MonoClassInit`，substitute 完成后逐字段调 `as_mono`）
  - `crates/scoopc/src/pipeline/hir_stage.rs`（`lowered.class_inits.insert` 等位置；非泛型 class 直接构造 `MonoClassInit`，泛型 class 仅入 `generic_class_decls`）
  - `crates/scoopc/src/llvm/codegen/`（所有读取 `class_inits.get(...)` / `ClassField.ty` / `ClassCtorParam.ty` 的位置；类型从 `TypeId` 收紧为 `MonoTypeId`）
  - `crates/scoopc/src/mir/`（如 MIR 层有读 ClassInit，按同一原则切换）
- 必须实现的内容：
  1. 引入 `hir::ClassField<T>` / `hir::ClassCtorParam<T>` / `hir::ClassInitStep<T>` 形参化结构（或拆双类型）：`T = TypeId` 用于 `GenericClassDecl`，`T = MonoTypeId` 用于 `MonoClassInit`。`steps`、`super_ctor_args`、`ctors[].delegation`、`ctors[].body` 等所有内嵌 TypeId 位置一并对齐。
  2. `GenericClassDecl` 与 `MonoClassInit` 各自独立 struct，**不**通过 `enum` 或 `Either` 共享；codegen 公共 API 只接受 `&MonoClassInit`。
  3. `ClassInitIndex = HashMap<String, MonoClassInit>`（key 形态在 `P7-T04-b-3` 再升级为 `ClassInstanceKey`，本任务不动 keying）；同时新增 `GenericClassDeclIndex = HashMap<String, GenericClassDecl>` 供 typecheck/monomorph 使用。
  4. `collect_generic_class_instantiation_inits` 在 substitute 字段类型后**立即**调 `TypeStore::as_mono`；任一字段失败即返回明确的 monomorph driver diagnostic（含 class FQN、字段名、leak path），不允许把含 `Param` 的实例放入 `class_inits`。
  5. 非泛型 class 在 HIR lowering 阶段直接构造 `MonoClassInit`：所有字段 `TypeId` 必须能 `as_mono`，否则 typecheck 已应阻断（应作为 verifier-style assertion，不是 silent skip）。
  6. typecheck/HIR lowering 端读取的位置改为读 `GenericClassDecl`；`super_class_fqn` lookup 同时查 `GenericClassDeclIndex` 与 `MonoClassInit` 的对应关系（具体 owner 边界由本任务决策并写入文档）。
  7. 同步 layout/effect_lowered/codegen 测试（含 dump golden）以反映新类型；如 dump 中包含 ClassInit 字段类型显示，须保持稳定 wire format。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_types`
  3. `cargo test -p scoopc --no-default-features hir`
  4. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（允许在仍未引入 `ClassInstanceKey` 前出现 b-3 范围内的 codegen 错误，但**不得**新增 panic 路径）
  7. `cargo clippy --all-targets -- -D warnings`
  8. `git diff --check`
- 完成条件：
  - `hir::ClassInit` 已拆为 `GenericClassDecl` / `MonoClassInit`；
  - `class_inits: HashMap<String, MonoClassInit>` 在 codegen 视野中只承载单态化条目；
  - 任一含 `Param` 的字段在构造 `MonoClassInit` 时被拒绝并触发明确 diagnostic；
  - 现存测试集通过（除可能因尚未升级 layout key 导致 `sysroot_atomic_basic` 在 b-3 前仍触发 verifier-style 错误；此时错误位置必须比当前 `expect_cg_ty_of` panic 更精确，禁止 silent path）。
- 依赖：P7-T04-b-1R
- 完成记录：
  - 待填写。

## [TODO] P7-T04-b-2R：Review `ClassInit` 拆分

- 参考：P7-T04-b-2。
- 重点：
  - `GenericClassDecl` 与 `MonoClassInit` 是否真的是独立 Rust 类型且不可隐式互转；
  - `ClassInitIndex` 值类型是否已改为 `MonoClassInit`，是否还有任何位置往里塞含 `Param` 的字段；
  - `collect_generic_class_instantiation_inits` 是否对每个字段调 `as_mono`，失败是否上报为明确 diagnostic；
  - `GenericClassDecl` 的所有读取者是否限定在 typecheck / HIR lowering / monomorph driver，codegen 是否仅读 `MonoClassInit`。
- 验证：
  - 重新运行 P7-T04-b-2 的所有验证；
  - 额外搜索 `crates/scoopc/src/llvm/` 中对 `GenericClassDecl` 的命中（应为零）；
  - 额外搜索 `class_inits.insert(` / `class_inits.get(` 在 `hir`、`llvm`、`mir` 中的命中，确认值类型一致。
- 完成条件：
  - review 结论明确写出 `ClassInit` 双语义已分离，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T04-b-2
- 完成记录：
  - 待填写。

## [TODO] P7-T04-b-3：引入 `ClassInstanceKey` 收回 layout key 字符串形态

- 阻塞原因：
  - 当前 `MainCodegen::class_init_layout(class_fqn: &str)` / `class_ctor_layout_key`（`effect_lowered/value.rs:863-890`）/ `mir_class_ctor_layout_key`（`mir_body/types.rs:740-759`）以 `String` / `&str` 作为 layout key 形态，在 `target_local` 缺失、target 类型不是 `Ref::Nominal`、`nominal.fqn != class_fqn` 三种情况下静默 fallback 为裸 FQN；裸 FQN 虽然在 `P7-T04-b-2` 之后已无法命中 `MonoClassInit`，但字符串 keying 让"该不该 fallback"无法在类型层判定，三处 `return Ok(class_fqn.to_string())` 仍可编译通过。
- 目标：
  - 在 HIR/codegen 边界引入 `hir::ClassInstanceKey(String)` newtype，唯一构造路径来自 `MonoTypeId` 的 `RefTypeKind::Nominal` 或 `MonoClassInit::key()`；
  - `ClassInitIndex` 升级为 `HashMap<ClassInstanceKey, MonoClassInit>`；
  - 所有 `class_init_layout` / layout key 计算 helper 只接受/返回 `ClassInstanceKey`；三处静默 fallback 在类型层不再成立，被强制改为显式 `Err(verifier_bug(...))`；
  - MIR verifier 提前拦截 `ClassCtor` rvalue 的 typed target 缺失/类型不匹配情形。
- 必须修改的主要位置：
  - `crates/scoopc/src/hir/mod.rs`（新增 `ClassInstanceKey`）
  - `crates/scoopc/src/hir/lower/util/generic_layouts.rs`（`collect_generic_class_instantiation_inits` 产出 `HashMap<ClassInstanceKey, MonoClassInit>`）
  - `crates/scoopc/src/pipeline/hir_stage.rs`（`class_inits.insert(class_fqn, ...)` 改为通过 `ClassInstanceKey::for_unparameterized(class_fqn)` 一类显式构造路径）
  - `crates/scoopc/src/llvm/codegen/layout.rs`（`class_init_layout` / `class_init_layout_inner` 接 `&ClassInstanceKey`；`class_layout_key_cache` 等同步）
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`（`class_ctor_layout_key` 返回 `Result<ClassInstanceKey, _>`，删除三处 fallback）
  - `crates/scoopc/src/llvm/codegen/mir_body/types.rs`（`mir_class_ctor_layout_key` 同上）
  - `crates/scoopc/src/llvm/codegen/mir_body/args.rs`、`crates/scoopc/src/llvm/codegen/mir_body/terminator.rs`（调用点改 `&ClassInstanceKey`）
  - `crates/scoopc/src/llvm/codegen/effect_lowered/types.rs`（`ClassInstanceLayout::class_key` 返回 `&ClassInstanceKey`；`class_instance_layouts: BTreeMap<TypeId, ClassInstanceLayout>` 与新 key 对齐）
  - `crates/scoopc/src/mir/`（MIR verifier：`ClassCtor` rvalue 检查 typed target local）
- 必须实现的内容：
  1. `ClassInstanceKey` 仅暴露受控构造器：
     - `ClassInstanceKey::from_mono_nominal(types: &TypeStore, ty: MonoTypeId) -> Option<ClassInstanceKey>`（要求 `kind_mono(ty)` 是 `Ref::Nominal`，否则返回 `None` 或专门错误类型）；
     - `ClassInstanceKey::for_unparameterized(class_fqn: &str)` 仅供 HIR lowering 注册无 type-param 的 class 使用，文档明示这是 monomorphic equivalent 的便捷入口，不允许在 codegen 中调用；
     - **不**提供 `From<&str>` / `From<String>` / `unsafe`/`unchecked` 公共构造；如确需测试用 unchecked 入口，置于 `cfg(test)` 且名称含 `for_test`。
  2. `class_init_layout` / `class_init_layout_inner` / `class_layout_key_cache` 全部接 `&ClassInstanceKey`；删除接受 `&str` 的重载。
  3. `class_ctor_layout_key` 返回类型从 `Result<String, _>` 改为 `Result<ClassInstanceKey, _>`；三处 fallback 改为 `Err(verifier_bug("class ctor target local missing typed nominal …"))`，同时在错误中携带 ctor span / class FQN / target_local 状态便于诊断。`mir_class_ctor_layout_key` 同步。
  4. MIR verifier 增加规则：`ClassCtor { class_fqn, .. }` 的 rvalue **必须**写入一个类型为 `Ref::Nominal { fqn == class_fqn, args.. }` 的 local；如 MIR 构造期不满足，则在 MIR 构造点立即报错（不留给 codegen）。
  5. `ClassInstanceLayout::class_key()` 返回 `&ClassInstanceKey`，`class_instance_layouts` 内部存 `ClassInstanceKey`。
  6. 同步所有 dump golden / facts wire format（如 stable dump 中含 class key 字符串展示，仍按 mangled FQN 字符串显示，因此 wire format 不变；只是 Rust API 类型升级）。
  7. `sysroot_atomic_basic` 等含泛型 class ctor 的 fixture 必须从原 `expect_cg_ty_of` panic 升级为：要么走通正确 `ClassInstanceKey` 路径，要么在 MIR verifier 处给出明确诊断；**禁止**保留任何 silent fallback。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_types`
  3. `cargo test -p scoopc --no-default-features hir`
  4. `cargo test -p scoopc --no-default-features mir`
  5. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  7. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（含 `sysroot_atomic_basic`，应当通过或在 verifier 处给出可读诊断）
  8. `cargo clippy --all-targets -- -D warnings`
  9. `git diff --check`
- 完成条件：
  - `ClassInstanceKey` 已发布且不可被 `&str` 隐式构造；
  - `class_init_layout` / `class_ctor_layout_key` / `mir_class_ctor_layout_key` 全部走 `ClassInstanceKey`，三处静默兜底已删除；
  - MIR verifier 拦截 `ClassCtor` typed target 缺失情形；
  - `sysroot_atomic_basic` 不再触发 `expect_cg_ty_of` panic；
  - `grep "class_fqn.to_string()"` 在上述三处函数内零命中。
- 依赖：P7-T04-b-2R
- 完成记录：
  - 待填写。

## [TODO] P7-T04-b-3R：Review `ClassInstanceKey` 字符串形态收回

- 参考：P7-T04-b-3。
- 重点：
  - `ClassInstanceKey` 是否真的不能从外部以 `&str` / `String` 构造（搜 `ClassInstanceKey(` / `pub fn ... -> ClassInstanceKey`）；
  - 三处原 fallback 是否全部改成 `Err(...)`（搜 `class_fqn.to_string()`）；
  - MIR verifier 是否实际拦截 `ClassCtor` typed target 缺失/不匹配的情况；
  - `sysroot_atomic_basic` 通过路径是 codegen 正确处理还是 verifier 诊断，需有结论。
- 验证：
  - 重新运行 P7-T04-b-3 的所有验证；
  - 额外搜索 `class_init_layout` / `class_ctor_layout_key` / `mir_class_ctor_layout_key` 命中确认无 `&str` 重载或字符串 fallback。
- 完成条件：
  - review 结论明确写出 layout key 字符串形态已收回，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T04-b-3
- 完成记录：
  - 待填写。

## [TODO] P7-T04-b-4：codegen 全面切换到 `MonoTypeId` —— 删除 `cg_ty_of` 的 `Option` 与 `expect_cg_ty_of`

- 阻塞原因：
  - `P7-T04-b-1..3` 完成后，`MonoTypeId` 与 `MonoClassInit` 已使 codegen 输入边界类型化，但 codegen 内部仍以 `TypeId` 作为通用 token 传递（163 个 `cg_ty_of(TypeId) -> Option<CgTy>` 与 52 个 `expect_cg_ty_of` 调用点），`expect_cg_ty_of` panic 路径仍可在 Rust 类型层成立。要把"non-codegen type 不可能进入 codegen"作为强不变量，必须把 codegen 内部 token 一次性升级为 `MonoTypeId`。
- 目标：
  - `cg_ty_of` 签名收紧为 `(MonoTypeId) -> CgTy`（infallible），删除 `TypeKind::Param` warn 分支；
  - 删除 `expect_cg_ty_of` 函数及全部 52 处调用；
  - codegen 内部所有承载 codegen-stage 类型的位置（`mir::Local.ty`、`MirLocalSlot.ty`、`MonoClassInit` 之外的 struct/enum/tuple field 类型、expression `expr.ty` 等）一律走 `MonoTypeId`；MIR→codegen 边界做唯一一次 `as_mono` 校验，失败即 verifier bug，hard fail 并显示来源 span。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/codegen/ty.rs`（`cg_ty_of` 签名收紧；删除 `Param` 分支与 `monomorph miss` 警告）
  - `crates/scoopc/src/llvm/codegen/main/context.rs`（删除 `expect_cg_ty_of`）
  - `crates/scoopc/src/llvm/codegen/`（全部 `cg_ty_of` / `expect_cg_ty_of` 调用点；按需要把上游 `local.ty` / `param.ty` / `field.ty` 调整为 `MonoTypeId`）
  - `crates/scoopc/src/mir/mod.rs`（如选用 `mir::Local.ty: MonoTypeId` 方案，则 MIR 构造点同步加 `as_mono`）
  - `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs`（`MirLocalSlot` / slot.cg_ty 处理）
  - 其他 struct/enum/tuple layout（layout 表内部字段 TypeId）
- 必须实现的内容：
  1. 选择 `mir::Local.ty: MonoTypeId` 还是"在 codegen 入口对 `mir::Local.ty: TypeId` 调一次 `as_mono`" —— 决策须写在任务完成记录中。优先采用前者（结构层面写死不变量），仅当 MIR 必须承载尚未单态化的泛型模板时才允许后者；本任务必须给出依据。
  2. `cg_ty_of(MonoTypeId) -> CgTy`：
     - 不再返回 `Option`；
     - `Param` 分支因类型不可达而**编译期消失**；
     - 内部读取 `kind_mono` 而非 `kind`，保证内嵌 TypeId 一并以 `MonoTypeId` 出现。
  3. 删除 `expect_cg_ty_of` 与所有 52 处 panic context 字符串；调用点直接以 `cg_ty_of(...)` 替代。
  4. 删除 `cg_ty_of` 中"`monomorph miss` 警告"代码；任何 `as_mono` 失败必须在源头（MIR 构造、layout 注册、`collect_generic_*` 等）以 hard error 终止编译，不再以 warn 残留。
  5. 删除 `codegen_type_store_for_type_id` 等仅为 fallback 服务的 helper；保留必要的 cross-store 桥接但只接受 `MonoTypeId`（且经过 P7-T04-a 引入的 TypeStore bridge owner 校验）。
  6. struct/enum/tuple layout 表内部字段类型（`StructField.ty` / `EnumVariant.fields[].ty` / `TupleLayout.elements` 等）一并升级为 `MonoTypeId`；构造点（HIR `collect_generic_struct_instantiation_layouts` / `collect_generic_enum_instantiation_layouts` 等）按 `MonoClassInit` 一致原则做 `as_mono` 校验。
  7. 同步 layout/codegen/effect_lowered 测试与 dump golden；任何依赖 `Option<CgTy>` 的 helper 重写为 infallible 形式。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_types`
  3. `cargo test -p scoopc --no-default-features hir`
  4. `cargo test -p scoopc --no-default-features mir`
  5. `cargo test -p scoopc --no-default-features llvm::codegen`
  6. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`
  7. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  8. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（全绿）
  9. `cargo clippy --all-targets -- -D warnings`
  10. `git diff --check`
- 完成条件：
  - `grep -r "expect_cg_ty_of" crates/scoopc/src` 零命中；
  - `grep -r "monomorph miss" crates/scoopc/src` 零命中；
  - `cg_ty_of` 签名为 `(MonoTypeId) -> CgTy`，函数体不含 `Param` 分支；
  - `mir::Local.ty` 等关键边界类型已切换到 `MonoTypeId`，并在完成记录中说明决策依据；
  - 全部 fixture suite 与 layout/codegen test 全绿。
- 依赖：P7-T04-b-3R
- 完成记录：
  - 待填写。

## [TODO] P7-T04-b-4R：Review codegen `MonoTypeId` 全面切换

- 参考：P7-T04-b-4。
- 重点：
  - `cg_ty_of` 是否仍存在 `Option` 返回或 `Param` 分支；
  - `expect_cg_ty_of` / `monomorph miss` 是否真已删除；
  - `mir::Local.ty` / `MirLocalSlot.ty` / struct/enum/tuple layout field 是否都已是 `MonoTypeId`；
  - 是否有任何位置仍在 codegen 内构造 `MonoTypeId` 时绕过 `as_mono`（搜 `MonoTypeId(`）。
- 验证：
  - 重新运行 P7-T04-b-4 的所有验证；
  - 额外搜索 `crates/scoopc/src/llvm/codegen/` 中 `TypeId` 仍出现在 codegen-internal 函数签名的位置（应仅限于 `as_mono` 入口、诊断输出、stable dump 等明确允许的边界）。
- 完成条件：
  - review 结论明确写出 codegen 内部 token 已全部切到 `MonoTypeId`，"non-codegen type" 在 codegen 内部 Rust 类型层不再可达，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T04-b-4
- 完成记录：
  - 待填写。

## [TODO] P7-T04-b：收窄 LLVM stage handoff 形状

- 目标：
  - 删除 `LlvmCodegenStageOutput` / `StageEmitInput` / `LirStageOutput` 中对 `LoweredHir`、`HirFacts`、`EffectLoweredStageOutput` wrapper、`MaterializedMirPassView` residual 的长期携带；
  - 让 LLVM backend 输入显式只携带 `LIR + LIR facts + base context (含 TypeStore bridge owner) + backend options`；
  - 不动 LLVM 内部对 class/vtable/itable/enum/type identity 的查询源（仍由 P7-T04-c 负责迁到 LIR facts），但要把这些查询源的数据 owner 收敛到 P7-T04-a 已发布的 backend contract base context（`MaterializedMir.backend_contracts` 等），不再走 `LoweredHir` 整包 scaffold。
- 必须修改的主要位置：
  - `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`
  - `crates/scoopc/src/pipeline/effect_lowering_stage.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/tests/`（仅同步 helper 输入 shape，不改 layout 业务逻辑）
  - `crates/scoopc/src/mir/materialize/`（按需扩展 `MaterializedBackendContracts` 或新增 sibling base context owner，承接 P7-T04-a 标注但尚未迁入的 HIR scaffold side tables）
- 必须实现的内容：
  1. 在 `pipeline/llvm_codegen_stage.rs` 中显式定义 LLVM backend 输入合同：`LlvmCodegenStageOutput` 不再带 `LoweredHir` 字段、`HirFacts` 字段或 `EffectLoweredStageOutput` wrapper；改为暴露 LIR program、LIR facts、ABI visibility LIR/LIR facts、和单一 `LlvmStageBaseContext`（owner = `MaterializedMir` 加必要的 HIR-equivalent side tables 容器，不得再以 `LoweredHir` 类型出现在公共 API 上）。
  2. `StageEmitInput` 同步收窄：不再以 `&EffectLoweredStageOutput` / `&LoweredHir` / `&HirFacts` 形式接收输入，而是按 `LlvmCodegenStageOutput` 暴露的 LIR + LIR facts + base context 接口接收。
  3. `LirStageOutput` 删除 `LirStageContext` 与 `llvm_residual_pass_view()` accessor；`MaterializedMir` 与 `MaterializedEffectFacts` 不再藏在 LIR stage output 里被 LLVM 反向取出，而是由 LLVM stage runner 显式从上游获取并放进 `LlvmStageBaseContext`。`LirStageOutput::types()` 这类 LLVM-only accessor 也一并删除或重新归位。
  4. ABI visibility 通道按 1/2 同样收窄：`abi_visibility_*` 字段只保留 ABI visibility 所需的 LIR/LIR facts/TypeStore，不再承载 P5 wrapper 或重复 base context；如果 ABI visibility 与 primary 共享 base context，应在文档与 verifier 中明确一致性约束。
  5. 在 `LlvmStageBaseContext` 内显式记录单一 TypeStore owner（`MaterializedMir.types` 或等价位置），使后续 `P7-T04-c` 与 `P7-T04` closure 可以用一处入口校验 primary/ABI visibility 类型上下文一致性。
  6. 不在 `LirFacts` / `MaterializedBackendContracts` 中复制冗余 callable side table；如确需新增 backend-private fields，必须以 P7-T04-a 已发布 base context 为 owner，并在 dump/verifier 中体现。
  7. 显式记录 `TypeId` cross-process stable wire format 推迟决策（沿用 P7-T04-a 的推迟到 P8/per-cone build artifact serialization 的结论），在 `LlvmStageBaseContext` 文档与 LIR stable dump 任一处留下永久记号，避免 P7-T04-c / P7-T04 closure 时再现场重新论证。
  8. 同步更新所有受影响 tests / fixtures / dump golden（特别是 `pipeline::llvm_codegen_stage` 测试与 `effect_lowered/layout/tests/` 中的 helper 构造），保证不再依赖被删除的 accessor。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm_codegen_stage`
  3. `cargo test -p scoopc --no-default-features pipeline::effect_lowering_stage`
  4. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`
  5. `cargo run -p scoop_tools -- dependency-gate`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  7. `cargo clippy --all-targets -- -D warnings`
  8. `git diff --check`
- 完成条件：
  - `LlvmCodegenStageOutput` / `StageEmitInput` / `LirStageOutput` 公共 API 中不再出现 `LoweredHir` / `HirFacts` / `EffectLoweredStageOutput` 类型作为命名字段或 accessor；
  - `llvm_residual_pass_view()` 与 `LirStageContext` 已被删除；
  - LLVM backend 仍能完整通过 run-pass fixture 与 layout tests，physical ABI/layout 内部读取顺势走 base context 的同一份数据 owner（P7-T04-c 之后才会迁到 LIR facts）；
  - `TypeId` wire-format 推迟决策已在 stage handoff 现场留记号。
- 依赖：P7-T04-b-4R
- 完成记录：
  - 待填写。

## [TODO] P7-T04-bR：Review LLVM stage handoff 形状收窄

- 参考：P7-T04-b。
- 重点：
  - LLVM backend 公共 API 是否真的脱离 `LoweredHir` / `HirFacts` / `EffectLoweredStageOutput` 类型；
  - `LirStageOutput` 是否还保留任何 LLVM-only accessor；
  - `LlvmStageBaseContext` 是否单一 owner，且 ABI visibility 一致性约束已明确；
  - HIR scaffold 是否被以非 `LoweredHir` 形式继续藏在 base context 中并打上正确归属（P7-T04-c 仍会把 physical layout 数据真正迁到 LIR facts，本 review 不要求那一步先完成）。
- 验证：
  - 重新运行 P7-T04-b 的所有验证；
  - 额外搜索 `hir_compat_scaffold`、`llvm_residual_pass_view`、`EffectLoweredStageOutput`、`MaterializedMirPassView`、`HirFacts` 在 `crates/scoopc/src/llvm/`、`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`、`crates/scoopc/src/pipeline/effect_lowering_stage.rs` 中的命中。
- 完成条件：
  - review 结论明确写出 stage handoff 形状已收窄，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T04-b
- 完成记录：
  - 待填写。

## [TODO] P7-T04-c：迁移 physical ABI/layout 查询面到 LIR facts

- 目标：
  - 让 `effect_lowered/layout/` 与 `effect_lowered/ty.rs` 等 LLVM physical ABI/layout 路径只读 `LirFacts.physical_layout` / `LirFacts.type_context` / `LirFacts.callable_symbols` 等 P7-T04-a 发布的合同；
  - 删除 `CompilationUnitCodegenCx` 中只为 physical layout 提供的 HIR scaffold side table（`class_inits` / `class_vtables` / `interfaces` / `class_itables` / `enum_layouts` 等）依赖，转而走 LIR facts；
  - 不动 stage handoff 形状（已由 P7-T04-b 完成）。
- 必须修改的主要位置：
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/abi.rs`
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/`（其他必要文件）
  - `crates/scoopc/src/llvm/codegen/effect_lowered/ty.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`（裁剪 `CompilationUnitCodegenInputs` 中只为 physical layout 服务的 HIR scaffold 字段）
  - `crates/scoopc/src/llvm/codegen/effect_lowered/layout/tests/`（同步测试）
  - `crates/scoopc_lir_facts/`（仅在确实缺合同时补；优先复用 P7-T04-a 已发布 facts）
- 必须实现的内容：
  1. 把 `effect_lowered/layout/abi.rs` 中 `class_key = crate::hir::mangle_nominal_fqn(...) → self.codegen.class_inits.get(&class_key)` / `enum_layouts.get(...)` 等读取，改为基于 `LirFacts.physical_layout.classes` / `LirFacts.physical_layout.class_vtables` / `LirFacts.physical_layout.interfaces` / `LirFacts.physical_layout.class_itables` / `LirFacts.physical_layout.enums`（按 P7-T04-a 实际发布的字段名）查询。
  2. 把 `effect_lowered/ty.rs` 中类似的 HIR scaffold 查询同步迁出。
  3. `CompilationUnitCodegenCx` / `CompilationUnitCodegenInputs` 中仅为 physical layout 服务的 HIR scaffold 字段裁剪掉，或改为对 `LirFacts` 的引用。如果某些字段在 body emission / runtime helper 路径仍被使用，必须显式划清责任，仅删除 physical layout 不再需要的部分。
  4. 多 `TypeStore` 桥接由 `LlvmStageBaseContext`（P7-T04-b 引入）单一 owner 收口，并在 physical ABI/layout 的入口处加 verifier，确认 ABI visibility 与 primary 类型上下文一致性。
  5. 同步更新 layout tests，让 helper 直接基于 LIR facts + base context 构造 `ProgramAbiMaterializer` 输入，不再吃 HIR side table。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_lir_facts`
  3. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`
  4. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  6. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  7. `cargo clippy --all-targets -- -D warnings`
  8. `git diff --check`
- 完成条件：
  - `effect_lowered/layout/` 与 `effect_lowered/ty.rs` 不再读取 `class_inits` / `class_vtables` / `interfaces` / `class_itables` / `enum_layouts` 等 HIR scaffold side table；
  - physical layout 路径中没有 `crate::hir::mangle_nominal_fqn` 等 HIR-only helper 残留（如确需相同的 mangle 行为，须封装到 LIR facts 已发布的 callable symbol / type identity 上）；
  - layout 与 run-pass fixture suite 全绿；
  - TypeStore bridge verifier 在 LLVM physical layout 入口实际触发。
- 依赖：P7-T04-bR
- 完成记录：
  - 待填写。

## [TODO] P7-T04-cR：Review physical ABI/layout 迁移结果

- 参考：P7-T04-c。
- 重点：
  - physical ABI/layout 是否只读 LIR facts / base type context；
  - `CompilationUnitCodegenCx` / `CompilationUnitCodegenInputs` 中是否还残留只为 physical layout 服务的 HIR scaffold 字段；
  - layout tests 是否真的按 LIR facts 构造，未隐藏 HIR side table 来源；
  - TypeStore bridge verifier 是否在 layout 入口生效。
- 验证：
  - 重新运行 P7-T04-c 的所有验证；
  - 额外搜索 `class_inits` / `class_vtables` / `interfaces` / `class_itables` / `enum_layouts` / `crate::hir::mangle_nominal_fqn` 在 `crates/scoopc/src/llvm/codegen/effect_lowered/` 中的命中。
- 完成条件：
  - review 结论明确写出 physical ABI/layout 已迁到 LIR facts，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T04-c
- 完成记录：
  - 待填写。

## [TODO] P7-T04：收尾——LLVM stage handoff 与 physical ABI cleanup 合并验证

- 目标：
  - 在 `P7-T04-b` 与 `P7-T04-c` 完成后，做一次合并验证与 wire-format 推迟落实，确保原 P7-T04 的 6 项要点全部成立；
  - 显式表态 `TypeId` cross-process stable wire format 处置（沿用 P7-T04-a / P7-T04-b 的推迟到 P8/per-cone build artifact serialization）。
- 必须修改的主要位置：
  - 仅文档/dump 同步（`PIPELINE_REFACTOR.md`、`PIPELINE-CLEANUP.md`、`README.md`、LIR stable dump 中的 wire-format 推迟记号），实现部分由 -b/-c 已完成。
- 必须实现的内容：
  1. 综合 -b/-c 完成度，确认 P7-T04 原 6 项要点（LLVM backend input 收窄、`LlvmCodegenStageOutput` / `StageEmitInput` 不再带 P5/HIR wrapper、`LirStageOutput` 不再带 LLVM residual、physical ABI/layout 走 LIR facts / base context、单一 TypeStore owner + verifier、wire-format 推迟）全部满足。
  2. 在 LIR stable dump 或同等 freezable 文档处永久记录 `TypeId` cross-process wire format 推迟决策（owner = P8/per-cone build artifact serialization；不阻塞 P7 验收的理由 = LLVM 仍只消费同进程 `TypeStore` owner，不落盘持久化 `TypeId`）。
  3. 跑一次完整验证集（见下），确认 -b/-c 的局部验证未掩盖跨任务回归。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features llvm_codegen_stage`
  3. `cargo test -p scoopc --no-default-features llvm::codegen::effect_lowered::layout`
  4. `cargo run -p scoop_tools -- dependency-gate`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  6. `cargo clippy --all-targets -- -D warnings`
  7. `git diff --check`
- 完成条件：
  - LLVM stage wrapper 不再嵌套或传播上游整包 stage output；
  - `llvm_residual_pass_view()` 等 P7 residual accessor 已删除；
  - physical ABI/layout 只把 LIR facts 映射成 LLVM-private layout，不回读 HIR/raw MIR/effect facts；
  - TypeStore 桥接收口结论中已经显式表态 cross-process stable wire format 的处置（落实或显式推迟 + owner）。
- 依赖：P7-T04-bR、P7-T04-cR
- 完成记录：
  - 待填写。

## [TODO] P7-T04R：Review LLVM stage handoff 与 physical ABI cleanup

- 参考：P7-T04。
- 重点：
  - `LlvmCodegenStageOutput` / `StageEmitInput` 是否不再传播 P5 wrapper 或 HIR scaffold；
  - `LirStageOutput` 是否不再保留 LLVM residual pass-view context；
  - physical ABI/layout 是否只做 backend-private 物理化；
  - TypeStore 桥接收口结论是否已显式表态 cross-process stable wire format 的处置（落实或显式推迟 + 未来 owner），未来 per-cone build artifact 落地不会因为 `TypeId` 不可序列化而被迫重新触动 backend。
- 验证：
  - 重新运行 P7-T04 的所有验证；
  - 额外搜索 `hir_compat_scaffold`、`llvm_residual_pass_view`、`EffectLoweredStageOutput`、`MaterializedMirPassView`、`EffectFactsStageOutput` 在 LLVM/pipeline handoff 中的生产命中。
- 完成条件：
  - review 结论明确写出 LLVM backend 输入边界已收口，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T04
- 完成记录：
  - 待填写。

## [TODO] P7-T05：P7 全包清场、文档同步与依赖审计

- 目标：
  - 对 LLVM backend cleanup 做最终清场；
  - 确认 codegen backend 只依赖 `LIR + LIR facts + base context`；
  - 同步文档、TODO 和 dependency gate。
- 必须修改的主要位置：
  - `TODO.md`
  - `TODO-6.md`
  - `PIPELINE-CLEANUP.md`
  - `PIPELINE_REFACTOR.md`（仅在设计边界实际变化时）
  - `README.md`
  - `tools/scoop_tools/src/dependency_gate.rs`（如需新增 backend 边界门禁）
- 必须实现的内容：
  1. 搜索确认 LLVM backend 无 HIR body/raw MIR body/effect facts/stage output wrapper 输入。
  2. 搜索确认 codegen/reachability 无 ordinary dispatch devirtualization residual。
  3. dependency gate 覆盖或记录 LLVM backend 输入边界，避免后续回退。
  4. 文档中明确 LLVM 与未来 C backend 共享同一套 backend-neutral 输入。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo test -p scoopc_lir_facts`
  4. `cargo test -p scoopc --no-default-features llvm_codegen_stage`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  6. `cargo clippy --all-targets -- -D warnings`
  7. `git diff --check`
- 完成条件：
  - P7 backend cleanup 完成；
  - `TODO.md` 与本文件状态同步；
  - P8 可以只做最终 residual 搜索、全仓验证和接口冻结。
- 依赖：P7-T04R
- 完成记录：
  - 待填写。

## [TODO] P7-T05R：Review P7 全包完成度

- 参考：P7-T05。
- 重点：
  - LLVM backend 是否只消费 `LIR + LIR facts + base context`；
  - 是否没有 HIR/raw MIR/effect facts/stage output wrapper residual；
  - P8 是否只剩最终验证和文档冻结。
- 验证：
  - 重新运行 P7-T05 的所有验证；
  - 额外搜索 `crates/scoopc/src/llvm` 与 `crates/scoopc/src/pipeline` 中的上游 stage output / HIR / MIR residual。
- 完成条件：
  - review 结论明确写出 P7 完成、P8 可以开始，或列出阻塞项并在本 review 内修复。
- 依赖：P7-T05
- 完成记录：
  - 待填写。

## [TODO] P8-T01：最终 residual 搜索、文档冻结与未来 C backend 输入边界

- 目标：
  - 全仓确认 pipeline refactor 不再保留旧 surface、旧 owner 或 stage output 嵌套；
  - 冻结 LLVM 与未来 C backend 的共享输入边界。
- 必须修改的主要位置：
  - `TODO.md`
  - `TODO-6.md`
  - `PLAN.md`（仅当阶段计划实际变化）
  - `PIPELINE_REFACTOR.md`
  - `PIPELINE-CLEANUP.md`
  - `README.md`
  - crate overview / dependency gate 文档
- 必须实现的内容：
  1. 搜索确认无旧 comptime/const surface 或专门兼容逻辑。
  2. 搜索确认无 HIR 层去虚化、无 codegen/reachability 普通去虚化。
  3. 搜索确认无 `StageOutput` 嵌套上游整包或下游把两个 fact table 当替代输入。
  4. 文档冻结未来 C backend 的输入：`LIR + LIR facts + base context`，不允许复制 LLVM 旧 HIR/raw MIR coupling。
  5. 若 dependency gate 还不能覆盖最终约束，补齐 gate 或在 TODO 中留下明确阻塞项，不能只靠人工记忆。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo run -p scoop_tools -- spec-fixtures check`
  4. `cargo test --all --all-targets`
  5. `git diff --check`
- 完成条件：
  - 文档、TODO、README 与实现边界一致；
  - 最终 residual 搜索无违反 P0-P8 硬约束的生产命中；
  - 未来 C backend 输入边界明确且可被 dependency gate 或文档验收。
- 依赖：P7-T05R
- 完成记录：
  - 待填写。

## [TODO] P8-T01R：Review final residual 搜索与文档冻结

- 参考：P8-T01。
- 重点：
  - residual 搜索是否覆盖 P0-P8 所有硬约束；
  - 文档是否没有把 workaround 或 historical residual 描述成长期合法边界；
  - 未来 C backend 输入边界是否足够具体。
- 验证：
  - 重新运行 P8-T01 的所有验证；
  - 抽查 residual 搜索命令和 dependency gate 规则是否能防止明显回退。
- 完成条件：
  - review 结论明确写出 residual/documentation freeze 成立，或列出阻塞项并在本 review 内修复。
- 依赖：P8-T01
- 完成记录：
  - 待填写。

## [TODO] P8-T02：最终全仓验证与 release readiness 清场

- 目标：
  - 运行最终验证矩阵；
  - 确认所有 P0-P8 任务完成记录、TODO 索引、文档和实现状态一致；
  - 为首次 release tag 前的最终自动化验收留出干净状态。
- 必须修改的主要位置：
  - `TODO.md`
  - `TODO-6.md`
  - 任何最终验证发现不一致的源码、fixture 或文档
- 必须实现的内容：
  1. 全量运行最终验证命令，任何失败必须修复后重跑对应范围。
  2. 确认所有任务标题状态与完成记录一致；完成记录有内容但标题未 `[DONE]` 的任务必须修正。
  3. 确认没有未提交的临时 fixture、golden、debug dump 或 plan workaround。
  4. 如果全量 fixture 中任何单个测试卡住超过 1 分钟，必须定位并修复，不得仅跳过。
- 验证：
  1. `cargo fmt`
  2. `cargo run -p scoop_tools -- dependency-gate`
  3. `cargo run -p scoop_tools -- spec-fixtures check`
  4. `cargo test --all --all-targets`
  5. `cargo run -p scoop -- test`（完整 fixture suite，至少 30 分钟 timeout）
  6. `cargo clippy --all-targets -- -D warnings`
  7. `git diff --check`
- 完成条件：
  - 最终验证矩阵通过；
  - `TODO.md` 与所有 `TODO-[1-6].md` 状态同步；
  - 代码、fixtures、文档和 dependency gate 不再暴露 P0-P8 residual。
- 依赖：P8-T01R
- 完成记录：
  - 待填写。

## [TODO] P8-T02R：Review final verification 与 release readiness

- 参考：P8-T02。
- 重点：
  - 最终验证是否完整通过；
  - P0-P8 是否没有遗漏的未完成任务或未跟踪 blocker；
  - release tag 前是否还有必须修复的工作区、文档或测试风险。
- 验证：
  - 重新运行 P8-T02 的所有验证，或在验证成本过高时至少重跑失败修复相关范围并明确引用最近一次完整通过记录；
  - 复查 `git status`、TODO 状态、residual 搜索结果。
- 完成条件：
  - review 结论明确写出项目实现完成并可进入最终 release tag 流程，或列出阻塞项并在本 review 内修复。
- 依赖：P8-T02
- 完成记录：
  - 待填写。
