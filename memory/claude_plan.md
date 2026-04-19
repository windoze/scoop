# 本轮执行计划（摘要版）

## 目标

按照 `TODO.md` 的顺序完成第一个未完成任务；如果遇到阻塞，则按要求改写 `TODO.md` / `PLAN.md` 并提交后停止。

## 已知约束

- 先检查最新提交是否提到已知问题；若有，需优先修复。
- 在开始实现前，需要确认第一个未完成任务是否过大，必要时拆分为更小子任务。
- 本轮只完成一个任务，然后提交并停止。
- 过程中的计划变化与关键进展需要持续更新到本文件。
- 输出与记录使用中文。

## 执行步骤

1. 查看最新一次提交信息，确认是否提到需要先修复的问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并理解上下文与依赖。
3. 如任务过大，先拆分任务并更新 `TODO.md` / `PLAN.md`，随后只处理拆分后的第一个子任务。
4. 阅读相关代码、测试与规范，确认实现边界，避免以临时绕过方式完成任务。
5. 实现该任务所需代码修改，并在必要时补充或调整文档/注释。
6. 运行相关格式化、测试与质量检查，至少包含与改动相关的测试；若任务范围允许，再运行更全面的检查。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 使用清晰的提交信息创建一次 git commit，然后停止。

## 进度记录

- 已创建本轮计划文件，下一步将检查最新提交与任务列表。
- 已检查最新提交 `e7f3304 [T4008c1] Support multi effect type param instantiation` 的完整提交信息；提交消息只有标题，没有额外正文，也未附带新的“先修复遗留问题”说明。
- 已阅读 `TODO.md` / `PLAN.md` 并定位首个未完成任务为 `T4008cS`：支持 effect state-machine 的多实参 `perform` payload lowering。
- 当前理解：
  - 现有缺口在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs::emit_perform_op`，state-machine 路径只接受 0/1 个 payload。
  - 该缺口会直接阻断后续 `T4008c2` 的 receiver effect op，因为 receiver 会占用第 0 个 payload 槽位。
  - 本轮预期需要同时覆盖普通 handle、immediate-resume、escape continuation 三条执行路径的回归。
- 下一步：
  1. 阅读 state-machine emitter / plan / lowering 中与 `perform` payload、handler arm binder 读取相关的代码。
  2. 构造最小复现，确认失败点是否仅在 emitter，还是还涉及 handler dispatch / arm binder storage。
  3. 实现修复并补充最小但足够覆盖的 run-pass / regression。
  4. 运行定向测试，再视耗时补全全量 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 关键发现：
  - 最小 probe `handle { Edge.visit(1, 2) } with { Edge.visit(from, to) -> ... }` 当前直接报 `scoop::llvm::unsupported_main_body: state machine perform arity`。
  - 进一步 probe `fun go(): Int / Edge { return Edge.visit(3, 4) }` + 外层 `handle { go() } ...` 后，程序实际输出 `3`、`3` 并返回 `6`，说明普通 `perform` lowering 也只保留了第 0 个实参。
  - 这表明问题不是单独的 state-machine emitter 缺口，而是“共享普通 perform transport + state-machine perform/binder lowering”两层缺口叠加。
- 计划调整：
  - 按仓库规则，本轮不能继续只修 `T4008cS`，否则会留下“普通 effectful callee -> 外层 handle 捕获”仍然错误的 spec mismatch。
  - 已决定先在 `TODO.md` / `PLAN.md` 中插入新的前置任务 `T4008cP`，专门收口普通 `perform` 的多实参 payload transport；原 `T4008cS` 顺延为其后续任务，只负责 state-machine 自身路径。
  - 本轮到此将只提交任务重排与 blocker 记录，不再继续实现代码。
