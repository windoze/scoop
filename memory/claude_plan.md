# 当前执行计划

说明：本文件记录可审计的执行计划、关键决策和进度更新，不包含隐藏推理过程。

## 约束

- 只处理 `TODO.md` 中第一个未完成任务；完成后停止。
- `TODO.md` 是任务顺序、验收和完成记录的权威来源。
- 不把隐藏推理写入文件；本文件记录可公开的计划、决策和进度。

## 初始步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务。
2. 如最新提交明确提到与该任务直接相关的未完成问题，纳入当前任务或在 `TODO.md` 中补为前置任务。
3. 读取任务涉及的代码、测试、文档和完成要求。
4. 若没有必须前置的阻塞问题，按任务要求实现最小正确改动。
5. 按要求运行格式化、lint、相关测试和必要的完整验证。
6. 更新 `TODO.md` 的任务标题与完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
7. 检查改动并提交一次清晰的 Git commit，然后停止。

## 当前记录：T1-06-R

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 检查最近提交是否明确留下与当前 review 任务直接相关的未完成事项。
3. 对照 T1-06-R 清单复核入口解析是否发生在 LIR/codegen 边界，且失败为干净诊断。
4. 确认 LLVM emit/main wrapper 不再按 `entry_main_fqn` 扫描入口，改为消费 `EntryRef` 的 stable callable key。
5. 确认默认 `main`、显式 entry override、入口不存在三类路径均有验证覆盖。
6. 运行格式化、lint、构建、相关单测、入口 fixture、dependency gate 和 spec fixture check；若无代码变更，复用 T1-06 的全量 test/fixture 绿色基线。
7. 成功后更新 `TODO.md`，给 `T1-06-R` 标题加 `[DONE]` 并填写完成记录。
8. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

当前进度：

- 已确认第一个未完成任务为 `T1-06-R：Review T1-06`。
- 最新提交 `544090d5 [T1-06] Resolve codegen entry refs` 是本 review 的直接对象，未在提交摘要中声明额外未完成事项。
- 已复核 `pipeline/lir_artifact.rs`、`pipeline/mod.rs`、`llvm_codegen_stage.rs`、LLVM `handoff.rs`、`emit.rs` 和 `main_entry.rs`，确认入口解析在主 `LirArtifact` 构建后、进入 LLVM emit 前完成。
- 已确认 `resolve_entry_ref` 用 `StableLirCallableKey` 校验入口落到 primary LIR program 的 callable body；缺显式入口返回带入口名的 `LlvmEmitError::Frontend`，缺默认入口返回 `MissingEntryMain`，未发现 panic/unwrap 路径。
- 已确认 LLVM emit/main wrapper 通过 `EntryRef.callable()` 查找 LIR body；`entry_main_fqn` 在 `crates/scoopc_codegen_llvm/` 与 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 零命中，`select_entry_main` 零命中。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build -p scoop -p scoopc`；`cargo test -p scoopc --all-targets build_llvm_codegen_input_`（3 passed）；`python3 tools/run_fixtures.py tests/fixtures/run-pass/minimal_main.scoop --exit-on-failure`（fixtures: ok 1）；`python3 tools/run_fixtures.py tests/fixtures/run_pass_cone/entry_package_selects_correct_main --exit-on-failure`（fixtures: ok 1）；`python3 tools/run_fixtures.py tests/fixtures/run_pass_cone/entry_package_missing_main_is_error --exit-on-failure`（fixtures: ok 1）；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`。
- 全量 `cargo test --all --all-targets` 与全量 `python3 tools/run_fixtures.py` 未重跑：本 review 未修改代码文件，复用 T1-06 完成记录中的绿色全量基线（run_fixtures ok 1664）。
- 已更新 `TODO.md`，将 `T1-06-R` 标记为 `[DONE]` 并填写完成记录。
- 提交前检查发现本文件曾覆盖旧记录，已恢复历史记录并把 T1-06-R 记录置顶。

