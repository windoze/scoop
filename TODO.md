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

### [DONE] T1-01-R：Review T1-01
- **关注点**：
  - `LirArtifact`/`CodegenInput` 字段与 §2 / FACT_REFACTOR §14.3 一致；过渡字段（`mir`、`facts`）有注释标明 P2 移除；未提前引入 arena/句柄全集。
  - 类型**单一定义**（无重复 def）；`llvm` feature 门控正确，非 llvm 构建不破。
  - **零行为变化**：新类型尚未接入任何运行路径。
- **确认验证**：
  - `cargo build -p scoop -p scoopc` + `cargo clippy --all-targets -- -D warnings` 通过。
  - `grep -rn "LirArtifact\|CodegenInput" crates/` 确认目前仅定义/导出，未被 `run`/调用方消费（行为未变）。
  - 复核 T1-01 完成记录里的 fixtures 数（ok 1664）与基线一致。
- 完成记录（2026-06-03）：复核 `LirArtifact` / `CodegenInput` 字段与过渡形态一致，补充 `facts`、`mir`、`entry` 过渡字段说明；确认精确匹配 `LirArtifact` / `CodegenInput` 仅出现在类型定义与 `pipeline` re-export，尚未接入运行路径。审查中发现 no-default 构建下现有 LLVM-only 路径未完整门控，已将 `single_cone`、LLVM artifact emission helper、LLVM-only frontend helpers/tests 正确挂到 `llvm` feature，确保非 LLVM 构建不破。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`；`cargo build -p scoop -p scoopc`；`cargo build -p scoopc --no-default-features`；`cargo test --all --all-targets`；`cargo test -p scoopc --all-targets --no-default-features`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [DONE] T1-02：抽出独立 LIR 阶段函数 `build_lir_artifact`
- 在 `llvm_codegen_stage.rs`（或迁到 `pipeline/lir_stage.rs`）新增 `pub(crate) fn build_lir_artifact(session, entry_source, lowered: CodegenLoweringOutput, preserve_published_resume_shells: bool) -> Result<LirArtifact, LlvmEmitError>`：
  - 内部 = 现 `run_lir_stage_from_lowered_hir` 的步骤 1–6，外加把 `(LateLoweredProgram, LirFacts)`、`base_context`、`materialized_mir`、`cone`(取自 `materialized_mir.stable_cone_key()`) 组装成 `LirArtifact`，`object_files` 置空。
  - 注意 `materialized_mir` 当前在步骤 5 被 `into_parts()` 消费，需调整所有权使其能同时进 `base_context` 与 `LirArtifact.mir`（`MaterializedMir` 必要时 `clone`，过渡期可接受）。
- 验收：`build_lir_artifact` 编译通过、可单测产出 `LirArtifact`。
- 完成记录（2026-06-03）：在 `llvm_codegen_stage.rs` 抽出 `pub(crate) fn build_lir_artifact(...) -> Result<LirArtifact, LlvmEmitError>`，复用原 LIR 准备流水线并组装 `cone/program/facts/base_context/mir/object_files`；`MaterializedMir` 在构建 base context 时改为借用，随后移动进 `LirArtifact`，避免重复构建和额外 clone；当前 codegen `run` 改为消费该 artifact 后再拆回既有 `LlvmCodegenStageOutput`，保持行为等价；新增单测验证 `LirArtifact` 的 cone、base context、空 object files、callable payload 与 type-context 一致性。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [DONE] T1-02-R：Review T1-02
- **关注点**：
  - `build_lir_artifact` 是现 `run_lir_stage_from_lowered_hir` 步骤 1–6 的**忠实搬迁**——逻辑零改动，仅重新打包成 `LirArtifact`（逐行对照 :293 原实现）。
  - `materialized_mir` 所有权：现同时进 `base_context` 与 `LirArtifact.mir`；确认无**重复构建**、clone 仅一次且语义不变。
  - `cone` 取自 `materialized_mir.stable_cone_key()`；`verify_lir_type_context(..., "primary")` 仍被调用；`object_files` 置空。
  - `abi_visibility` 路径（`preserve_published_resume_shells=true`）仍能经同一函数产出，未丢分支。
- **确认验证**：
  - 全套基线（§4）绿；尤其 `python3 tools/run_fixtures.py` 仍 ok 1664（行为应与重构前**逐位等价**，因为是同一流水线换包装）。
  - 抽样 diff：对 1 个 fixture 比对重构前后产出的 LLVM IR/可执行行为应一致。
- 完成记录（2026-06-03）：逐项复核 T1-02 实现，确认 `build_lir_artifact` 忠实搬迁原 `run_lir_stage_from_lowered_hir` 步骤 1-6；`MaterializedMir` 未重复构建且未额外 clone，先借给 base context 再移动进 `LirArtifact.mir`；`cone` 取自 `materialized_mir.stable_cone_key()`，`object_files` 为空，`verify_lir_type_context(..., "primary")` 保留；`abi_visibility` 通过 `preserve_published_resume_shells=true` 走同一构建函数并保留 ABI visibility type-context 校验。抽样对比 `tests/fixtures/umb_fix/P5-T02-immortal/pos_string_literal_immortal_ir.scoop`，用 `HEAD^` 与当前版本分别 emit LLVM IR，`diff -u` 无差异，SHA-256 均为 `529ed3daec679f67f965836ed47b12723d3a6762a01ea10f855f970e0b25ffc1`。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [DONE] T1-03：依赖 handoff → `LirArtifact` 适配
- 新增 `fn lir_artifact_from_dep(dep: CachedDepArtifactHandoff) -> Result<LirArtifact, LlvmEmitError>`：`cone = stable_cone_key`、`program = lir`、`facts = lir_facts`、`object_files = object_files`；`base_context` 由 `type_store` 重建（复用现有依赖 base-context 构建路径；定位 `handoff.rs` 中消费 dep 的现有逻辑并改为产出 `LlvmStageBaseContext`），`mir` 对依赖不需要（设计上依赖 LIR 不应再 overlay 回 MIR——若现状仍需，记为 P2 风险并临时携带空/占位）。
- 验收：依赖 cone 能转成 `LirArtifact`。
- 完成记录（2026-06-04）：新增公开适配函数 `pipeline::lir_artifact::lir_artifact_from_dep(...) -> Result<LirArtifact, LlvmEmitError>`，将 cached dependency handoff 的 `cone/program/facts/object_files` 直接映射进 `LirArtifact`；`LlvmStageBaseContext::from_cached_dep_type_store(...)` 复用现有 cached dep 消费所需的 `TypeStore` + `LirFacts` owner 校验来重建依赖 base context；`LirArtifact.mir` 改为 `Option<MaterializedMir>`，主 cone 为 `Some`，cached dep 明确为 `None`，未静默放入占位 MIR。新增单测覆盖 cached dep handoff 到 `LirArtifact` 的转换、object files 保留、base context type-context 校验和 dep MIR 缺席。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [DONE] T1-03-R：Review T1-03
- **关注点**：
  - 依赖 `base_context` 重建**复用现有 dep 消费路径**，未引入新的 recompute/fallback（对照 `handoff.rs` 中原消费 `CachedDepArtifactHandoff` 的逻辑）。
  - `mir` 字段对依赖的处置：确认依赖 LIR **不再 overlay 回 MIR**；若现状仍需 MIR，必须**显式登记为 P2 风险**（注释 + 备注），**不得**静默塞占位掩盖。
  - `cone/program/facts/object_files` 字段映射正确无丢。
- **确认验证**：
  - 用一个**带依赖 cone** 的 fixture（多 cone 编译）走通：编译 + 链接 + 运行结果与重构前一致。
  - `cargo test --all` 中跨 cone/dep 相关用例绿。
- 完成记录（2026-06-04）：复核 `lir_artifact_from_dep` 与 `LlvmStageBaseContext::from_cached_dep_type_store`，确认 cached dependency handoff 的 `cone/program/facts/object_files` 字段直接映射到 `LirArtifact`，依赖 base context 只从现有 cached dep `TypeStore` + `LirFacts` owner/fingerprint 契约重建，未引入 recompute/fallback；依赖 artifact 的 `mir` 明确为 `None`，没有静默塞占位 MIR，现有 cached dep ABI materialization 仍只消费 LIR/facts/type store/object files。带依赖 cone fixture `tests/fixtures/run_pass_cone/source_path_dependency_public_call` 编译、链接、运行通过并输出既有期望 `42`。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`python3 tools/run_fixtures.py tests/fixtures/run_pass_cone/source_path_dependency_public_call --exit-on-failure`（fixtures: ok 1）；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [DONE] T1-04：codegen `run` 改吃 `CodegenInput`
- 改 `fn run` 签名为 `run(session, input: CodegenInput)`；删除其内部对 `run_lir_stage_from_lowered_hir` 的调用（已移至 T1-02）。
- emit 段（现 :399 之后）改为消费 `input.program`（`LirArtifact`）的 `program/facts/base_context/mir` + `input.deps` + `input.abi_shell`，逻辑等价于现有「`(lir, lir_facts) + base_context + cached_dep_artifacts`」。
- 删除 `LlvmCodegenStageInput`、`LlvmLirRun`、`into_abi_visibility_parts`、`run_lir_stage_from_lowered_hir`（其职责已拆到 T1-02 / `CodegenInput`）。
- 验收：codegen 阶段源码中**不再出现** `lowered_hir`/`frontend_index`/`type_env`/`CodegenLoweringOutput`。
- 完成记录（2026-06-04）：将 LIR artifact 构建逻辑从 `llvm_codegen_stage.rs` 迁至独立 `pipeline/lir_stage.rs`，`llvm_codegen_stage::run` 改为直接消费 `CodegenInput` 的 `program` / `abi_shell` / `deps` / `entry` / `source_map` / `opt_level`；删除旧 `LlvmCodegenStageInput`，并移除 codegen stage 内部对 frontend lowering / HIR / MIR materialization 输入的依赖。依赖 artifact 在进入 LLVM stage output 前转为 codegen-owned `LlvmDepLirArtifactHandoff`，避免从 `LirArtifact` 回造旧 cached-dep handoff。为保持构建闭环，`pipeline::build_llvm_codegen_input` 负责在 codegen 前构建主 cone / ABI shell LIR artifact 并适配 cached deps；`pipeline` 与 `single_cone` 调用点已改为组装 `CodegenInput` 后调用 codegen。验收 grep 通过：`llvm_codegen_stage.rs` 对 `lowered_hir|materialized_mir|frontend_index|type_env|CodegenLoweringOutput` 零命中，仓库内对 `LlvmCodegenStageInput|LlvmLirRun|into_abi_visibility_parts|run_lir_stage_from_lowered_hir` 零命中。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [DONE] T1-04-R：Review T1-04（本阶段核心 review）
- **关注点**：
  - emit 行为与重构前**等价**：`input.program` 的 `program/facts/base_context/mir` + `deps` + `abi_shell` 喂入路径，对应原 `(lir, lir_facts)+base_context+cached_dep_artifacts`，无遗漏分支（尤其 abi_shell ↔ 原 abi_visibility_lowered）。
  - **删除彻底**：`LlvmCodegenStageInput`、`LlvmLirRun`、`into_abi_visibility_parts`、`run_lir_stage_from_lowered_hir` 全部移除，无 dead code / 无 `#[allow(dead_code)]` 掩盖。
  - **斩断前端漏（§3.3）**：这是本阶段最关键验收。
