# TODO — P1：LIR↔codegen handoff（抽出独立 LIR 阶段）

> 计划：[`PLAN.md`](./PLAN.md) §3 P1；设计与契约：[`FACT_REFACTOR.md`](./FACT_REFACTOR.md) §12（codegen 输入需求）/§13（新 LIR 草案）/§14（handoff 契约）。
> 路线：strangler + 后→前。本阶段**只动 LIR↔codegen 边界与阶段切分**，不碰 LIR 内部表示。

## 0. 目标与非目标

**目标**：把「effect-facts + LIR lowering + base-context 构建」从 LLVM codegen 阶段**抽出**为独立的 LIR 阶段，产出自包含的 `LirArtifact`；codegen 收缩为「消费 `CodegenInput` → emit」。主 cone 与依赖 cone 拿到**同一类型** `LirArtifact`。codegen 不再直接消费 `LoweredHir`/`MaterializedMir`/`Index`/`TypeEnv`。

**非目标（本阶段明确不做，留给 P2+）**：
- 不重设计 LIR 内部（`LateLoweredProgram`/`LirFacts` 维持现状，作为 `LirArtifact` 的内部字段被包进来）。
- 不消除 LIR-overlay：`LateLoweredProgram` 仍切片回 `mir::Body`（`ir.rs` `LateLoweredSourceBody = crate::mir::Body`），故 `LirArtifact` **暂时**携带 `MaterializedMir` 作为 overlay 后备（P2 lift 指令后删除）。
- 不引入 arena / 定型句柄全集（仅按需引入 `entry` 的解析引用）。
- 不删 `LirFacts` 平表、不动 `dependency_gate.py` 黑名单（P2+ 随结构消失再清）。

> 判定本阶段完成的标志：`llvm_codegen_stage::run` 的输入里**不再出现** `lowered_hir`/`materialized_mir`/`frontend_index`/`type_env`；主/依赖 cone 统一为 `LirArtifact`；全套验证基线绿。

## 1. 代码地图（直接按符号定位，勿全库搜索）

**入口与当前流程** — `crates/scoopc/src/pipeline/llvm_codegen_stage.rs`：
- `struct LlvmCodegenStageInput { lowered: CodegenLoweringOutput, abi_visibility_lowered: Option<CodegenLoweringOutput>, source_map: SourceMap, entry_source_id: SourceId, entry_main_fqn: Option<String>, opt_level: OptLevel, cached_dep_artifacts: Vec<CachedDepArtifactHandoff> }`；构造器 `new(...)`、`with_cached_dep_artifacts(...)`。
- `fn run(session, input) -> Result<LlvmCodegenStageOutput, LlvmEmitError>`（约 :345）：解构 input → `lowered.into_parts()` 得 `(lowered_hir, materialized_mir, frontend_index, type_env)` → 调 `run_lir_stage_from_lowered_hir(...)` → `primary_run.output.into_parts()` 得 `(lir, lir_facts)` → 对 `abi_visibility_lowered` 再跑一次 → 之后（约 :399 起）用 `lir + lir_facts + base_context + cached_dep_artifacts` 实际 emit。
- `fn run_lir_stage_from_lowered_hir(...) -> Result<LlvmLirRun, LlvmEmitError>`（:293）：**这就是要抽出的整段后端流水线**——
  1. `HirStageOutput::new_with_frontend_artifact(lowered_hir, &source_path, frontend_index, type_env)`；
  2. `mir_stage::run(typed_hir_output)?.with_materialized_mir(materialized_mir)`；
  3. `super::build_effect_facts_stage_output(session, entry_source, &mir_stage_output)?`；
  4. `super::effect_lowering_stage::build_lir_stage_output_from_stage_outputs(&mir_stage_output, &effect_facts_stage_output, opt_options)?`；
  5. `build_llvm_stage_base_context_from_lowered_hir(base_hir, hir_facts, materialized_mir, effect_facts)`；
  6. `base_context.verify_lir_type_context(output.lir_facts(), "primary")?`；
  7. 返回 `LlvmLirRun { output: LirStageOutput, base_context: LlvmStageBaseContext }`。
- `struct LlvmLirRun { output: LirStageOutput, base_context: LlvmStageBaseContext }`（:281）；`fn into_abi_visibility_parts(self) -> (LateLoweredProgram, LirFacts, TypeStore)`（:287）。
- `fn build_llvm_stage_base_context_from_lowered_hir(lowered_hir, hir_facts, materialized_mir, effect_facts) -> LlvmStageBaseContext`（:206）。

**lowered 包** — `crates/scoopc/src/frontend.rs`：
- `struct CodegenLoweringOutput { lowered_hir: hir::LoweredHir, materialized_mir: crate::mir::MaterializedMir, frontend_index: Index, type_env: TypeEnv }`；`into_parts()`。