## 先前记录：T1-06

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 检查最近提交是否明确留下与当前任务直接相关的未完成事项。
3. 定位现有 `entry_source_id` / `entry_main_fqn` 到 LLVM codegen 的数据流，以及入口选择的字符串扫描位置。
4. 在 LIR/codegen handoff 中新增解析后的入口引用，并在 LIR artifact 构建边界解析默认 main 或显式 entry fqn。
5. 修改 LLVM main emit 路径，使其消费解析后的 entry callable key，而不是在 codegen emit 阶段按字符串重新扫描。
6. 补充默认 main、显式 entry、入口不存在诊断测试。
7. 按顺序运行格式化、lint、完整 Rust 测试、build、dependency gate、spec fixture check 和完整 fixture suite。
8. 成功后更新 `TODO.md`，给 `T1-06` 标题加 `[DONE]` 并填写完成记录。
9. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

先前进度：

- 已确认第一个未完成任务为 `T1-06：entry 改为解析引用`。
- 最新提交 `bf74da2b [T1-05-R] Review codegen caller wiring` 未明确指出与 T1-06 直接相关的未完成阻塞。
- 已定位现有入口选择：`scoopc_codegen_llvm/src/llvm/emit.rs` 通过 `entry_main_fqn` 或默认 `main` 扫描 `LirFacts.callables`，并在 codegen emit 阶段完成选择与参数形态分类。
- 已完成代码修改：`EntryRef` / `EntryMainArgShape` 放入 LLVM handoff；入口解析前移到 `build_llvm_codegen_input`；main emit 改为接收 `EntryRef`，并通过 stable callable key 查找 LIR body；补充默认 main、显式 entry、显式入口缺失的单测。
- 验证已完成：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 验收 grep 已完成：`entry_main_fqn` 在 `crates/scoopc_codegen_llvm/` 与 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 零命中；旧 emit 入口字符串扫描 helper 零命中。
- 已更新 `TODO.md`，将 `T1-06` 标记为 `[DONE]` 并填写完成记录。
- 提交前检查发现本文件曾覆盖旧记录，已恢复历史记录并把 T1-06 记录置顶。

## 先前记录：T1-05-R

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 检查最近提交是否明确留下与当前 review 任务直接相关的未完成事项。
3. 对照 T1-05-R 清单复核三个调用点是否都先构建 LIR artifact，再组装 `CodegenInput` 调用 codegen。
4. 确认 cached dep 经 `lir_artifact_from_dep` 统一进入 `deps`，旧 `LlvmCodegenStageInput` 无残留，错误传播不被吞掉。
5. 运行格式化、lint、构建、单 cone fixture、多 cone fixture、dependency gate 和 spec fixture check；若无代码变更，复用 T1-05 的全量 test/fixture 绿色基线。
6. 成功后更新 `TODO.md`，给 `T1-05-R` 标题加 `[DONE]` 并填写完成记录。
7. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

先前进度：

- 已确认第一个未完成任务为 `T1-05-R：Review T1-05`。
- 最近提交 `3b2f14a5 [T1-05] Wire LIR artifacts into codegen callers` 是本 review 的直接对象，未在提交摘要中声明额外未完成事项。
- 已复核 `pipeline/mod.rs`、`single_cone.rs`、`lir_stage.rs`、`lir_artifact.rs` 和 `llvm_codegen_stage.rs`，确认三个调用路径均先组装 `CodegenInput` 再进入 codegen。
- 已确认主 cone 与可选 ABI shell 通过 `build_lir_artifact` 构建，cached dep 通过 `lir_artifact_from_dep` 进入 `deps`，相关错误均通过 `?` 传播为 `LlvmEmitError`。
- 验收搜索通过：`LlvmCodegenStageInput` 在 `crates/` 中无残留。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo build -p scoop -p scoopc`；`python3 tools/run_fixtures.py tests/fixtures/run-pass/minimal_main.scoop --exit-on-failure`（fixtures: ok 1）；`python3 tools/run_fixtures.py tests/fixtures/run_pass_cone/source_path_dependency_public_call --exit-on-failure`（fixtures: ok 1）；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`。
- 提交前确认无代码文件变更；全量 `cargo test --all --all-targets` 与全量 `python3 tools/run_fixtures.py` 复用 T1-05 的绿色基线，因为本 review 仅修改任务记录和进度文档。
- 已更新 `TODO.md`，将 `T1-05-R` 标记为 `[DONE]` 并填写完成记录。
- 提交前检查发现本文件曾覆盖旧记录，已恢复历史记录并把 T1-05-R 记录置顶。