- **确认验证**：
  - `grep -nE "lowered_hir|materialized_mir|frontend_index|type_env|CodegenLoweringOutput" crates/scoopc/src/pipeline/llvm_codegen_stage.rs` **零命中**。
  - 全套基线绿；run_fixtures ok 1664。
  - 抽样 diff LLVM IR：codegen 改造前后对同一输入产出一致。
- 完成记录（2026-06-04）：复核 T1-04 后的 `llvm_codegen_stage::run`，确认其只消费 `CodegenInput` 中的 `program` / `abi_shell` / `deps` / `entry` / `source_map` / `opt_level`，主 cone 的 `program/facts/base_context`、ABI shell 的 `program/facts/type_store`、依赖 cone 的 `LlvmDepLirArtifactHandoff` 输入路径与 T1-04 前 emit handoff 等价；`LirArtifact.mir` 未被 codegen 重新读取。确认 `LlvmCodegenStageInput`、`LlvmLirRun`、`into_abi_visibility_parts`、`run_lir_stage_from_lowered_hir` 全部移除，`llvm_codegen_stage.rs` 无 `#[allow(dead_code)]` 掩盖；`llvm_codegen_stage.rs` 对 `lowered_hir|materialized_mir|frontend_index|type_env|CodegenLoweringOutput` 零命中，仓库内对旧 helper/type 名称零命中。抽样对比 `tests/fixtures/build/emit_llvm_basic.scoop`，用 T1-04 前提交 `ea60a3e6` 与当前版本分别 emit LLVM IR，`diff -u` 无差异，SHA-256 均为 `7d8aea309a3754ead6bea4d74d127c9be8b1e3a940bb8caaea3caa02524c0523`。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [DONE] T1-05：调用方串新阶段
- `pipeline/mod.rs:183` `run_llvm_codegen_stage` 及其调用点（:226 / :446）、`single_cone.rs:165`：在调 codegen 前先 `build_lir_artifact`（主 + 可选 abi_shell）、把 `cached_dep_artifacts` 经 `lir_artifact_from_dep` 转成 `deps`，组装 `CodegenInput` 再调 `run`。
- 验收：`cargo build -p scoop -p scoopc` 通过；端到端可编译运行。
- 完成记录（2026-06-04）：确认调用方已串接新 LIR 阶段：`pipeline::build_llvm_codegen_input` 在 codegen 前构建主 cone 与可选 ABI shell 的 `LirArtifact`，并将 `cached_dep_artifacts` 经 `lir_artifact_from_dep` 转为统一的 `deps`；`pipeline/mod.rs` 的 single-file 与 production artifact 路径、`single_cone.rs` 的 artifact compile 路径均先组装 `CodegenInput` 再调用 `run_llvm_codegen_stage` / `llvm_codegen_stage::emit_artifact_to_file`。验收 grep 通过：`LlvmCodegenStageInput` 无残留，`llvm_codegen_stage.rs` 对 `lowered_hir|frontend_index|type_env|CodegenLoweringOutput` 零命中。验证通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。

