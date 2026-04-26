# 当前轮执行计划

## 约束说明

- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始任务前，先检查最新提交是否提到需要先修复的既有问题；若有，先修复这些问题。
- 在执行过程中，任何探测、测试、评审中发现的既有 bug、回归、规格不一致、未完成实现边界或现有 workaround，都必须优先处理，不能绕过。
- 若当前任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
- 完成实现后必须运行相关验证；若可行，还要运行更严格的无警告检查，例如 `cargo clippy --all-targets -- -D warnings`。
- 需要持续维护本文件；当计划调整、发现阻塞、完成关键步骤时，及时更新这里。

## 初始执行步骤

1. 查看最新一次 git 提交，确认提交信息里是否提到待修复的既有问题，必要时先定位并修复。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的背景、依赖与当前项目阶段。
4. 结合代码与测试现状评估该任务是否可在本轮完整完成：
   - 若可完成：继续实现。
   - 若过大或存在未被记录的前置缺口：把任务拆成更小子任务，更新 `TODO.md` / `PLAN.md`，本轮只执行新的第一个子任务，或者在被阻塞时仅记录新的前置任务后停止。
5. 在实现前阅读相关模块、现有测试、规格说明与最近变更，明确影响面。
6. 实现任务，并同步补充或调整测试。
7. 运行与该任务相关的测试；若影响面较大，补充运行更广泛的测试与静态检查。
8. 更新 `TODO.md`、`PLAN.md`、本文件，记录已完成内容或阻塞原因。
9. 提交 git commit，提交信息聚焦本轮完成的任务或新识别的前置修复任务。

## 决策准则

- 如果发现“已有问题”会影响当前任务的正确实现，则先修该问题；如果无法在本轮直接修复，就必须先把该问题写入 `TODO.md` 并排到当前任务之前，然后停止。
- 不接受通过缩小范围、改变数据表示、削弱测试形状、增加特判、跳过错误路径等方式“先让任务过掉”。
- 所有任务状态必须明确：完成才勾选；若受阻，保持未完成并调整顺序，不使用 `[BLOCKED]`。

## 本轮预期产出

- 一个已完成并验证通过的首个未完成任务，或者
- 一个被正确拆分/重排并记录前置依赖的任务列表更新（若存在真实阻塞）。

## 进度记录

- 已完成：创建本计划文件，准备开始仓库检查。
- 已完成：检查最新 git 提交说明，提交标题为 `[T5000e3R] Review monomorphization boundary stability`，未发现额外正文中声明的待修复遗留问题。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，定位到首个未完成任务为 `T5000f 建立 per-instance summary 基础设施`。
- 已完成：读取 `T5000e3R` 与 `T5000f` 相邻上下文，确认前一轮 review 未留下必须先插入 `TODO.md` 的未记录阻塞项。
- 已完成：评估 `T5000f` 的可实现性，结论是本轮可直接实现，但过程中暴露一个真实前置边界缺口：当前 MIR 的 `Return` terminator 不显式携带返回 operand，这会削弱 `result_provenance` 与“参数是否经返回逃逸”的稳定计算。
- 决策：该缺口直接服务 `T5000f` 的验收目标，且影响面目前集中在 `crates/scoopc/src/mir/{mod,lower,materialize}.rs` 与相关测试，预计可在本轮一并修复，不需要先拆任务或改写 `TODO.md` 顺序。
- 接下来的实施步骤：
  1. 修改 MIR `Return` 形状，让 terminator 显式携带 `Option<Operand>` 返回值，并更新 lowering / materialization / MIR 测试。
  2. 设计并实现 `per-instance summary` 数据结构，挂到 `MaterializedMir` 的稳定 side tables 上。
  3. 基于 materialized MIR 计算 summary：
     - `body_known`
     - `size_cost`
     - `recursive_scc`
     - `may_outward_effect`
     - `may_allocate_closure`
     - `param_use_summaries`
     - `result_provenance`
  4. 为 `dump` / compilation-unit materialization 添加回归测试，覆盖：
     - summary 按 `InstanceKey` 建立，而不是按模板函数名；
     - 返回参数/closure/直接函数时的 provenance；
     - `DirectCallOnly` / `Escapes` 的基本分类；
     - 递归实例与 declaration-only instance 的保守 summary。
  5. 运行格式化、相关测试、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
  6. 更新 `TODO.md` / `PLAN.md` / 本文件，提交 commit，然后停止。
- 已完成：实现 `T5000f` 主体。
  - 已新增 `crates/scoopc/src/mir/summary.rs`，把 per-instance summary 挂到 `MaterializedMir` 上；
  - 已把 `TerminatorKind::Return` 改为显式携带 `Option<Operand>`，并同步接通 lowering/materialization；
  - 已落地 `body_known / size_cost / recursive_scc / may_outward_effect / may_allocate_closure / param_use_summaries / result_provenance`。
- 已完成：验证实现。
  - `cargo fmt --all`
  - `cargo test -p scoopc mir::summary -- --nocapture`
  - `cargo test -p scoopc`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以上全部通过。
- 已完成：更新 `TODO.md` 与 `PLAN.md`，把 `T5000f` 标记为完成，并把下一条待执行任务切到 `T5000fR`。
- 下一步：检查工作区 diff，提交本轮变更，然后停止。