## 先前记录：T1-05

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 检查最新提交是否明确留下与当前任务直接相关的未完成事项。
3. 对照 T1-05 清单复核 `pipeline/mod.rs` 与 `single_cone.rs` 的调用路径。
4. 确认调用方在 codegen 前先构建主 cone / ABI shell `LirArtifact`，并把 cached deps 转为统一的 `deps`。
5. 确认旧 `LlvmCodegenStageInput` 构造无残留，错误传播不被吞掉。
6. 按顺序运行格式化、lint、完整 Rust 测试、build、dependency gate、spec fixture check 和完整 fixture suite。
7. 成功后更新 `TODO.md`，给 `T1-05` 标题加 `[DONE]` 并填写完成记录。
8. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

先前进度：

- 已确认第一个未完成任务为 `T1-05：调用方串新阶段`。
- 最近提交 `c35ca3b7 [T1-04-R] Review codegen LIR artifact handoff` 未在提交摘要中声明与 T1-05 直接相关的未完成问题。
- 已确认 `pipeline::build_llvm_codegen_input` 在 codegen 前构建主 cone / ABI shell `LirArtifact`，并把 `cached_dep_artifacts` 经 `lir_artifact_from_dep` 转为统一 `deps`。
- 已确认 `pipeline/mod.rs` 的 single-file 与 production artifact 路径、`single_cone.rs` 的 artifact compile 路径均先组装 `CodegenInput` 再调用 `run_llvm_codegen_stage` / `llvm_codegen_stage::emit_artifact_to_file`。
- 验收搜索通过：`LlvmCodegenStageInput` 无残留；`llvm_codegen_stage.rs` 对 `lowered_hir|frontend_index|type_env|CodegenLoweringOutput` 零命中。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 `T1-05` 标记为 `[DONE]` 并填写完成记录。
- 提交前检查发现本文件曾覆盖旧记录，已恢复历史记录并把 T1-05 记录置顶。

## 先前记录：T1-04-R

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 检查最近提交是否明确留下与当前 review 任务直接相关的未完成事项。
3. 对照 T1-04-R 清单复核 `llvm_codegen_stage::run` 的 `CodegenInput` 消费路径、ABI shell 路径和 dependency handoff 路径。
4. 确认旧输入类型和旧 helper 删除彻底，且没有用 dead-code allow 掩盖残留。
5. 确认 `llvm_codegen_stage.rs` 不再包含前端 lowering 相关字段或类型名。
6. 按顺序运行格式化、lint、完整 Rust 测试、build、dependency gate、spec fixture check 和完整 fixture suite。
7. 用临时 worktree 对比 T1-04 前后同一 fixture 生成的 LLVM IR。
8. 成功后更新 `TODO.md`，给 `T1-04-R` 标题加 `[DONE]` 并填写完成记录。
9. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

先前进度：

