## 当前执行记录

### 约束说明
- 按要求先记录执行计划，再进行仓库检查与代码/命令操作。
- 这里记录的是可审计的执行计划、决策依据和进度，不包含不可审计的内部推理细节。

### 初始计划
1. 检查最新一次 Git 提交的标题、正文与变更范围，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如首个未完成任务过大，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
4. 实现当前应执行的任务，并补充或调整相关测试。
5. 运行必要验证，至少覆盖：
   - 相关针对性测试；
   - `cargo test --all`（若耗时或依赖过大，则先跑受影响范围，再评估是否补全全量）；
   - `cargo clippy --all-targets -- -D warnings`（若本次改动影响到对应 crate）。
6. 更新文档与任务状态：
   - 在 `TODO.md` 中将已完成任务标记完成，或在阻塞时调整任务顺序并说明依赖；
   - 更新 `PLAN.md`；
   - 继续维护本文件中的进度记录。
7. 使用清晰的提交信息提交本轮改动，然后停止，不继续后续任务。

### 待确认事项
- 最新提交是否显式提到未修复问题。
- `TODO.md` 中第一个未完成任务的具体范围与依赖。
- 当前仓库是否已有未提交改动，需要避免覆盖。

### 当前结论（已确认）
- 最新提交标题为 `[T2003c0c2d1] Support pure direct multiple escape arms`，提交正文未附带“已知问题待补”说明，因此没有额外的“先修提交中注明问题”事项。
- `TODO.md` 中第一个未完成任务是 `T2003c0c2d2`。
- 该任务当前同时覆盖：
  - multiple escape arms + sibling non-resuming；
  - multiple escape arms + `finally`；
  - indirect call site；
  - nested block / if / while replay；
  - 上述能力的组合收口。
- 这些维度分别落在不同 lowering 路径与 cleanup 语义上，单轮实现和回归面过大，必须先拆分。

### 已调整计划
1. 先更新 `TODO.md` / `PLAN.md`，把 `T2003c0c2d2` 拆成更小子任务。
2. 将本轮执行目标固定为拆分后的第一个子任务：`T2003c0c2d2a`。
3. 读取现有 `multi_escape` / `mixed` / `shared` lowering，复用现成的 sibling non-resuming dispatch helper，避免重复实现。
4. 实现 `T2003c0c2d2a`，范围限定为：
   - 无 immediate-resume；
   - 多个 escape-continuation arms；
   - 允许 sibling non-resuming；
   - 仅 top-level direct single-site；
   - 继续保留 `finally` / indirect / nested replay 的稳定边界。
5. 新增或改造 fixtures，优先把现有 sibling non-resuming build-fail 边界转为 run-pass，并补一个仍未支持组合的负例锁边界。
6. 运行格式化、相关测试、全量测试/LLVM fixture/clippy，更新 `TODO.md`、`PLAN.md` 与本文件后提交。

### 当前进度（已完成）
- 已把 `T2003c0c2d2` 拆成 `T2003c0c2d2a`～`T2003c0c2d2f`，并同步更新 `TODO.md` / `PLAN.md`。
- 已完成 `T2003c0c2d2a` 的实现：
  - 删除了 `handle multiple escape-continuation arms with sibling non-resuming not yet supported` 的入口门禁。
  - `codegen_handle_expr_multiple_escape_top_level_direct` 现已支持 no-immediate multiple escape arms + sibling non-resuming 的 top-level direct single-site 子集。
  - handle body 与 continuation step 已接入 sibling custom non-resuming / `Raise.raise` dispatch。
  - escape arm body 与 sibling catch body 若再次触发同源 sibling non-resuming，会走 cleanup/unwind 向外传播，不再自捕获。
- 已更新 fixtures：
  - 新增 run-pass：`effect_multi_escape_multi_arm_with_nonresuming`
  - 新增 build：`effect_multi_escape_multi_arm_with_finally_is_error`
  - 删除旧的 sibling 边界 build fixture：`effect_multi_escape_multi_arm_with_nonresuming_is_error`
- 已完成验证：
  - `cargo fmt --all`
  - `cargo check -p scoopc`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`

### 收尾步骤
1. 检查工作区 diff，确认只包含本轮任务与计划/记录更新。
2. 将 `T2003c0c2d2a` 标记完成并确认 `T2003c0c2d2b` 成为首个未完成任务。
3. 提交本轮改动，提交后停止。