### [TODO] T1-05-R：Review T1-05
- **关注点**：
  - **三个调用点全改**：`pipeline/mod.rs:226`、`pipeline/mod.rs:446`、`single_cone.rs:165`；无残留构造旧 `LlvmCodegenStageInput` 处。
  - 顺序正确：先 `build_lir_artifact`（主 + 可选 abi_shell），dep 经 `lir_artifact_from_dep` 转 `deps`，再组 `CodegenInput` 调 `run`。
  - 错误传播路径（`LlvmEmitError`）未被吞。
- **确认验证**：
  - `grep -rn "LlvmCodegenStageInput" crates/` 仅余定义即将删（或已删）痕迹，无新构造。
  - `cargo build -p scoop -p scoopc` 通过；单 cone 与多 cone fixture 端到端均跑通。

### [TODO] T1-06：`entry` 改为解析引用
- 定义 `EntryRef`：在 LIR 阶段把 `entry_source_id + entry_main_fqn` 解析成对 `LirArtifact.program` 内某 callable 的引用（过渡期可用 `StableLirCallableKey`；P2 换 `LirCallableId`）。解析失败 = 错误（缺入口是合法输入错误，应在此报，而非后端 panic）。
- 把 `CodegenInput.entry` 占位换成 `EntryRef`；codegen emit 用它而非字符串 fqn 比对。
- 验收：codegen 中入口选择不再用 `entry_main_fqn` 字符串扫描。

