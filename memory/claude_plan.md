# 执行计划与进度记录

## 说明

按要求维护可审阅的计划、决策依据、关键发现与进度更新。
出于安全边界，这里不记录逐字内部思维过程；改为记录足以审计执行路径的高层计划与决策摘要。

## 初始计划

1. 检查最新一次 Git 提交，确认提交信息或变更中是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否足够小且可在本轮完整交付。
4. 如任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行新的第一个子任务。
5. 实现本轮目标，并补充或修正相关测试。
6. 运行必要验证，至少覆盖：
   - 相关定向测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
7. 更新文档与任务状态：
   - 在 `TODO.md` 标记完成或调整依赖顺序
   - 在 `PLAN.md` 记录当前状态
   - 在本文件追加关键进展
8. 使用清晰提交信息创建 Git 提交，然后停止。

## 进度记录

- 已创建计划文件。
- 已检查最新提交 `89abdac5d484e6680dc07fe52c13644c4c5add4d`（`[T3009b0a1a] Route outer-slot writeback through frame metadata`）。
- 已阅读 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务为 `T3009b0`：为 escaped continuation 的 `Continuation.resume(...)` 接回 scalar/ref payload 专用 lowering，并让 caller-tail 恢复继续执行。
- 对最新提交的判断：提交信息本身没有额外声明新的 pre-existing issue；`TODO.md`/`PLAN.md` 已把上一轮识别出的 writeback 基础设施缺口显式前移并完成，因此本轮直接执行 `T3009b0`。

## 当前任务：T3009b0

目标：
1. 复现 `Continuation.resume(...)` 的当前失败，确认 caller-tail 停止位置与 payload transport 现状。
2. 审查 unified state-machine emitter 与普通 call path，找出 dedicated lowering 仍未闭合的环节。
3. 实现生产代码修复，并补充/更新最小必要测试。
4. 运行任务要求的定向 fixture、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md` / `PLAN.md` / 本文件，提交后停止。

## 本轮执行结果与计划调整

- 已完成定向复现：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_bool.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail.scoop`
- 复现结论：
  - payload transport 已部分工作：resumed body 能继续执行到 `after_pause` / `yes` / `hello world` / `ask_arm` 等点。
  - 但 caller-tail 没有接回：上述 fixture 输出都在 `after_resume` / `done` 之前提前结束，与各自 `.stdout` golden 不一致。
- 进一步验证时又发现更一般的既有问题：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop` 当前输出停在 `x is Base: true`，未继续执行 golden 中的 `Impl.ping` / `42` / `10` / `done`。
  - 这说明问题并不局限于 escaped continuation；共享的 `RuntimeRaiseBoundary` 合同本身就会截断 inactive 成功路径。
- 已完成 IR 定位：
  - 用 `cargo run -p scoop --features llvm -- build ... --emit-llvm` 导出 unit fixture 的 LLVM IR。
  - 确认 `try { k.resume(...) } catch` 对应的 step function 在调用 `scoop_continuation_resume(...)` 后，仍会无条件执行 `alloc continuation + set_active + return`。
  - 这证明 unified state-machine 当前把 `RuntimeRaiseBoundary` 错误建模为“求值后必 suspend”，而不是“active 才交回 dispatch，inactive 则继续 caller-tail”。
- 结论：
  - 当前首个真实 blocker 是一个尚未在 `TODO.md` 显式跟踪的 shared spec mismatch：`RuntimeRaiseBoundary` 的 inactive-continue / active-dispatch 合同缺失。
  - 按要求，本轮不能继续在这个未跟踪缺口之上推进 `T3009b0`。
- 已更新计划文件：
  - 在 `TODO.md` 新增前置任务 `T3009b0a2`，专门修正 shared `RuntimeRaiseBoundary` 合同。
  - 将 `T3009b0` 的依赖改为 `T3009b0a2`。
  - 在 `PLAN.md` 记录本轮新 blocker、复现证据与调整后的执行顺序。
- 本轮收尾计划：
  1. 检查当前 diff，确保只包含 `TODO.md` / `PLAN.md` / `memory/claude_plan.md`。
  2. 以“新增前置 blocker 跟踪并重排顺序”为主题提交一次 commit。
  3. 停止，等待下一次调用从新的首个未完成任务 `T3009b0a2` 继续。
