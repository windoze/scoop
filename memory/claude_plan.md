# 本次执行计划

说明：按安全与协作要求，这里记录可审阅的执行计划、关键判断与进度更新，不记录内部私有推理细节。

## 初始计划

1. 读取 `TODO.md`，确认它只是索引，并找出引用的详细任务文件。
2. 按任务顺序读取相关 `TODO-Px.md`，以标题是否带 `[DONE]` 作为唯一完成判定，定位第一个未完成的详细任务。
3. 查看最近一次提交信息，判断是否存在与该任务直接相关且明确未完成的问题；若有，则将其视为当前任务组成部分或需要写入详细 TODO 的前置依赖。
4. 阅读当前任务涉及的代码、测试、规范与约束，确认实现边界，不做开放式历史问题排查。
5. 实现该任务；若遇到阻塞当前任务且不能规避的缺口或缺陷，则在对应 `TODO-Px.md` 中补入最小前置任务，并同步 `TODO.md`。
6. 运行与该任务直接相关的验证；如有必要，再运行更广的回归验证，直到结果稳定。
7. 更新文档记录：
   - 在对应 `TODO-Px.md` 中把任务标题标记为 `[DONE]` 并补全完成记录；
   - 若任务索引、标题、顺序或状态变化，同步更新 `TODO.md`；
   - 只有在阶段计划真的变化时才更新 `PLAN.md`。
8. 检查工作区变更，按要求创建一次 git 提交，然后停止，不继续处理下一个任务。

## 进度更新规则

- 在识别出首个未完成任务后，补充任务编号、标题与相关文件。
- 在开始实现前，补充实施要点与计划验证命令。
- 若计划因真实阻塞而变化，立即在本文件中追加“计划变更”。
- 完成关键里程碑后，追加“进度更新”与“验证结果”。

## 当前任务

- 任务编号：`P6-T01R`
- 标题：`Review LLVM stage 入口，确认 refactor 路径已与 legacy production_lowered_hir / old effect backend 分离`
- 依据：`TODO.md` 中第一个未带 `[DONE]` 的索引项为 `P6-T01R`；`TODO-P6.md` 中对应条目也未完成。
- 相关详细文件：`TODO-P6.md`
- 最近提交：`[P6-T01a] Add refactor LLVM fail-fast guard`
- 当前判断：最近提交直接对应 `P6-T01R` 在完成记录中引入的前置任务 `P6-T01a`，因此本次需要在包含该修复后的代码上继续完成 review，而不是跳到后续实现任务。

## 当前执行步骤

1. 读取 `P6-T01R` 指定的关键源码位置，确认 refactor LLVM stage 的真实入口、输入形状、以及 CLI/emit 分发路径。
2. 进行定向代码搜索，确认 refactor 主实现没有继续把 `production_lowered_hir` 或旧 effect backend 当成真正的 lowering 数据源；同时区分允许存在的 legacy API/注释/测试命中。
3. 重跑 `P6-T01` 与 `P6-T01a` 要求的核心验证，确认 refactor stage 入口、非 effectful smoke、以及 effectful fail-fast 诊断仍然成立。
4. 若 review 通过：
   - 在 `TODO-P6.md` 将 `P6-T01R` 标记为 `[DONE]` 并填写完成记录；
   - 同步 `TODO.md` 中 `P6-T01R` 的状态；
   - 记录本文件的验证结果；
   - 提交本次变更并停止。
5. 若 review 不通过：
   - 保持 `P6-T01R` 未完成；
   - 只在必要时新增最小前置任务并同步 `TODO.md`；
   - 记录 blocker、提交变更并停止。

## 计划中的重点验证

- 文本搜索：`production_lowered_hir|prepare_single_file_codegen_unit|LoweredHir|effect_pipeline|refactor|legacy`
- 单测：`cargo test -p scoopc refactor_llvm_codegen_stage`
- CLI 验证：
  - `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_legacy_emit.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_refactor_emit.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-obj tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.o`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-asm tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.s`
  - `cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/minimal_main.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/effect_facts/handle_perform.scoop -o /tmp/p6_refactor_effect_fail.ll`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## 进度更新

- 已完成代码审阅，结论为：`P6-T01R` 可通过。
- 关键依据：
  - `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 在 refactor 模式下已改为进入 `llvm_codegen_stage::emit_artifact_to_file(...)`，legacy 模式才继续调用 `emit_minimal_main_*_from_production_lowered_hir_with_entry_with_opt_level(...)`。
  - `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs` 已显式推进到 `RefactorEffectLoweredStageOutput`，并产出不含旧 `materialized_pass_view` scaffold 的 `RefactorLlvmCodegenStageOutput`。
  - `crates/scoop/src/commands/build.rs` 的 `Executable` / `LlvmIr` / `Obj` / `Asm` 四条路径都共用同一 dispatcher；`crates/scoop/src/commands/run.rs` 通过复用 `build::run(...)` 共享同一路径。
  - `crates/scoopc/src/llvm/emit.rs` 中仍复用部分中立 LLVM helper，但 effectful callable 会先经过 `ensure_refactor_effect_lowering_is_supported(...)` 检查；old effect backend 已不再是 refactor correctness path。

## 验证结果

- 通过：`cargo test -p scoopc refactor_llvm_codegen_stage`
- 通过：`cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_legacy_emit.ll`
- 通过：`cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_refactor_emit.ll`
- 通过：`cargo run -p scoop -- --effect-pipeline refactor build --emit-obj tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.o`
- 通过：`cargo run -p scoop -- --effect-pipeline refactor build --emit-asm tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.s`
- 通过：`cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/minimal_main.scoop`
- 按预期失败：`cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/effect_facts/handle_perform.scoop -o /tmp/p6_refactor_effect_fail.ll`
  - 诊断：`RefactorEffectLoweringUnsupported`
  - 含义：effectful lowering 不再静默回落到 legacy handler-stack / `EffectOutcome` backend。
- 通过：`cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`

## 当前剩余步骤

1. 检查本次 diff，只包含 `TODO.md`、`TODO-P6.md`、`memory/claude_plan.md`。
2. 以 `P6-T01R` 为主题创建一次提交。
3. 停止，不进入 `P6-T02`。
