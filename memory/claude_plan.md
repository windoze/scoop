# 执行计划与进度记录

## 说明

按要求先记录完整的可执行计划与关键决策摘要。这里保留可审计的步骤、依据、风险与进度更新；不记录不可审计的内部推理细节。

## 初始目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。若发现最新提交已注明的遗留问题，或在执行中发现任何既有缺陷/规格不匹配/实现边界不完整，则优先修复该问题；如果该问题无法在本轮直接修复，则必须先把它整理成前置任务写回 `TODO.md` / `PLAN.md`，提交后停止。

## 执行步骤

1. 查看最新一次 git 提交信息，确认是否明确提到了待修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务及其上下文。
3. 判断该任务是否足够小可以在本轮完整实现。
4. 若任务过大：
   - 将其拆分为更小的可执行子任务。
   - 更新 `PLAN.md`。
   - 更新 `TODO.md`，让新的第一个子任务成为当前任务。
   - 本轮只执行拆分后的第一个子任务。
5. 在正式实现前，检查相关代码、测试、规格和最近变更，避免误改或绕过已有缺陷。
6. 实现当前任务所需代码变更。
7. 运行相关测试；若有失败，区分是本次引入还是已有问题：
   - 若是本次变更问题，立即修复并重测。
   - 若暴露既有问题且会阻塞当前任务，则先修复；若无法在本轮直接修复，则写入前置任务并停止。
8. 更新文档与任务状态：
   - 在 `TODO.md` 标记当前任务完成，或在阻塞场景下重排任务依赖。
   - 在 `PLAN.md` 记录当前状态与后续顺序。
   - 在本文件追加关键进度记录。
9. 运行质量检查，至少覆盖与本任务相关的测试，以及在可行范围内运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
10. 提交 git commit，提交信息直接描述本轮完成事项。
11. 停止，不继续处理下一个任务。

## 当前已知风险

- 尚未读取仓库现状，不能假设 `TODO.md` 的首个未完成任务规模合适。
- 尚未确认最新提交是否包含“先修复某既有问题”的要求。
- 尚未确认工作树是否存在用户未提交修改；执行时需要避免覆盖非本轮改动。

## 进度日志

- 2026-04-27：已创建本文件并写入初始计划；下一步查看最新提交与任务列表。
- 2026-04-27：已检查最新提交 `3e2584b1f3f039bed9467cc56d788b9f9450c56c`（`[T5000h0] Insert MIR body frontend prerequisite`）。
  - 结论：该提交没有声明“另有一个尚未修复但必须先实现的代码缺陷”；它做的是任务重排，把一个已确认的前置边界缺口显式插入到 `T5000h` 之前。
  - 当前首个未完成任务已定位为 `T5000h0 让 build/frontend 主路径消费 materialized MIR body，而不再只把 MIR 当 instance collection 发现器`。
- 2026-04-27：针对 `T5000h0` 的当前执行计划细化如下。
  1. 阅读 `crates/scoopc/src/mir/materialize.rs`，确认 `MaterializedMir` 当前携带哪些 body / summary / type 信息。
  2. 阅读 `crates/scoopc/src/hir/lower/mod.rs` 及其 frontend 入口，确认 build / single-file 主路径目前如何只消费 `instance_keys`。
  3. 追踪 LLVM frontend / codegen 接口，看 callable body 真正来自哪里，以及哪些 side tables 仍依赖 HIR 兼容 lowering。
  4. 设计最小且正确的接线方案：
     - 让 production 主路径显式携带 materialized MIR body；
     - 让现有 side tables 继续可用，但明确区分“实例发现”与“body 来源”；
     - 给后续 summary / MIR rewrite 留稳定入口。
  5. 实现代码修改，并补齐/更新测试，优先覆盖：
     - build / single-file 主路径确实能看到 materialized MIR body；
     - MIR body 在进入 production 前可以被稳定读取或改写；
     - 相关现有回归不退化。
  6. 跑格式化、定向测试、全量测试/fixture、clippy。
  7. 更新 `TODO.md` / `PLAN.md` / 本文件，提交 commit，然后停止。
- 2026-04-27：进一步阅读后，确认原始 `T5000h0` 需要跨越三个边界，一次做完风险过高，已决定先拆分并执行首个子任务。
  - 证据：
    - `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)` 当前只把 `materialized.instance_keys` / `materialized.types` 回灌到 HIR 兼容 lowering；
    - `llvm/emit.rs`、`llvm/reachability.rs`、`effect_state_machine_analysis.rs` 仍把 `hir::FunDecl.body` 当作 callable body 的直接来源；
    - 仓库里不存在现成的“materialized MIR body -> 当前 production/codegen 主线”完整桥接，因此把“保留 MIR 产物”“建立 canonical 视图”“改 entry 接线”一次混做会难以保证单轮可验收。
  - 拆分结果：
    1. `T5000h0a`：先让 build / single-file frontend 的 production 产物稳定保留 `MaterializedMir.file/types/instance_keys/summaries`。
    2. `T5000h0b`：再在 production 产物上建立 canonical materialized callable body / summary 视图。
    3. `T5000h0c`：最后调整 LLVM build / single-file entry，显式接入该视图。
  - 本轮将先更新 `TODO.md` / `PLAN.md` 完成拆分并提交，然后继续实现 `T5000h0a`。
