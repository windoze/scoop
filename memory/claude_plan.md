# 执行记录

## 初始计划

### 目标
- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始实际实现前，先检查最新提交是否提到需要先修复的遗留问题；如有，则先处理这些问题。
- 在任务过大时，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。

### 约束与执行原则
- 先记录计划，再执行检查、实现、测试、文档更新和提交。
- 不跳过测试；需要尽量运行与改动直接相关的测试，并满足无警告要求。
- 只完成一个任务；完成后更新 `TODO.md`、`PLAN.md`，提交 git commit，然后停止。
- 如果遇到阻塞，不把任务标成 blocked，而是调整 `TODO.md` 顺序并在 `PLAN.md` 记录原因。

### 预计步骤
1. 检查最新一次 git 提交信息，确认是否提到需要优先修复的问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md`、相关代码和测试，判断任务范围与实现方式。
4. 如果任务过大，先拆分成更小的子任务，并更新 `TODO.md` 与 `PLAN.md`。
5. 实现当前目标任务。
6. 运行格式化、测试、`clippy` 或其他必要校验，修复发现的问题。
7. 更新 `TODO.md`、`PLAN.md` 和本文件的进度记录。
8. 提交本轮改动，提交信息聚焦当前任务。
9. 停止，不继续处理下一个任务。

### 当前未知项
- 最新提交是否包含必须先修复的遗留问题。
- `TODO.md` 的第一个未完成任务具体是什么。
- 当前任务是否需要先拆分。

## 进度更新

### 已确认事项
- 最新提交为 `[T0147c-2d] Clippy 基线清理：typecheck expr 主干签名收口`，提交说明未额外指出必须先修复的遗留问题。
- `TODO.md` 中第一个未完成任务是 `T0147c-3`：清理 `result_large_err` 与剩余零散 warning，恢复严格 `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- 当前任务范围清晰，先不拆分；先以严格 clippy 输出为准锁定剩余告警，再做代码调整。

### 当前执行计划
1. 运行严格 `cargo clippy --workspace --all-targets -- -D warnings`，记录剩余告警的文件和类型。
2. 评估任务规模；如果超出单轮可控范围，则先拆分 `T0147c-3` 并同步 `TODO.md` / `PLAN.md`。
3. 优先实现新的第一子任务。
4. 运行格式化和该子任务相关的验证命令。
5. 更新 `TODO.md`、`PLAN.md` 和本文件，提交当前任务并停止。

### 本轮基线观察
- 严格 `clippy` 当前失败规模远大于“少量零散 warning”，总计约 175 个错误。
- 其中绝大多数是 `result_large_err`，集中在：
  - `typecheck/expr/*`
  - `typecheck/properties.rs`
  - `typecheck/val_pat.rs`
  - `typecheck/when_*`
  - `typecheck/override_effects.rs`
  - `typecheck/eff_row_subst.rs`
  - `cone/scoopir/export.rs`
  - `monomorph/lower.rs`
- 其余还包括 `private_interfaces`、`dead_code`、`large_enum_variant`、`type_complexity`、`question_mark`、`if_same_then_else`、`while_let_loop`、`vec_init_then_push`、`cloned_ref_to_slice_refs` 等。

### 计划调整
- `T0147c-3` 对单轮来说过大，需要拆分。
- 拟拆分方向：
  1. 先处理大头：统一收缩 `result_large_err`（优先 typecheck 主路径，并覆盖 cone/monomorph 的返回错误载体）。
  2. 再处理结构性/风格性 lint（可见性、死代码、复杂类型、控制流写法、enum 体积等）。
- 下一步：阅读错误类型定义与热点模块签名，确定最小但成体系的第一子任务边界，然后更新 `TODO.md` / `PLAN.md`。

### 拆分结果
- 已将 `T0147c-3` 拆分为：
  1. `T0147c-3a`：收缩非 `Expr` 主路径的大 `Err`
  2. `T0147c-3b`：收缩 `Expr`/模式匹配路径的大 `Err`
  3. `T0147c-3c`：清零剩余结构性 warning
- 本轮执行目标已切换为新的第一子任务 `T0147c-3a`。

### 本轮实现结果
- 已完成 `T0147c-3a`。
- 实现要点：
  - `typecheck/properties.rs`
  - `typecheck/override_effects.rs`
  - `cone/scoopir/export.rs`
  - `monomorph/lower.rs`
  以上模块统一切到 boxed result alias，缩小返回错误载体。
  - 为 `override_effects.rs` / `monomorph/lower.rs` 的 `?` 链路补齐 `From<...> for Box<...>`。
  - `scoop/src/fixtures/mod.rs` 新增 boxed diagnostic 包装函数；`commands/build.rs`、`commands/dump_ir.rs`、`cone/scoopir/export.rs` 内部 `miette::Report` 转换点改为显式解箱。

### 验证结果
- `cargo fmt --all` 通过。
- `cargo check --workspace --message-format short` 通过。
- `cargo clippy --workspace --all-targets --message-format short -- -A warnings -D clippy::result_large_err` 已执行：
  - 本轮负责的 4 个模块已不再出现在输出中。
  - 剩余 `result_large_err` 仅在后续 `T0147c-3b` 范围：`typecheck::expr/**`、`eff_row_subst`、`val_pat`、`when_pat`、`when_exhaustiveness`。
- `cargo test --all` 通过。
- `cargo run -p scoop -- test` 通过（`fixtures: ok (852)`）。

### 收尾步骤
1. 更新 `TODO.md` 与 `PLAN.md` 为当前完成状态。
2. 查看 `git diff`，确认改动范围与任务一致。
3. 提交 commit。
4. 停止，等待下一轮处理 `T0147c-3b`。
