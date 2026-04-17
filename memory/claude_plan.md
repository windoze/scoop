## 本轮工作记录（2026-04-18）

### 当前已知状态

- 继承上一轮未提交工作：原目标任务为 `T3009b2`。
- 代码层面已经完成一部分前置调查与实现：
  - ordinary callee 已开始真实消费 `source_path.frames`；
  - statement-container 路径不再直接在规划阶段返回 `None`；
  - 新增了针对 `block / if / when / while body` 的 focused matrix fixture；
  - 新增了一个 IR 定向测试，用来确认 handle call-site active-dispatch 仍然存在。
- 进一步验证后确认：`T3009b2` 现在暴露出的真实前置阻塞不是 statement-container rebuild 本身，而是 escaped continuation 在第一次 `resume(...)` 之后继续执行 resumed caller-tail 时，下一次 outward `perform` 没有重新进入 captured handler dispatch loop。
- 这个问题属于更前置的语义/运行路径缺口，已在 `TODO.md` 中前移为 `T3015a` / `T3015aR`，并让 `T3009b2` 显式依赖 `T3015aR`。
- 目前尚未完成的管理动作：
  - `PLAN.md` 还没有同步这次阻塞重排；
  - 还没有按 blocked 流程提交 commit；
  - 本轮应在补齐文档与提交后停止，不能继续实现 `T3015a`。

### 已完成的核对

- 已检查最新 commit：`[T3009b2bR] Review ordinary callee resumed-body restore`，提交说明本身没有再引入一个必须先于当前 blocker 处理的新遗留问题。
- 已确认 `TODO.md` 现在的首个未完成任务已经是前移后的 `T3015a`，这说明上一轮发现的 blocker 重排方向正确；本轮不应继续实现 `T3015a`，而应先把本次 blocked 重排正式落盘并提交。
- 已更新 `PLAN.md`，把本轮 blocker、验证结果与新的执行顺序（`T3015a` → `T3015aR` → `T3009b2`）写入计划文件。

### 对最新提交和现状的处理策略

- 先查看最新 commit，确认其中是否提到需要本轮先修复的 pre-existing issue。
- 若最新 commit 没有新增必须先修的遗留问题，则以当前已识别出的 blocker 作为本轮的唯一处理对象。
- 由于 blocker 已经被确认且已经影响任务排序，本轮目标不是继续写功能，而是把任务依赖和计划文档修正为真实状态，并提交一次清晰的历史记录。

### 本轮执行计划

1. 检查最新 commit 内容，确认是否存在必须优先处理的 pre-existing issue。
2. 读取 `TODO.md` 与 `PLAN.md`，确认 `T3015a` / `T3015aR` / `T3009b2` 的当前排序和描述状态。
3. 更新 `PLAN.md`：
   - 写明 statement-container rebuild 已接上；
   - 写明 `T3009b2` 被 resumed-segment redispatch 缺口阻塞；
   - 把执行顺序调整为先 `T3015a`，再 `T3015aR`，后 `T3009b2`。
4. 视检查结果补充更新 `memory/claude_plan.md`，记录关键结论与完成状态。
5. 复查工作树差异，确保没有遗漏必须纳入本次 commit 的文件。
6. 按 blocked 流程提交：
   - 保持 `T3009b2` 为未完成；
   - 保留并前移 blocker 任务；
   - 提交本轮代码、测试、`TODO.md`、`PLAN.md`、`memory/claude_plan.md` 的一致状态。
7. 停止，不继续下一任务。

### 本轮完成判定

- `PLAN.md` 已同步 blocker 与顺序调整；
- `memory/claude_plan.md` 已记录本轮最终判断；
- 已完成 git commit；
- 提交后立即停止。

### 已完成的验证

- `cargo check -p scoopc`：通过。
- `cargo test -p scoopc indirect_if_branch_callee_keeps_handle_call_site_active_dispatch -- --nocapture`：通过。
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_indirect_callee_suspend_matrix.scoop`：运行到第二个 indirect callee 的 `counter_enter` 后截断，确认 shared multi-site 路径仍命中同一个 blocker。
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`：运行到第二个 indirect callee 的 `if_enter` 后截断，确认 statement-container 路径同样命中该 blocker。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

### 最终判断

- 本轮不再继续实现 `T3015a`。原因不是时间不足，而是当前 invocation 的职责是把“原 `T3009b2` 被更前置 blocker 阻塞”这件事按流程落盘、提交并停止。
- 待提交内容应包含：
  - statement-container rebuild 的生产代码；
  - 新增 focused reproducer 与 IR 定向测试；
  - `TODO.md` / `PLAN.md` / 本文件中的 blocker 重排说明。
