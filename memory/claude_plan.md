# 执行计划

## 约束与执行原则

- 本轮只处理 `TODO.md` 中第一个未完成任务；若发现其依赖缺失或任务过大，则先更新 `TODO.md`/`PLAN.md` 做任务拆分或依赖重排，再停止在当前应执行的第一个子任务上。
- 在继续任何功能开发前，先检查最新提交是否提到必须先修复的既有问题；若存在，则这些问题优先于 `TODO.md` 当前任务。
- 所有变更必须包含实现、测试、文档/计划同步、Git 提交四部分；除非被新的前置缺陷阻塞，否则不接受半完成状态。
- 如遇到规范不匹配、语言能力缺失、实现边界不完整或依赖任务遗漏，不做规避实现；而是把缺口转为新的前置任务，更新 `TODO.md`/`PLAN.md` 后提交并停止。

## 初始步骤

1. 检查最新一次 Git 提交的提交信息与变更上下文，确认是否提到了尚未修复的问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`、必要的仓库说明与相关代码，判断该任务是否可直接完成，或需要拆分为更小的可执行子任务。
4. 若任务需要拆分：
   - 更新 `PLAN.md` 记录拆分后的执行路径与依赖。
   - 更新 `TODO.md`，将原任务替换或扩展为更细的子任务，并保证第一个子任务成为当前轮要执行的事项。
5. 实现当前首个可执行任务，修改相关代码与测试。
6. 运行与该任务相关的验证，至少包括针对性测试；若变更范围触及通用编译/静态检查路径，则补充运行 `cargo test` / `cargo clippy --all-targets -- -D warnings` / 相关夹具测试中的必要子集或全量命令。
7. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成状态、依赖调整、测试结果及剩余风险。
8. 使用清晰的提交信息提交本轮所有变更，然后停止。

## 进度记录

- 已完成：建立本计划文件。
- 已完成：检查最新提交 `cd3e025952e0d2017507093ed0fb3af9f0754826`。提交信息为 `[T2003c0c1a] Support escape sibling non-resuming direct site`，未额外提到需要先修复的遗留问题。
- 已完成：读取 `TODO.md` 与 `PLAN.md`，确认当前第一个未完成任务为 `T2003c0c1b`，即“LLVM 多 arm handle dispatch（escape-continuation + sibling non-resuming，single indirect site）”。
- 已观察到：当前工作树在 `crates/scoopc/src/llvm/codegen/effect.rs` 与本文件上有未提交改动；其中 `effect.rs` 的差异高度聚焦在 `T2003c0c1b` 对应的 indirect mixed-arm lowering。
- 当前判断：先审阅 `effect.rs` 的现有未提交实现，确认它是否已经基本覆盖 `T2003c0c1b`，再决定是补完实现还是先修正设计/测试缺口。
- 已完成：审阅 `effect.rs` 中的 indirect mixed-arm 改动。确认 step trampoline 已接入 sibling non-resuming dispatch，但 source-handle main path 的 `effect_dispatch_nomatch_bb` / `raise_catch_bb` / custom catch blocks 仍未落地，因此任务尚未完成。
- 已完成：补上 `codegen_handle_expr_immediate_resume_with_escape_sibling_indirect` main path 的 sibling non-resuming dispatch / detach / catch lowering，覆盖 `Raise.raise` 与 custom non-resuming 两条路径。
- 已完成：新增 run-pass fixtures：
  - `tests/fixtures/run-pass/effect_resume_mixed_escape_raise_indirect_single_site.scoop`
  - `tests/fixtures/run-pass/effect_resume_mixed_escape_custom_nonresuming_indirect_single_site.scoop`
- 已完成：通过单夹具构建/运行确认两条新增回归的期望输出。
- 已发现并修复：effect `op_tag` 之前按单个 `MainCodegen` 实例局部分配，导致 caller / callee / nested step trampoline 对同一个 effect FQN 产生不同 tag；现已改为整个编译单元共享状态。
- 已发现并修复：第一次补丁把一整段 indirect `effect_dispatch` 块误插入 direct lowering，造成 direct single-site fixture 在 LLVM verifier 上报 “Terminator found in the middle of a basic block”；重复块已删除，并重新验证 direct / indirect 子集。
- 已完成验证：
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`（`fixtures: ok (957)`）
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步：更新 `TODO.md` / `PLAN.md` 为完成状态，整理变更后提交本轮 commit 并停止。
