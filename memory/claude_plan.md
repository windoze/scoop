## 执行计划

1. 读取 `TODO.md`，识别第一个标题未标记 `[DONE]` 的任务，并确认其依赖、验证要求、完成记录约束。
2. 检查最近一次提交是否直接提到与该任务相关的未完成问题；若存在且会影响当前任务，则将其视为当前任务组成部分或在 `TODO.md` 中补充为前置任务。
3. 阅读为完成当前任务所必需的最小范围代码、测试、规范与文档，避免进行无边界问题排查。
4. 实现当前任务要求的改动；若遇到阻塞当前任务的真实缺口或规格不匹配，不采用规避方案，而是在 `TODO.md` 中添加最小前置任务并调整顺序。
5. 运行当前任务要求的验证与必要的回归测试，并修复发现的问题，直到当前任务满足要求或确认被前置阻塞。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划调整。
7. 按要求更新 `TODO.md`：若任务完成，则将任务标题标记为 `[DONE]` 并填写/更新完成记录；若被阻塞，则保持任务未完成并写入新增前置任务与依赖关系。
8. 仅在阶段级计划、依赖结构或完成标准发生变化时更新 `PLAN.md`。
9. 检查工作区变更，按要求提交一次清晰的 Git 提交，包含本次任务涉及的全部未提交文件。
10. 停止，不继续处理下一个任务。

## 进度记录

- 初始状态：已写入执行计划，尚未读取 `TODO.md`。
- 已读取 `TODO.md` 索引，确认首个未完成任务为 `P7-T03S`：修复 GC env 下 explicit-frame stale-root / `ptr poison` blocker。
- 已检查最新提交 `897b82183711a840a0362ca783e8ad3cc83b8e79`，提交主题直接对应 `P7-T03S`，说明当前任务已有部分修复但仍保留 blocker；后续工作需在该基础上继续闭合，而不是跳到 `P7-T04`。
- 已读取 `TODO-P7.md` 中 `P7-T03S` 定义：本轮必须复现 `effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop` 在 `SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1` 下的 `verify-roots` abort，定位 explicit-frame stale-root / `ptr poison` 来源，修复 compiler-owned spill/writeback 或 root-home contract，并保持既有三个 GC env 守护样本不回归。
- 下一步：先查看当前工作树状态，再定向复现 `P7-T03S` 失败并检查相关 LLVM/codegen 路径与现有定向单测覆盖。
- 已复现失败：`effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop` 在第二次 replay 进入 composed call-boundary 前触发 `verify-roots`，runtime 报 explicit-frame invalid roots。
- 已通过 emitted LLVM IR 缩小问题范围：`fetch` direct entry 本身无可达 `ptr poison`；真正可达的 poison 出现在 `main` mixed replay 的 composed continuation materialization 路径，表现为 `refactor_composed_callee_root` / `refactor_cont_root` 等 compiler-generated root slot 只有当前块初始化，没有 entry 级初始化，SROA 后在可达路径上退化为 `ptr poison`，再被同步进 explicit-frame slot。
- 当前修复思路：让 `create_refactor_gc_root_slot(...)` 对底层 entry alloca 做 entry-level null 初始化，避免 compiler-owned root spill slot 在任何路径上未定义；随后补一条 production LLVM 回归，锁定 mixed replay fixture 不再出现 `ptr poison`，再跑任务要求的 GC env 验证矩阵。
- 后续定位确认：真正剩余 blocker 不是 `fetch` direct entry，而是 composed replay 进入 `__scoop_refactor_surface_resume_owner_dispatch__fetch__k0` 之前，owner-dispatch / composed-resume call args 把 continuation 与 payload 长时间保留在 SSA 中，跨 replay prefix/runtime call 后直接拿旧 SSA 发起 surface resume。
- 已完成修复：
  1. explicit-frame 模式下的 compiler-generated refactor GC root slot 改为以 explicit-frame mirror 为 authoritative home，不再依赖单独 stack shadow 的 load/store/track；
  2. `resume_composed_call_boundary_case(...)` 现在会把 composed-resume 的 callee continuation 与 payload 都接入 defer/reload contract，在 replay prefix 之后、真正调用 surface resume 之前从 explicit-frame-backed homes reload，并在调用后清理临时 root homes。
- 已完成验证：`cargo fmt --all`；`cargo test -p scoopc --lib cross_call_escape_resume_roots_do_not_degrade_to_poison_in_explicit_frame -- --nocapture`；`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`；`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`；`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_binder_string_use.scoop`；`cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_arm_nested_handle_replay_tail_basic.scoop`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all`。
- 当前状态：`P7-T03S` 已满足完成条件；下一步只需检查工作树、更新 `TODO.md`/`TODO-P7.md` 完成标记，并创建本次任务提交。