**依赖 handoff 与 base context** — `crates/scoopc_codegen_llvm/src/llvm/handoff.rs`：
- `struct CachedDepArtifactHandoff { cone_id: ConeId, stable_cone_key: StableConeKey, lir: LateLoweredProgram, lir_facts: LirFacts, type_store: TypeStore, object_files: Vec<PathBuf> }`。
- `struct LlvmStageBaseContext`（含 `new(...)`、`into_type_store()`、`verify_lir_type_context(&LirFacts, &str)`、`types()`）。

**调用方**（改这里来串新阶段）：
- `crates/scoopc/src/pipeline/mod.rs`：`pub(crate) fn run_llvm_codegen_stage(...)`（:183）；构造 `LlvmCodegenStageInput::new(...)`（:226）与 `::with_cached_dep_artifacts(...)`（:446）。`build_effect_facts_stage_output`（:105）、`build_lir_stage_output`（:121）也在此。
- `crates/scoopc/src/single_cone.rs:165`：`run_llvm_codegen_stage(session, LlvmCodegenStageInput::with_cached_dep_artifacts(...))`。
- `crates/scoopc/src/pipeline/effect_lowering_stage.rs`：`build_lir_stage_output_from_stage_outputs(...)`、`struct LirStageOutput`（`into_parts() -> (LateLoweredProgram, LirFacts)`、`lir_facts()`）。

**LIR/facts/ids 定义**（本阶段只引用，不改内部）：
- `crates/scoopc_lir/src/effect_lowered/ir.rs`：`LateLoweredProgram`；`type LateLoweredSourceBody = crate::mir::Body`（:350，overlay 证据）。
- `crates/scoopc_lir_facts/src/lib.rs`：`struct LirFacts`。
- `crates/scoopc_ids/src/lib.rs`：`StableConeKey`、`StableLirCallableKey { canonical_text, readable_path }`、`CanonicalTextKey`。

## 2. 目标类型（本阶段过渡形态）

放在新文件 `crates/scoopc/src/pipeline/lir_artifact.rs`（或 codegen crate 的 handoff 模块，二选一，保持单一定义）：

```rust
/// 一个 cone 在 LIR 阶段的自包含产物。过渡期内部仍是现有 LIR 表示。
pub struct LirArtifact {
    pub cone: StableConeKey,
    pub program: LateLoweredProgram,        // P2 替换为新 LirProgram
    pub facts: LirFacts,                    // P2 折叠进 program
    pub base_context: LlvmStageBaseContext, // 类型/布局上下文（含 TypeStore）
    pub mir: MaterializedMir,               // 过渡：overlay 后备，P2 lift 指令后删除
    pub object_files: Vec<PathBuf>,         // 依赖链接产物；主 cone 为空
}

/// codegen 的全部输入——只有 LIR + 诊断/入口/选项。
pub struct CodegenInput {
    pub program: LirArtifact,
    pub abi_shell: Option<LirArtifact>,     // 原 abi_visibility_lowered 的产物
    pub deps: Vec<LirArtifact>,             // 依赖 cone，与 main 同类型
    pub entry: Option<EntryRef>,            // 见 T1-06；替 entry_source_id+entry_main_fqn
    pub source_map: SourceMap,              // 仅诊断锚点
    pub opt_level: OptLevel,
}
```

## 3. 任务（按序执行；每个收尾跑 §4 基线）

### [DONE] T1-01：新增 `LirArtifact` / `CodegenInput` 类型
- 新建 `crates/scoopc/src/pipeline/lir_artifact.rs`，定义上述两个 struct（`entry` 字段先用 `Option<(SourceId, Option<String>)>` 占位，T1-06 再换 `EntryRef`）。在 `pipeline/mod.rs` `pub mod lir_artifact;` 并按需 re-export。
- 验收：编译通过；尚无行为变化。
- 完成记录（2026-06-03）：新增 `pipeline/lir_artifact.rs`，定义 `LirArtifact` 与 `CodegenInput` 过渡类型；在 `pipeline/mod.rs` 按 `llvm` feature 导出模块与类型，未改变运行行为。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [TODO] T1-02：抽出独立 LIR 阶段函数 `build_lir_artifact`
- 在 `llvm_codegen_stage.rs`（或迁到 `pipeline/lir_stage.rs`）新增 `pub(crate) fn build_lir_artifact(session, entry_source, lowered: CodegenLoweringOutput, preserve_published_resume_shells: bool) -> Result<LirArtifact, LlvmEmitError>`：
  - 内部 = 现 `run_lir_stage_from_lowered_hir` 的步骤 1–6，外加把 `(LateLoweredProgram, LirFacts)`、`base_context`、`materialized_mir`、`cone`(取自 `materialized_mir.stable_cone_key()`) 组装成 `LirArtifact`，`object_files` 置空。
  - 注意 `materialized_mir` 当前在步骤 5 被 `into_parts()` 消费，需调整所有权使其能同时进 `base_context` 与 `LirArtifact.mir`（`MaterializedMir` 必要时 `clone`，过渡期可接受）。
