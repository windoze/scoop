# 本轮执行计划

## 目标
- 先记录执行计划，再开始任何仓库检查或命令执行。
- 检查最新提交是否提到已有问题；若有，优先修复。
- 读取 `TODO.md`，确定第一个未完成任务。
- 如任务过大，先拆分任务并更新 `PLAN.md` / `TODO.md`。
- 只完成当前首个未完成任务，然后测试、更新文档、提交并停止。

## 执行步骤
1. 查看最新一次提交信息，确认是否提到需要先修复的既有问题。
2. 读取 `TODO.md`、`PLAN.md`，确定当前首个未完成任务及其上下文。
3. 判断任务是否可在本轮完整完成：
   - 若可完成，直接实现。
   - 若过大或被前置缺陷阻塞，先在 `TODO.md` / `PLAN.md` 中补充或重排任务。
4. 实现当前任务，必要时补充或调整测试。
5. 运行相关验证：
   - 至少运行与改动直接相关的测试。
   - 若改动影响面较大，运行更完整的测试集。
   - 目标是通过 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`；若范围受限，会在此文件中记录原因。
6. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或阻塞原因。
7. 检查工作区差异，整理提交，使用清晰的提交信息提交。
8. 提交后停止，不继续处理下一个任务。

## 记录约定
- 如果计划发生变化，立即更新本文件。
- 每完成一个关键步骤，在本文件追加进度记录。

## 进度记录
- 已创建本文件并写入初始计划，尚未开始仓库检查。
- 已检查最新提交 `49f3279`，提交信息为 `[T4007S] 关闭已收口的 compaction 验证 blocker`；未见需要优先修复的新既有问题说明。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务为 `T4007R`：复审 RTTI 是否仍会对 generic / `eff` target 静默退回未参数化 descriptor / interface id。
- 当前细化执行计划：
  1. 读取 `T4007R` 相关任务说明、`ISSUES.md` 证据条目与 RTTI 相关实现位置。
  2. 审查参数化 nominal / interface / `eff` target 的 lowering、descriptor 构造、旧 RTTI 查询与运行期匹配代码路径。
  3. 运行定向测试与必要的全量验证，确认 `dump-rtti`、`is/as/as?`、canonical name、`type_id` 观测一致。
  4. 若发现裂缝，先修复并补回归；若未发现，则更新 `TODO.md` / `PLAN.md` 记录 review 结论。
  5. 完成后提交一次 git commit，并停止。
- 已完成 RTTI 相关代码复审：
  - `crates/scoopc/src/rtti/mod.rs`
  - `crates/scoopc/src/rtti/type_desc.rs`
  - `crates/scoopc/src/itable.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/gc.rs`
- 已完成定向验证：
  - `cargo test -p scoopc rtti:: -- --nocapture`
  - `cargo run -q -p scoop -- dump-rtti ... --type 'Holder<Int>'`
  - `cargo run -q -p scoop -- dump-rtti ... --type 'StringReadable'`
  - `cargo run -q -p scoop -- dump-rtti ... --type 'PureManaged'`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop`
- 已完成额外 probe：`/tmp/t4007r_named_readable_any_probe.scoop` 证明当源码中出现 `NamedReadable<Any>` target 时，运行期 `is` 结果与 `dump-rtti` 导出的 `runtime_match_type_names` / `runtime_match_type_ids` 仍同步，没有退回 base interface id。
- 已完成全量验证：
  - `cargo run -p scoop -- test` -> `fixtures: ok (1055)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 结论：未发现需要新增到 `TODO.md` 的 RTTI blocker；本轮将仅更新 `TODO.md` / `PLAN.md` / 本文件并提交。