- 已确认第一个未完成任务为 `T1-04-R：Review T1-04（本阶段核心 review）`。
- 最近提交 `f8beb084 [T1-04] Feed codegen from LIR artifacts` 未在提交正文中声明未完成事项。
- 已按 T1-04-R 清单检查 `llvm_codegen_stage.rs`、`lir_stage.rs`、`lir_artifact.rs`、`pipeline/mod.rs`、`single_cone.rs` 与 LLVM handoff/emit 路径；未发现旧输入类型、旧 helper、dead-code allow 或 codegen 阶段前端字段残留。
- 已完成验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py` 全部通过。
- 已用临时 worktree 对比 T1-04 前后 `tests/fixtures/build/emit_llvm_basic.scoop` 的 LLVM IR，`diff -u` 无差异，SHA-256 均为 `7d8aea309a3754ead6bea4d74d127c9be8b1e3a940bb8caaea3caa02524c0523`。
- 已更新 `TODO.md`，将 `T1-04-R` 标记为 `[DONE]` 并填写完成记录。
- 提交前检查发现本文件曾覆盖旧记录，已恢复历史记录并把 T1-04-R 记录置顶。

## 先前记录：T1-04

- 2026-06-04：已创建执行计划文件，下一步读取 `TODO.md` 定位第一个未完成任务。
- 2026-06-04：已定位第一个未完成任务为 `T1-04：codegen run 改吃 CodegenInput`；最新提交为 `T1-03-R`，工作区仅有本计划文件改动。
- 2026-06-04：已将 LIR artifact 构建逻辑迁出 `llvm_codegen_stage.rs`，删除旧 `LlvmCodegenStageInput`，并把 codegen `run` 改为接收 `CodegenInput`。
- 2026-06-04：验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（fixtures: ok 1664）。`TODO.md` 已将 T1-04 标记为 `[DONE]` 并记录完成情况。

## 更早记录：T1-03-R

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 仅检查与所选任务直接相关的最近提交信息，不做开放式历史问题排查。
3. 复核 `T1-03` 的 cached dependency handoff 到 `LirArtifact` 的适配实现。
4. 对照现有 `CachedDepArtifactHandoff` 消费路径，确认 dependency `base_context` 重建没有引入 recompute/fallback。
5. 确认 dependency artifact 不携带 MIR overlay，也没有静默放入占位 MIR。
6. 确认 `cone/program/facts/object_files` 映射无丢失。
7. 用带依赖 cone 的 fixture 验证编译、链接、运行路径保持既有行为。
8. 按顺序运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、完整 Rust 测试、build、dependency gate、spec fixtures 和完整 fixture suite。
9. 成功后更新 `TODO.md`，给 `T1-03-R` 标题加 `[DONE]` 并填写完成记录。
10. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

更早进度：

- 已确认第一个未完成任务为 `T1-03-R：Review T1-03`。
- 最近提交 `f69bbb88 [T1-03] Adapt cached deps to LIR artifacts` 是本 review 的直接对象，未显示额外未完成前置项。
- 已复核 `lir_artifact_from_dep` 与 `LlvmStageBaseContext::from_cached_dep_type_store`：cached dependency handoff 的 `cone/program/facts/object_files` 直接映射进 `LirArtifact`，base context 只从现有 cached dep `TypeStore` 与 `LirFacts` owner/fingerprint 契约重建。
- 已确认 dependency artifact 的 `mir` 明确为 `None`，没有静默塞占位 MIR；现有 cached dep ABI materialization 仍只消费 LIR/facts/type store/object files。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`python3 tools/run_fixtures.py tests/fixtures/run_pass_cone/source_path_dependency_public_call --exit-on-failure`（fixtures: ok 1）；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 `T1-03-R` 标记为 `[DONE]` 并填写完成记录。
- 提交前检查发现本文件曾覆盖旧记录，已恢复历史记录并把 T1-03-R 记录置顶。

## 更早记录：T1-03

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 仅检查与所选任务直接相关的最近提交信息，不做开放式历史问题排查。
3. 定位 `CachedDepArtifactHandoff` 的现有消费路径、`LlvmStageBaseContext` 构造方式，以及 `LirArtifact` 的过渡字段约束。
4. 新增 cached dep handoff 到 `LirArtifact` 的适配函数，直接映射 `cone/program/facts/object_files`。
5. 通过 cached dep `TypeStore` 与 `LirFacts` 重建依赖 cone 的窄 `LlvmStageBaseContext`，不重算、不回退到前端结构。
6. 明确 dependency cone 不携带 MIR overlay；如现状需要 MIR，则记录风险而不是静默占位。
7. 添加单测验证依赖 cone 能转换为 `LirArtifact`，且 object files、type-context 校验和 MIR 缺席语义正确。
8. 按顺序运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、完整 Rust 测试、build、dependency gate、spec fixtures 和完整 fixture suite。
9. 成功后更新 `TODO.md`，给 `T1-03` 标题加 `[DONE]` 并填写完成记录。
10. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

更早进度：

- 已确认第一个未完成任务为 `T1-03：依赖 handoff → LirArtifact 适配`。
- 最近提交 `bfcc25ea [T1-02-R] Review LIR artifact builder` 未显示需要优先处理的 T1-03 直接未完成问题。
- 已定位现有 cached dep 消费路径：LLVM ABI materialization 目前消费 `CachedDepArtifactHandoff` 的 `stable_cone_key/lir/lir_facts/type_store/object_files`，依赖 LIR 不 overlay 回 MIR。
- 已完成核心实现：`LirArtifact.mir` 改为 `Option<MaterializedMir>`，主 cone 保留 `Some`，cached dep 明确为 `None`；新增公开适配函数 `lir_artifact_from_dep`；`LlvmStageBaseContext` 新增从 cached dep `TypeStore` 与 LIR facts 重建最小 base context 的入口。
- 第一次 `cargo clippy --all-targets -- -D warnings` 失败于 `lir_artifact_from_dep` 未接入生产路径导致的 `dead_code`；按 T1-03/T1-05 顺序，将该函数改为公开的 LIR handoff API，而不是加 `allow` 或提前改调用方。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 `T1-03` 标记为 `[DONE]` 并填写完成记录。
- 提交前检查发现本文件曾覆盖旧记录，已恢复历史记录并把 T1-03 记录置顶。

## 更早记录：T1-02-R

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 仅检查与所选任务直接相关的最近提交信息，不做开放式历史问题排查。
3. 复核 `T1-02` 的实现是否忠实搬迁原 `run_lir_stage_from_lowered_hir` 步骤 1-6。
4. 审查 `MaterializedMir` 所有权流向，确认无重复构建、无额外 clone，且同一份 MIR 能同时支撑 base context 与 `LirArtifact.mir`。
5. 确认 `cone`、`object_files`、`verify_lir_type_context(..., "primary")` 与 `abi_visibility` 的 `preserve_published_resume_shells=true` 路径符合任务要求。
6. 用一个 fixture 对比 `HEAD^` 与当前版本产出的 LLVM IR，确认重构前后等价。
7. 按顺序运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、完整 Rust 测试、build、dependency gate、spec fixtures 和完整 fixture suite。
8. 成功后更新 `TODO.md`，给 `T1-02-R` 标题加 `[DONE]` 并填写完成记录。
9. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

更早进度：

- 已确认第一个未完成任务为 `T1-02-R：Review T1-02`。
- 最近提交 `fb12b99e [T1-02] Extract LIR artifact builder` 是本 review 的直接对象，未提到额外未完成前置项。
- 已逐项复核 `build_lir_artifact`、`LirArtifact` 拆包使用点和 `abi_visibility` 路径，未发现需要代码修正的问题。
- 已确认 `MaterializedMir` 未重复构建且未额外 clone；实现先借给 base context，再移动进 `LirArtifact.mir`。
- 已确认 `cone` 取自 `materialized_mir.stable_cone_key()`、`object_files` 为空、`verify_lir_type_context(..., "primary")` 保留，ABI visibility 仍走 `preserve_published_resume_shells=true` 并保留 ABI visibility type-context 校验。
- 已完成抽样 IR 对比：`tests/fixtures/umb_fix/P5-T02-immortal/pos_string_literal_immortal_ir.scoop` 在 `HEAD^` 与当前版本的 LLVM IR `diff -u` 无差异，SHA-256 均为 `529ed3daec679f67f965836ed47b12723d3a6762a01ea10f855f970e0b25ffc1`。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 `T1-02-R` 标记为 `[DONE]` 并填写完成记录。

## 更早记录：T1-02

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 仅检查与所选任务直接相关的最近提交信息，不做开放式历史问题排查。
3. 阅读 `T1-02` 的任务要求，确认需抽出 `build_lir_artifact` 且保持现有 codegen 行为等价。
4. 对照 `llvm_codegen_stage.rs` 当前 LIR 准备流水线和 `LirArtifact` 定义，做最小重构。
5. 新增 `pub(crate) fn build_lir_artifact(...) -> Result<LirArtifact, LlvmEmitError>`，复用原步骤 1-6，组装 `cone/program/facts/base_context/mir/object_files`。
6. 调整 `MaterializedMir` 所有权，使其构建 `LlvmStageBaseContext` 时只被借用，随后移动进 `LirArtifact`，避免重复构建和不必要 clone。
7. 让当前 `run` 路径通过 `build_lir_artifact` 产物拆回既有 `LlvmCodegenStageOutput`，不改变 emit 行为。
8. 添加单测验证 `build_lir_artifact` 能产出自包含 `LirArtifact`。
9. 按顺序运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、完整 Rust 测试、build、dependency gate、spec fixtures 和完整 fixture suite。
10. 成功后更新 `TODO.md`，给 `T1-02` 标题加 `[DONE]` 并填写完成记录。
11. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

更早进度：

- 已确认第一个未完成任务为 `T1-02：抽出独立 LIR 阶段函数 build_lir_artifact`。
- 最近提交 `d329bf5b [T1-01-R] Review LIR artifact handoff types` 是已完成 review，不包含与 `T1-02` 直接相关的未完成前置项。
- 已实现 `build_lir_artifact`，其复用原 LIR 准备流水线并组装 `LirArtifact`。
- 已将 `build_llvm_stage_base_context_from_lowered_hir` 改为借用 `MaterializedMir`，随后将同一份 MIR 移入 `LirArtifact.mir`。
- 已让当前 `run` 路径消费 `build_lir_artifact` 结果后拆回既有 stage output，保持后续 emit 输入等价。
- 已新增 `build_lir_artifact_produces_self_contained_handoff` 单测，验证 cone、base context、空 object files、callable payload 与 type-context 一致性。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 `T1-02` 标记为 `[DONE]` 并填写完成记录。
- 提交前检查已完成；下一步暂存本任务文件并创建提交。

## 更早记录：T1-01-R

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 选择第一个未完成任务。
2. 仅检查与所选任务直接相关的最近提交信息，不做开放式历史问题排查。
3. 复核 `T1-01` 新增的 `LirArtifact` / `CodegenInput` 字段、导出、`llvm` feature 门控和零行为变化要求。
4. 如 review 发现与验收直接相关的问题，做最小正确修正。
5. 先运行 `cargo fmt`，再运行 `cargo clippy --all-targets -- -D warnings`，再按任务要求和变更范围运行构建、测试与 fixture 验证。
6. 成功后更新 `TODO.md`，给 `T1-01-R` 标题加 `[DONE]` 并填写完成记录。
7. 检查 git 状态、diff 和近期提交，提交本任务相关变更，然后停止。

更早进度：

- 已确认第一个未完成任务为 `T1-01-R：Review T1-01`。
- 最近提交 `d268ad5b Add per-task review tasks to P1 TODO` 未提到与 `T1-01-R` 直接相关的未完成实现问题。
- 已复核 `LirArtifact` / `CodegenInput` 的精确匹配使用点，确认新类型仅在定义和 `pipeline` re-export 出现，尚未进入运行路径。
- 已补充 `facts`、`mir`、`entry` 过渡字段说明，明确 P2/T1-06 的移除或替换方向。
- 验证中发现 `cargo build -p scoopc --no-default-features` 失败，根因是现有 `single_cone`、`tool_commands` 和若干 tests/helpers 无条件引用 LLVM-only API；该问题直接影响本 review 的“非 llvm 构建不破”验收，已纳入本任务修复。
- 已将 `single_cone` 模块、LLVM artifact emission helper、LLVM-only frontend helpers/tests 正确挂到 `feature = "llvm"`，并修正 no-default 下 `TypeEnv` 绑定警告。
- 验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings`；`cargo build -p scoop -p scoopc`；`cargo build -p scoopc --no-default-features`；`cargo test --all --all-targets`；`cargo test -p scoopc --all-targets --no-default-features`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 `T1-01-R` 标记为 `[DONE]` 并填写完成记录。

