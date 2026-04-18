## 本轮工作思路摘要

目标是严格按 `TODO.md` 的顺序只完成一个任务，并在完成前先排查最近一次提交里是否提到了遗留问题。执行中如果发现任务依赖缺失、实现边界不完整、或与规范不一致，不做绕过，而是先把阻塞项拆成新的前置任务，更新 `TODO.md` 和 `PLAN.md` 后提交并停止。本文件用于记录本轮计划、关键决策和进度状态；其中写的是结构化工作笔记与执行依据，不是逐字内部思维转录。

## 执行计划

1. 查看最新一次 Git 提交的提交信息与改动，确认是否提到了需要先修复的已有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md` 与相关上下文，判断该任务是否足够小、是否存在显式或隐式依赖。
4. 如果任务过大或被缺失能力/规范问题阻塞：
   - 拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 调整 `TODO.md` 中任务顺序与依赖；
   - 记录原因并提交，然后停止。
5. 如果任务可直接执行：
   - 阅读相关代码、规范和测试；
   - 完整实现该任务；
   - 补充或调整测试，确保行为符合规范而不是依赖临时性方案。
6. 运行必要验证：
   - `cargo fmt --check` 或 `cargo fmt`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如任务相关，补充运行特定夹具/工具命令
7. 更新文档与计划状态：
   - 在 `TODO.md` 标记当前任务完成；
   - 更新 `PLAN.md`；
   - 在本文件记录关键进展和任何计划调整。
8. 检查工作区状态，整理提交内容，创建单个清晰的 Git 提交，然后停止，不继续做下一个任务。

## 进度记录

- 2026-04-18：已初始化本轮计划文件，尚未开始仓库检查。
- 2026-04-18：已检查最新提交与最近提交历史。`HEAD` 的最新提交 `[T3016b] Record final execution log` 只改了 `memory/claude_plan.md`，未在提交信息中引入新的待修已有问题。
- 2026-04-18：已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务是 `T3016bR`（review 任务），不需要继续拆分。
- 2026-04-18：已收敛本次复审的生产代码范围：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_transform.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
- 2026-04-18：已初步确认 `T3016b` 的修复链条是：
  1. plan 阶段为符合条件的 resumed-body state 生成 `escape_resume_target`
  2. segment / unified machine 合同贯通该元数据
  3. emitter 在 `EscapeContinuation` arm 绑定 continuation 时重写 `resume_state_tag`
  4. emitter 在 replay `SuspendCall` 前把当前 resume payload 注入 captured callee suspend state
- 2026-04-18：下一步将执行两类检查：
  - 代码复审：确认上述实现只依赖统一 state-machine / resume-path / suspend-site 元数据，不按 block/if/while 等源码容器形状分流。
  - 运行验证：重跑 `T3016b` 的 4 条 run-pass fixture、对应结构测试，以及 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 2026-04-18：代码复审已完成，未发现需要新增前置任务或回退排序的真实问题。结论如下：
  - `attach_escape_resume_targets()` 只根据“state 首 action 是 `ResumeAfterSite`、且 terminator 仍是 `Suspend`”这一状态机条件生成 replay state，不读取 block/if/while 的源码容器类别。
  - `escape_resume_target` 仅作为 suspend-site 元数据经 plan → segments → unified machine 传递，验证逻辑也仅检查结构一致性。
  - emitter 侧只在 continuation 绑定和 `SuspendCall` replay 前消费这些元数据；没有发现 fixture 名称、源码文本、block/if/while 分支名或 direct/indirect 局部形状硬编码。
- 2026-04-18：验证已完成，结果如下：
  - `cargo test -p scoopc source_plan_assigns_escape_replay_target_for_mixed_direct_indirect_call_site -- --nocapture` 通过。
  - 4 条 mixed direct+indirect run-pass fixture 全部直接运行通过。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 2026-04-18：已更新 `TODO.md` / `PLAN.md`，将 `T3016bR` 标记为完成；下一项待执行任务为 `T3016c`。
