# 本轮执行计划

## 约束
- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 若在检查、实现或测试过程中发现已有问题、回归、规范不匹配或实现边界缺口，需要优先修复；若无法当场修复，则在 `TODO.md` 中把该问题加入为前置任务，并更新 `PLAN.md` 后停止。
- 在实现后需要补充/更新测试，运行相关验证，并确保没有新的告警。
- 需要在结束前更新 `TODO.md`、`PLAN.md`，并按仓库约定提交一次 git commit。

## 初始步骤
1. 检查最新提交信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖与现有计划。
4. 如任务过大，则把它拆分为更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`；随后只执行拆分后的第一个子任务。
5. 实现当前目标任务，并在过程中记录任何新发现的问题。
6. 运行与改动直接相关的测试；如有必要，再运行更广的检查命令。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 创建符合仓库风格的提交并停止。

## 当前任务
- `TODO.md` 中当前第一项未完成任务为 `T5002b2b2b`：修复 nested-handle immediate-resume replay-state 穿过 ordinary callee boundary 时的 replay owner 错位。
- 最新提交 `[T5002b2b2a] Preserve ordinary replay token on non-call suspend` 没有引入额外需要先插队处理的新任务；当前已知剩余 blocker 就是 `PLAN.md` / `TODO.md` 中记录的 `T5002b2b2b`。

## 本轮细化步骤
1. 定位 nested-handle immediate-resume 最小路径涉及的 codegen / runtime 边界，确认 ordinary callee token、pending continuation、legacy replay-state 的当前 owner 与穿越顺序。
2. 复现并锁定错误：outer ordinary call boundary 把 legacy replay-state 误当成 callee resume entry token 保存，导致 replay 落到错误对象形状。
3. 在最小范围内修复 replay owner / 保存顺序，使 outer ordinary call replay 只保存真正的 callee resume token，不吞并 nested immediate-resume bookkeeping。
4. 补 focused LLVM 或 run-pass 回归，证明：第一次 `k.resume(...)` 后可再次 outward suspend；第二次 `k.resume(...)` 后继续 inner callee tail，而不是过早返回 outer payload。
5. 运行相关测试与 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md`、本文件，提交后停止。

## 进度记录
- 已创建本轮计划文件，并完成对最新提交、`TODO.md`、`PLAN.md` 的初步检查。
- 已用最小 run-pass 程序稳定复现 nested-handle immediate-resume 链路错误：当前第二次 `k.resume(11)` 会把 `11` 直接当成整个 nested handle 的结果，输出 `after_nested -> 11`、`after_outer_resume -> 16`，同时跳过 `after_boom` / `inner_arm_after_resume`。
- 当前判断：问题直接落在 `ResumeAfterSiteReason::NestedHandleBoundary` 的恢复逻辑上。它目前只会把 frame 中的 `resume_word/resume_gc_ref` 直接绑定到 nested handle 的 resume slot，没有为“inner handle 因 `Continuation.resume(...)` replay 再次 outward suspend”保留/回放 inner replay token。
- 已完成的 partial groundwork：
  - `NestedHandleBoundary` 现在有独立 replay token frame slot，并会在 `ResumeAfterSiteReason::NestedHandleBoundary` 上优先 replay nested token；
  - non-tail escape arm 且 body 仍会 outward suspend 的形状现在会打开 segmented-body 入口；新增分析断言 `non_tail_escape_arm_with_outward_suspend_builds_inner_resume_site` 已确认 inner arm body 递归 nested handle 中能看见 first-class `Continuation.resume(...)` hidden suspend site；
  - 现有 focused 回归 `continuation_resume_answer_replay_basic.scoop`、`effect_resume_nested_escape_handle_tail_multi_perform_nonunit.scoop` 与 `cargo clippy --all-targets -- -D warnings` 均通过。
- 新发现的前置 blocker：arm body 自身的 nested handle / `try { k.resume(...) } catch ...` 仍缺少 source-path / resume-fragment 合同。当前 replay 已能继续到 `after_boom -> 11`，但仍会把 `18` 直接当成 outer arm 结果，跳过 `inner_arm_after_resume` / `resumed + 1`。这说明必须先把该缺口前插为新的 TODO 前置任务，再回到原 `T5002b2b2b`。
- 下一步不再继续硬做当前任务，而是先按用户约束更新 `TODO.md` / `PLAN.md`：新增“non-tail escape arm segmented-body resume-fragment 合同”前置任务，把原 `T5002b2b2b` 顺延到它之后，然后提交并停止。