## 最早记录：T1-01

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判定第一个未完成任务。
2. 查看最近提交信息，只在其明确提到与当前任务直接相关的未完成事项时纳入当前任务或作为 `TODO.md` 前置项。
3. 阅读当前任务关联的规格、代码和测试，确认要求、依赖与验证命令。
4. 完整实现第一个未完成任务；如发现阻塞当前任务的缺失特性、规格不匹配或测试失败，优先修复，或在 `TODO.md` 插入最小前置任务后停止。
5. 运行格式化、lint 和相关测试；若代码发生变更，再按要求运行完整测试与 fixture 套件。
6. 更新 `TODO.md`：完成时给任务标题加 `[DONE]` 并填写 completion record；只有阶段级计划变化时才更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关全部变更，然后停止，不继续下一个任务。

最早进度：

- 已创建初始计划文件，下一步读取任务列表并定位第一个未完成任务。
- 已确认第一个未完成任务为 `T1-01：新增 LirArtifact / CodegenInput 类型`。
- 最近提交 `d5e0b0ad Pivot to structural fact refactor; archive fact-unify plan` 未明确提到与 T1-01 直接相关的未完成修复。
- 当前任务执行策略：只新增过渡类型与模块导出，保持行为不变；随后按基线运行格式化、lint、测试与 fixture 验证。
- 已新增 `crates/scoopc/src/pipeline/lir_artifact.rs`，并在 `pipeline/mod.rs` 中按 `llvm` feature 导出 `CodegenInput` 与 `LirArtifact`。
- 验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 T1-01 标记为 `[DONE]` 并填写完成记录；下一步检查 diff 并提交。
