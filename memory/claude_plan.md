# 当前执行计划

1. 先读取 `TODO.md`，确认它引用了哪些详细任务文件。
2. 按任务顺序读取相关 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务，并记录其约束、依赖、验证要求。
3. 检查最新提交是否直接提到与该任务相关且未完成的问题；如果该问题会阻塞当前任务，将其视为当前任务的一部分或按要求补入前置任务。
4. 在不偏离规范、不采用变通方案的前提下，直接实现当前任务；如遇真实阻塞，最小化新增前置任务并同步 `TODO.md`。
5. 运行与当前任务直接相关的测试、格式化、lint 与必要的构建验证，修复出现的问题，直到结果稳定。
6. 更新 `memory/claude_plan.md` 记录关键进展；在任务完成后更新对应 `TODO-Px.md` 的完成记录与 `[DONE]` 标记，并在需要时同步 `TODO.md` 与 `PLAN.md`。
7. 按仓库提交风格创建一次原子提交，只完成当前这一个任务后停止。

## 记录原则

- 这里记录执行计划、关键决策、阻塞点和完成情况，不记录冗长的内部推理。
- 如果实施过程中发现计划需要调整，会在本文件追加更新。

## 进展更新

- 已读取 `TODO.md` 与 `TODO-P6.md`，当前首个未完成详细任务为 `P6-T01`。
- 已确认最新提交仅完成 `P5-T07R`，没有额外记录会直接阻塞 `P6-T01` 的未完问题。
- 现状确认：`crates/scoop/src/commands/build.rs` 虽然已经经由 `effect_refactor_pipeline::emit_production_llvm_artifact_to_file(...)` 分发，但该 helper 在 refactor 模式下仍直接调用 legacy `llvm::emit_minimal_main_*_from_production_lowered_hir*`；这正是 `P6-T01` 要切断的回落路径。
- 当前实现里的可复用点：
  - build frontend 已能产出带 canonical materialized MIR/pass-view 的 `hir::LoweredHir`；
  - `effect_refactor_pipeline` 已具备 typed HIR、direct-style MIR、effect facts、late lowering 各阶段输出；
  - LLVM backend 已能消费 canonical pass-view，但入口仍包装在旧的 `production_lowered_hir` emit API 里。
- 当前决定的实现方向：
  - 新增 `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`，作为 refactor LLVM 显式阶段入口；
  - 在该 stage 内显式串起 `TypedHirStageOutput -> RefactorMirStageOutput -> RefactorEffectFactsStageOutput -> RefactorEffectLoweredStageOutput`；
  - 新增 refactor 专属 LLVM emit API，并让 refactor build/run/fixtures 统一走这条新 stage；
  - 为避免继续依赖 legacy `production_lowered_hir` emit helper，stage 会把 build frontend 的 HIR 产物拆成“HIR compatibility scaffold（仅保留现有非 effect codegen 所需 side tables）”与“P5 late-lowered stage output”，再由新的 refactor emit API 同时消费这两部分。
- 已完成实现：
  - 新增 `crates/scoopc/src/effect_refactor_pipeline/llvm_codegen_stage.rs`；
  - 新增 refactor emit API：`emit_refactor_main_{ir,obj,asm}_to_file_from_stage_output(...)`；
  - `build.rs` / `run.rs` / fixtures 经由 `emit_production_llvm_artifact_to_file(...)` 的 refactor 路径现在会先进入新 stage，再进入新的 emit API；legacy 路径保持原样。
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_codegen_stage`
  - `cargo run -p scoop -- --effect-pipeline legacy build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_legacy_emit.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-llvm tests/fixtures/build/emit_llvm_basic.scoop -o /tmp/p6_refactor_emit.ll`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-obj tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.o`
  - `cargo run -p scoop -- --effect-pipeline refactor build --emit-asm tests/fixtures/run-pass/minimal_main.scoop -o /tmp/p6_refactor_emit.s`
  - `cargo run -p scoop -- --effect-pipeline refactor run tests/fixtures/run-pass/minimal_main.scoop`
  - `cargo test -p scoop build_emit_llvm_writes_ll_file`
  - `cargo test -p scoop run_builds_and_executes_minimal_main`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
