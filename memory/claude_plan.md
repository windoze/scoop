# 本轮执行计划

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始任务前，先检查最新一次提交是否提到已知问题；若提到，先修复该问题。
- 若在执行过程中发现任何既有 bug、回归、规范不一致、实现边界未完成或临时绕过逻辑，必须优先修复，或者将其作为前置任务插入 `TODO.md` 并停止。
- 不允许通过收窄范围、修改任务目标、改夹具形状、增加特判或其他 workaround 绕过问题。
- 需要在完成实现后更新 `TODO.md`、`PLAN.md`、本文件，并提交 git commit。
- 需要进行充分验证，至少覆盖相关测试，并满足无警告要求，例如 `cargo clippy --all-targets -- -D warnings`（若适用且成本合理，结合任务范围执行）。

## 初始步骤计划

1. 查看最新一次 git commit，确认提交信息和提交内容中是否提到遗留问题、已知缺陷或待补修复项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖与计划状态。
4. 评估该任务是否可在本轮完整落地。
5. 如果任务过大：
   - 将任务拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，让新的第一个子任务成为当前执行项；
   - 本轮只完成拆分后的第一个子任务。
6. 如果任务可直接执行：
   - 阅读相关代码、测试、规范或 fixture；
   - 实施修改；
   - 增补或调整测试；
   - 运行相关验证；
   - 修复验证中暴露的既有问题。
7. 完成后：
   - 更新 `TODO.md` 中对应任务状态；
   - 更新 `PLAN.md` 反映当前进度；
   - 更新本文件记录关键决策、执行结果和是否有计划调整；
   - 使用清晰的提交信息创建 commit；
   - 停止，不继续下一个任务。

## 当前认知下的执行策略

- 在未读取仓库状态前，不预设具体实现方案。
- 优先通过最小必要范围的代码阅读建立上下文，避免盲改。
- 若测试或实现暴露出当前任务依赖的基础能力缺失，则按要求先把依赖修复任务前插到 `TODO.md`，提交后停止。
- 若仓库已有未提交改动，会避免覆盖或回退非本次工作内容，只在必要文件上谨慎增量修改。

## 进度记录

- 已完成：创建本计划文件，准备开始检查最新提交与任务列表。
- 已完成：检查最新一次 git commit、`TODO.md`、`PLAN.md`。
- 已确认：
  - 最新提交 `432b0b03 [T5000h0aR] Review production frontend materialized MIR retention` 的提交信息未额外引入“先修复某个遗留问题”的明确指示；
  - `TODO.md` 中第一个未完成任务是 `T5000h0b 建立 production 可复用的 materialized callable body / summary 视图`；
  - 该任务当前可在一轮内完成，不需要先拆成更小子任务。
- 已补充上下文阅读：
  - `crates/scoopc/src/hir/lower/types.rs`
  - `crates/scoopc/src/hir/lower/mod.rs`
  - `crates/scoopc/src/mir/{mod,materialize,summary}.rs`
  - `crates/scoopc/src/llvm/frontend.rs`
  - `crates/scoop/src/commands/build.rs`
  - `crates/scoopc/src/llvm/tests.rs`
- 当前判断的真实边界缺口：
  - production frontend 虽然已经保留 `MaterializedMir` 与 `summaries`，但消费方仍需要：
    - 手动扫描 `MaterializedMir.file.items` 找 body；
    - 用 `instance_keys + summaries.get(key)` 自己拼装实例视图；
    - 甚至在 overloaded generic 场景下，单靠 `InstanceKey` 还无法在消费侧可靠重建 root symbol。
  - 这说明“canonical materialized callable body / summary 视图”确实尚未建立，属于当前任务本身，而不是新的前置阻塞。

## 当前实现计划（已细化）

1. 在 `crates/scoopc/src/mir/` 新增专门的 callable-view 模块，定义：
   - materialized instance -> root callable / callable family 的稳定 side table；
   - 基于 `&MaterializedMir` 的只读查询视图类型。
2. 扩展 `MaterializedMir`：
   - 在 materialization 产出阶段保留每个 `InstanceKey` 对应的 `root_fqn` 与 family callable FQN 集；
   - 暴露 `MaterializedMir::callable_view()` 一类的稳定查询入口。
3. 扩展 `LoweredHir`：
   - 新增面向 production 消费侧的 `materialized_callable_view()` getter；
   - 文档注释中明确 raw `materialized_mir()` 与 canonical view 的职责区分。
4. 调整/补充测试：
   - MIR 侧覆盖 overloaded generic 实例在 view 中保持 distinct root/body/summary 身份；
   - single-file/build production 测试改为通过新 view 断言 body / summary 可查询，而不是手扫 `file.items`。
5. 运行格式化、相关测试、全量测试与 clippy；若暴露既有问题，先修复后再继续。
6. 完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交一次 commit。

## 下一步

- 开始实现 materialized callable view 与对应测试。

## 已完成实现

- 已完成 `T5000h0b 建立 production 可复用的 materialized callable body / summary 视图`。
- 具体落地：
  - 新增 `crates/scoopc/src/mir/callables.rs`，建立 canonical `MaterializedCallableView` / `MaterializedCallableFamilyView`；
  - `crates/scoopc/src/mir/materialize.rs` 现已在产出 `MaterializedMir` 时保留 `InstanceKey -> root_fqn / callable family` side table，并暴露 `MaterializedMir::callable_view()`；
  - `crates/scoopc/src/hir/lower/types.rs` 中新增 `LoweredHir::materialized_callable_view()`，供 production/frontend/codegen 主路径直接消费；
  - `crates/scoop/src/commands/build.rs` 与 `crates/scoopc/src/llvm/tests.rs` 的 production 回归已改为直接经由该 view 断言 root body / owner instance / family summary。

## 本轮额外修复的既有问题

- 在新 view 的验证过程中，暴露出一个真实既有 bug：
  - body-less `FunDecl`（例如 declaration-only surface）会被误当成“有 body 的 callable”，从而导致 `summary.body_known` 与 root body 可见性不一致；
  - 同时，`MaterializedMir` 产出阶段存在“同一 `InstanceKey` 已 materialize 后，又被 declaration-only 输入重复覆盖”的风险。
- 已修复方式：
  - 新 view 与 materialized family side table 现在只索引 `FunDecl.body.is_some()` 的真实 callable body；
  - 在 materializer 收尾阶段过滤“已 materialize 的 exact `InstanceKey`”对应的 declaration-only 输入，避免覆盖 canonical family / summary 视图。

## 已完成验证

- `cargo fmt --all`
- `cargo test -p scoopc callable_view_keeps_overloaded_generic_roots_distinct -- --nocapture`
- `cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
- `cargo test -p scoop build_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo run -p scoop -- test`（结果：`fixtures: ok (1201)`）

## 收尾状态

- 已完成：
  - `TODO.md` / `PLAN.md` 的任务状态与完成记录已写回。
- 待做：
  - 创建本轮 git commit；
  - 停止，不进入下一条任务。