### [TODO] T1-06-R：Review T1-06
- **关注点**：
  - `entry` 解析发生在 **LIR 阶段边界**；解析失败 = **干净的输入错误诊断**（缺入口是合法用户错误），**不是 panic / 不是后端 unwrap**。
  - codegen emit 不再有 `entry_main_fqn` 字符串扫描 / fqn 比对。
  - `EntryRef` 正确落到 `program.callables`：覆盖「显式 entry-point override」与「默认 `main`」两条路径。
- **确认验证**：
  - `grep -n "entry_main_fqn" crates/scoopc_codegen_llvm/ crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 在 emit 路径零命中。
  - fixture：显式 entry-point + 默认 main 均产出正确可执行；故意「入口不存在」用例**报诊断而非崩溃**。

### [TODO] T1-07：验证与收口
- 跑 §4 全部基线（fmt / clippy -D warnings / test --all / build / dependency_gate / spec_fixtures / run_fixtures，run_fixtures 约 1664 checks 全过）。
- 自检：`grep -n` 确认 `llvm_codegen_stage.rs` 无 `lowered_hir|materialized_mir|frontend_index|type_env`；`CachedDepArtifactHandoff` 仅在 T1-03 适配处出现。
- 完成记录：（待填，附验证结果）。

### [TODO] T1-07-R：Review P1（整体阶段验收）
- **关注点（对照 §0 完成标志）**：
  - `llvm_codegen_stage::run` 输入中**不再出现** `lowered_hir`/`materialized_mir`/`frontend_index`/`type_env`。
  - 主 cone 与依赖 cone **统一为 `LirArtifact`**；codegen 只 walk LIR + 诊断/入口/选项。
  - LIR 阶段已**独立于 codegen**（`build_lir_artifact` 在 codegen 之外、之前）。
  - **范围纪律**：未越界做 P2（无 arena/句柄重设计、未删 LirFacts 平表、未动 `dependency_gate.py` 黑名单）；过渡字段 `mir`/`facts` 注释清楚、登记为 P2 待移除。
  - **无新增 fallback / 字符串 live key**（不能因重构顺手引入新回退路径）。
- **确认验证**：
  - §4 全套基线绿（含 `dependency_gate.py`、`spec_fixtures.py check`、`run_fixtures.py` ok 1664）。
  - 对若干代表 fixture diff 重构前后的可执行行为/LLVM IR，确认等价。
  - 在 PLAN.md §3 把 P1 标记为 DONE，并记录遗留给 P2 的 transitional 项（`LirArtifact.mir`、`facts`、依赖 overlay 风险）。

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
