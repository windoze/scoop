# 本轮执行计划（摘要）

## 约束
- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始实现任务前，先检查最新提交是否提到已有问题；若有，优先修复。
- 若首个任务过大，需要先拆分并更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
- 任何与规范不符的实现缺口、缺失特性、测试缺陷或运行时问题，都必须先记录为前置任务，不能用绕过方案继续推进。

## 初始步骤
1. 查看最新提交信息，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认现有计划、依赖关系与任务上下文。
4. 结合代码与测试现状，判断该任务是否可在本轮完整落地；若过大，则拆分任务并同步更新计划文件。

## 执行步骤
1. 实现当前目标任务所需的代码修改。
2. 运行相关测试，并补充必要测试。
3. 运行格式化、必要检查以及 `cargo clippy --all-targets -- -D warnings`，确保无告警。
4. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态或阻塞依赖调整。
5. 提交本轮变更，提交后停止，不进入下一个任务。

## 进度记录规则
- 若任务边界、依赖关系或实施方案发生变化，及时更新本文件。
- 若完成关键阶段（定位任务、开始实现、完成测试、准备提交），及时更新本文件。

## 当前进度
- 已检查最新提交：`c843eee7f437e805fa91aa8ae2c10a8d21ad8069` 仅为 `Update plan`，未提到需要先修复的既有问题。
- 已定位 `TODO.md` 中首个未完成任务：`T2003c0c2b3c2-1`，内容为将 `crates/scoopc/src/llvm/codegen/effect.rs` 拆为目录模块，要求纯重构、无语义变化。
- 已确认 `PLAN.md` 的“当前下一步”仍停留在旧的 `T2003c0c2b3c3`，本轮完成任务后需要同步更新计划文件，使其与 `TODO.md` 的新顺序一致。
- 已完成实现：`effect.rs` 已拆为 `effect/` 目录模块，采用 `effect/mod.rs` + `include!` 分片组合的方式保留原有模块作用域与私有 helper 可见性。
- 已完成验收：`cargo fmt --all --check`、`cargo test --all`、`cargo run -p scoop -- test`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。
- 已完成收尾文档同步：`TODO.md` 已将 `T2003c0c2b3c2-1` 标为完成，`PLAN.md` 已记录目录模块化成果并把下一步切到 `T2003c0c2b3c2-2`。
- 当前剩余动作：检查 git diff、提交本轮改动并停止。

## 当前实施细化
1. 检查 `crates/scoopc/src/llvm/codegen/` 目录结构，以及 `effect.rs` 对父模块和同级模块的可见性依赖。
2. 评估 `effect.rs` 的内部结构，设计目录模块边界，尽量按“函数原样搬迁”拆到 `shared`、`scan`、`nonresuming`、`immediate_resume`、`escape_continuation`、`mixed`、`matrix` 或等价模块。
3. 完成模块迁移，并确保 `codegen/mod.rs` 仍可按现有形态引用 `effect::EffectUnwindTarget`、`effect::ImmediateResumeCtx`。
4. 运行格式化、测试、LLVM 端到端与 clippy，验证重构未引入语义变化。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，然后提交本轮改动。
