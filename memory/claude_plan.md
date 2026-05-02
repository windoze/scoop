# 执行计划

说明：我不会记录逐字的内部推理细节，但会持续维护一份可审阅的执行计划、关键判断依据与进度记录。

## 初始计划

1. 读取 `TODO.md`，确认它只作为索引使用，并按索引顺序定位对应的详细任务文件。
2. 读取相关的 `TODO-Px.md`，按详细文件中的实际顺序和完成记录，确定第一个未完成的详细任务。
3. 检查最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；如果有，将其视为当前任务的一部分或必要前置。
4. 阅读当前任务涉及的代码、测试、规范或文档，确认实现边界、依赖与验证要求。
5. 如果任务可直接完成：实现改动、补充或更新测试，并运行必要的验证命令（至少覆盖任务要求，并确保无警告通过 `cargo clippy --all-targets -- -D warnings`，若适用）。
6. 如果存在阻塞当前任务的真实缺口：在对应 `TODO-Px.md` 中以最小必要粒度添加前置任务，保持当前任务未完成；同步更新 `TODO.md`；仅在阶段计划发生变化时更新 `PLAN.md`。
7. 在详细任务文件中记录完成情况；如任务索引、标题、顺序或文件引用变化，同步更新 `TODO.md`。
8. 检查工作区变更，确认只提交与本次任务相关的改动；按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进度记录

- 已写入初始计划。
- 已读取 `TODO.md`、`TODO-P0.md`、`TODO-P1.md`、`TODO-P2.md`，确认 `P0` 与 `P1` 全部已有完成记录。
- 当前首个未完成详细任务：`TODO-P2.md` 中的 `P2-T03R`（Review continuation typed 语义，确认没有残留隐藏通道或 legacy 魔法）。
- 最近一次提交主题为 `[P2-T03] Surface continuation typed contracts`，与当前 review 直接相关；下一步需要检查其实现与验证产物，确认是否存在未完成问题或需补充的前置缺陷。

## 当前执行细化

1. 读取 `P2-T03R` 相关代码与测试位置，确认 `Continuation.resume(...)` typed contract、runtime error ordinary effect 传播、以及 compiler-owned `Continuation` 约束的实际落点。
2. 复核运行 `P2-T03R` 要求的测试与搜索，确认 refactor 路径不再依赖隐藏错误通道或 legacy 魔法。
3. 如果 review 发现问题：优先修复；若无法在当前任务内正确修复，则最小化补入前置任务并同步 `TODO.md` / `TODO-P2.md`。
4. 如果 review 通过：在 `TODO-P2.md` 的 `P2-T03R` 下补写完成记录，必要时同步其它文档；然后创建一次 git 提交并停止。

## 最新进展

- 已复核 `sysroot/core.scoop`、`crates/scoopc/src/typecheck/expr/call.rs`、`crates/scoopc/src/typecheck/interfaces.rs`、`crates/scoopc/src/typecheck/expr/error.rs`、`crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs`。
- 结论：`Continuation.resume(...)` 的 payload/answer/effect contract、`Raise<RuntimeError>` ordinary effect 传播、以及 compiler-owned `Continuation` 的“不可实现/不可构造”约束都已在 typecheck 与 refactor typed HIR stage 中显式落地；未发现需要新增前置任务的阻塞缺陷。
- 已完成定向验证：`cargo test -p scoopc --no-default-features refactor_continuation_typecheck`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo test -p scoopc --no-default-features continuation_resume`、`cargo test -p scoop --no-default-features dump_hir`、7 个 refactor typecheck fixture、以及 `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`，均通过。
- 已把 `P2-T03R` 完成记录写回 `TODO-P2.md`；下一步仅需检查工作区并创建本次任务的 git 提交，然后停止。
