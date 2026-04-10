# 当前执行计划

## 约束说明

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始正式实现前，先检查最近一次提交是否提到遗留问题；若有，先修复这些问题。
- 若首个未完成任务过大，则先把它拆成更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行拆分后的第一个子任务。
- 过程中持续更新本文件，记录计划变化、关键进展、测试结果与阻塞情况。

## 初始步骤

1. 查看最新一次 Git 提交信息，确认是否提到需要先处理的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md` 及相关上下文，判断该任务是否足够小且可直接完成。
4. 如任务过大，先细化任务并更新 `PLAN.md`/`TODO.md`。

## 执行步骤

1. 阅读与当前任务直接相关的源码、测试、文档。
2. 实现任务所需代码修改，必要时补充注释与文档。
3. 运行格式化、测试、lint，并修复发现的问题，目标是不产生告警。
4. 更新 `TODO.md` 与 `PLAN.md`，记录本轮完成情况。
5. 提交 Git commit，然后停止，不继续下一个任务。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交 `be8c63c1146daf9ac72de1d9d26c286a97c63dc1`：提交信息未显式提到需先修复的遗留缺陷。
- 已读取 `TODO.md` / `PLAN.md`，定位到首个未完成任务为 `T0147c-3b`：收缩 `ExprTypeError` 主路径的 `result_large_err`。
- 已运行严格 `cargo clippy --all-targets -- -D warnings`：
  - 结果确认 `ExprTypeError` 相关 `result_large_err` 仍是当前主阻塞之一；
  - 其它失败项（`private_interfaces`、`dead_code`、`large_enum_variant`、`question_mark` 等）属于后续 `T0147c-3c` 范围。
- 当前 refined plan：
  1. 先收缩 `ExprTypeError` 自身尺寸，优先处理已确认的超大 variant `GenericTypeArgInferenceConflict`。
  2. 重新运行定向 clippy（`-D clippy::result_large_err`）验证是否已清空 `ExprTypeError` 主路径告警。
  3. 若仍有 `result_large_err`，再评估是否需要引入 boxed result alias 扩展到 `expr/**`、`eff_row_subst`、`val_pat`、`when_*`。
  4. 通过后运行相关测试与全量 clippy 复核，更新 `TODO.md` / `PLAN.md` / 提交。
- 关键进展：
  - 已将 `ExprTypeError::GenericTypeArgInferenceConflict` 的 6 个 `String` 字段改为装箱形式，并同步更新 `infer.rs` / `call.rs` 两个构造点。
  - 已运行 `cargo clippy --workspace --all-targets --message-format short -- -A warnings -D clippy::result_large_err`，结果通过。
  - 结论：`ExprTypeError` 的 `result_large_err` 主问题已消除；下一步转入完整验收与文档更新。
- 验收结果：
  - `cargo fmt --all` 通过。
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings` 已复核：
    - 不再出现 `ExprTypeError` 相关 `result_large_err`；
    - 剩余失败仅为后续 `T0147c-3c` 的结构性 lint（`private_interfaces`、`dead_code`、`large_enum_variant`、`question_mark`、`type_complexity` 等）。
  - `cargo test --all` 通过。
  - `cargo run -p scoop -- test` 通过，结果为 `fixtures: ok (852)`。
- 待完成收尾：
  1. 已更新 `TODO.md`，将 `T0147c-3b` 标记为完成并写入完成说明。
  2. 已更新 `PLAN.md`，同步记录本轮实现与当前剩余阻塞已转移到 `T0147c-3c`。
  3. 待检查工作区 diff 并提交本轮改动。
