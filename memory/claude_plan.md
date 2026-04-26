# 本轮执行计划

## 目标

完成 `TODO.md` 中第一个未完成任务；如果在检查最近提交、测试或实现过程中发现已有缺陷、规格不匹配或阻塞项，则先修复该问题，或把它整理成位于阻塞任务之前的前置任务，然后停止。

## 约束

- 先检查最近一次提交是否提到需要先处理的遗留问题。
- 先定位 `TODO.md` 中第一个未完成任务，再决定是否需要拆分。
- 不采用绕过实现缺口的变通方案；发现规格缺口时要显式写入 `TODO.md` / `PLAN.md`。
- 本轮只完成一个任务（或在被阻塞时只完成任务重排与记录），随后提交 git commit 并停止。
- 变更前后都要同步更新本文件，记录计划调整、关键步骤和结论。

## 初始步骤

1. 检查最近一次提交信息，确认是否存在必须先修复的遗留问题。
2. 查看工作树状态，识别可能影响本轮任务的未提交改动。
3. 阅读 `TODO.md` 与 `PLAN.md`，确定首个未完成任务及其上下文。
4. 评估该任务是否过大；若过大，则拆成更小子任务并更新 `TODO.md` / `PLAN.md`。
5. 实现当前应执行的任务。
6. 运行相关测试与质量检查，至少覆盖受影响范围；必要时补充测试。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 提交本轮改动，提交后停止。

## 记录区

- 初始化：已写入初始计划，尚未开始仓库检查。
- 已完成仓库检查：
  - 最近一次提交为 `[T5000e2cR] Finalize execution log`，提交消息未额外点名需要先修复的遗留问题。
  - 当前工作树仅有本文件改动，未发现其他未提交文件影响本轮判断。
  - `TODO.md` 的首个未完成条目是 `T5000e2R Review：确认编译单元级 monomorphization 已脱离 HIR eager materialization`。
- 当前审查焦点：
  1. 核对 build/frontend 与 single-file LLVM frontend 是否都已走 `lower_for_compilation_unit_multi_files_via_mir_instance_collection(...)` 主路径。
  2. 核对 `TemplateKey` / `InstanceKey`、canonical template 选择与跨文件 request binding 是否稳定。
  3. 核对 HIR compatibility lowering 是否仍在生产主路径上保留 eager clone 语义。
  4. 若确认只是注释/文档错配，也要一并修复后再完成 review。
- 已完成实现与验证：
  - 已修正 `crates/scoop/src/commands/build.rs` 中关于 `monomorph_keys` / `typecheck_types` 的过期注释，避免继续把当前主路径误写成 HIR eager lowering。
  - 已新增 build frontend 回归测试：`build_frontend_does_not_eager_materialize_unused_owner_specialized_getter`。
  - 已验证：
    - `cargo test -p scoop build_frontend_ -- --nocapture`
    - `cargo test -p scoopc single_file_frontend_ -- --nocapture`
    - `cargo test -p scoopc typechecked_compilation_unit_materialization_ -- --nocapture`
    - `cargo test --all`
    - `cargo clippy --all-targets -- -D warnings`
    - 全部通过。
- 本轮结论：
  - `T5000e2R Review` 可判定完成；
  - 未发现需要插入到 `T5000e3` 之前的新前置缺陷任务；
  - 下一条待执行任务应为 `T5000e3`。