- 验收：`build_lir_artifact` 编译通过、可单测产出 `LirArtifact`。

### [TODO] T1-03：依赖 handoff → `LirArtifact` 适配
- 新增 `fn lir_artifact_from_dep(dep: CachedDepArtifactHandoff) -> LirArtifact`：`cone = stable_cone_key`、`program = lir`、`facts = lir_facts`、`object_files = object_files`；`base_context` 由 `type_store` 重建（复用现有依赖 base-context 构建路径；定位 `handoff.rs` 中消费 dep 的现有逻辑并改为产出 `LlvmStageBaseContext`），`mir` 对依赖不需要（设计上依赖 LIR 不应再 overlay 回 MIR——若现状仍需，记为 P2 风险并临时携带空/占位）。
- 验收：依赖 cone 能转成 `LirArtifact`。

### [TODO] T1-04：codegen `run` 改吃 `CodegenInput`
- 改 `fn run` 签名为 `run(session, input: CodegenInput)`；删除其内部对 `run_lir_stage_from_lowered_hir` 的调用（已移至 T1-02）。
- emit 段（现 :399 之后）改为消费 `input.program`（`LirArtifact`）的 `program/facts/base_context/mir` + `input.deps` + `input.abi_shell`，逻辑等价于现有「`(lir, lir_facts) + base_context + cached_dep_artifacts`」。
- 删除 `LlvmCodegenStageInput`、`LlvmLirRun`、`into_abi_visibility_parts`、`run_lir_stage_from_lowered_hir`（其职责已拆到 T1-02 / `CodegenInput`）。
- 验收：codegen 阶段源码中**不再出现** `lowered_hir`/`frontend_index`/`type_env`/`CodegenLoweringOutput`。

### [TODO] T1-05：调用方串新阶段
- `pipeline/mod.rs:183` `run_llvm_codegen_stage` 及其调用点（:226 / :446）、`single_cone.rs:165`：在调 codegen 前先 `build_lir_artifact`（主 + 可选 abi_shell）、把 `cached_dep_artifacts` 经 `lir_artifact_from_dep` 转成 `deps`，组装 `CodegenInput` 再调 `run`。
- 验收：`cargo build -p scoop -p scoopc` 通过；端到端可编译运行。

### [TODO] T1-06：`entry` 改为解析引用
- 定义 `EntryRef`：在 LIR 阶段把 `entry_source_id + entry_main_fqn` 解析成对 `LirArtifact.program` 内某 callable 的引用（过渡期可用 `StableLirCallableKey`；P2 换 `LirCallableId`）。解析失败 = 错误（缺入口是合法输入错误，应在此报，而非后端 panic）。
- 把 `CodegenInput.entry` 占位换成 `EntryRef`；codegen emit 用它而非字符串 fqn 比对。
- 验收：codegen 中入口选择不再用 `entry_main_fqn` 字符串扫描。

### [TODO] T1-07：验证与收口
- 跑 §4 全部基线（fmt / clippy -D warnings / test --all / build / dependency_gate / spec_fixtures / run_fixtures，run_fixtures 约 1664 checks 全过）。
- 自检：`grep -n` 确认 `llvm_codegen_stage.rs` 无 `lowered_hir|materialized_mir|frontend_index|type_env`；`CachedDepArtifactHandoff` 仅在 T1-03 适配处出现。
- 完成记录：（待填，附验证结果）。

## 4. 验证基线（每任务收尾）

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all --all-targets
cargo build -p scoop -p scoopc
python3 tools/dependency_gate.py
python3 tools/spec_fixtures.py check
python3 tools/run_fixtures.py
```

## 5. 风险 / 备注

- `MaterializedMir` 过渡期在 `LirArtifact` 内被携带（overlay 后备），可能引发 clone 开销；可接受，P2 lift 指令后移除。
- 依赖 cone 的 `base_context` 重建路径若与主 cone 不同，T1-03 需对齐现有依赖消费逻辑（先定位 `handoff.rs` 里消费 `CachedDepArtifactHandoff` 的现有代码，照其所需字段产出 `LlvmStageBaseContext`）。
- 若发现 codegen emit 仍在某处直接读 `Index`/`TypeEnv`（§3.3 残留漏），记录具体位置，本阶段把该需求改为从 `base_context` 取；确无法满足的，登记为 P2 阻塞而非临时回看。
