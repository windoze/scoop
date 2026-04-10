# 本轮执行计划

说明：按要求先记录当前的高层执行计划与检查顺序。已完成初步仓库检查；以下补充本轮识别出的具体任务、范围与执行细化计划。

## 初始约束

- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始该任务前，先检查最新提交是否提到已有问题；若有，先修复这些问题。
- 如果首个未完成任务过大，需要先拆分，并同步更新 `PLAN.md` 与 `TODO.md`。
- 所有改动完成后，需要运行格式化、测试和无 warning 的 lint。
- 完成后更新 `TODO.md`、`PLAN.md`，并提交 git commit。

## 初始执行步骤

1. 查看最新一次 git 提交，确认是否提到已知问题、回归或待补修事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 结合 `PLAN.md`、相关源码与测试，判断该任务是否可直接落地；若过大，先拆分任务并更新计划文件。
4. 实现当前目标任务所需的代码改动，必要时补充或整理注释与文档。
5. 运行相关测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`，修复发现的问题。
6. 更新 `TODO.md` 与 `PLAN.md`，记录本轮完成情况与后续状态。
7. 使用清晰的提交信息创建一次 git commit，然后停止。

## 进度记录

- 已创建初始计划文件。
- 已检查最新提交：`a5fe8fb03d0aceda065ee386f8b22ac350cd4480`（`[T0147c-2b] Refactor typecheck support module inputs`）。
- 最新提交说明中未显式提到待补修的遗留问题；当前无“必须先修复的提交内问题”阻塞项。
- 已读取 `TODO.md` 与 `PLAN.md`：首个未完成任务为 `T0147c-2c`，目标是清理 `typecheck/expr/member.rs`、`typecheck/expr/ops.rs`、`typecheck/expr/stmt.rs` 中全部 `too_many_arguments`。
- 该任务已经是可单轮完成的子任务，当前不需要继续拆分 `TODO.md` / `PLAN.md`。
- 已完成精确告警收集：确认目标文件共有 17 个 `too_many_arguments`（`member.rs` 5、`ops.rs` 4、`stmt.rs` 8）。
- 已完成代码修改：引入共享输入/局部状态对象，收口三个目标模块及其调用点的长参数函数。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo check -p scoopc`
  - `cargo clippy -p scoopc --all-targets --message-format short -- -W clippy::too_many_arguments`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
- 验证结果：
  - `member.rs` / `ops.rs` / `stmt.rs` 的 17 个 `too_many_arguments` 告警已全部清零。
  - 全量测试通过；fixture 回归结果为 `fixtures: ok (852)`。
  - 剩余 `too_many_arguments` 总数为 36，集中在后续任务 `T0147c-2d`（`call.rs` 12、`entry.rs` 9、`infer.rs` 15）。
  - 全量严格 `cargo clippy --workspace --all-targets -- -D warnings` 仍被后续任务范围内的既有 warning 阻塞（剩余长参数、`result_large_err`、`private_interfaces`、`dead_code` 等），不属于本轮新增问题。
- 待执行：更新 `TODO.md` / `PLAN.md` 后提交 git commit 并停止。

## 本轮具体计划（T0147c-2c）

1. 收集 `member.rs` / `ops.rs` / `stmt.rs` 中 `too_many_arguments` 的精确函数列表与调用路径。
2. 识别重复传递的共享参数，优先提炼为局部上下文结构体或请求对象，避免简单加 `allow`。
3. 修改三个模块及必要调用点，保持 typecheck 规则、诊断文本与 fixture 行为不变。
4. 运行 `cargo fmt --all`。
5. 运行针对性验证与全量验证：
   - `cargo clippy -p scoopc --all-targets --message-format short -- -W clippy::too_many_arguments`
   - `cargo test --all`
   - `cargo run -p scoop -- test`
6. 更新 `TODO.md` 与 `PLAN.md`，将 `T0147c-2c` 标记完成并记录本轮收敛结果。
7. 提交 git commit，然后停止。
