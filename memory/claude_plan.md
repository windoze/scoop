# Claude Plan

## 当前目标
- 按 `TODO.md` 的顺序只完成第一个未完成任务，然后停止。

## 初始执行计划
1. 检查最新一次提交信息，确认是否提到了任何遗留问题；若提到，优先修复该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，先拆分任务：更新 `PLAN.md` 与 `TODO.md`，把可执行的第一个子任务放到最前，然后本次只执行这个子任务。
4. 实现当前要做的任务，避免引入规避性方案；如果发现已有缺陷、规格不匹配或缺失能力，先修复，或将其作为前置任务插入 `TODO.md` 并停止。
5. 运行相关测试与必要的质量检查，至少覆盖受影响范围，并根据结果继续修复直到通过。
6. 更新文档状态：在 `TODO.md` 标记任务完成，更新 `PLAN.md` 反映当前进展与后续顺序。
7. 按仓库提交风格创建一次 git 提交，然后停止，不继续下一个任务。

## 约束与原则
- 不回退或覆盖我未创建的现有改动。
- 不用变通、夹具特判或降级行为绕过规格问题。
- 如果任务被前置问题阻塞，则先把前置问题加入 `TODO.md` 的正确位置，并更新 `PLAN.md` 说明原因。
- 代码修改尽量最小化，并在必要时补充测试与简短注释。

## 当前结论
- 原 `T5002b2b` 在执行时拆成两个子问题：
  - `T5002b2b1`：callee resume entry 自身的显式 incoming token ABI / publish 对齐。
  - `T5002b2b2`：resumed ordinary callee 经 nested-handle boundary 再次 outward suspend 时，replay chain 仍会丢失。
- 本次已完成并验证的是 `T5002b2b1`；`T5002b2b2` 已作为新的前置 blocker 写回 `TODO.md` / `PLAN.md`。

## 进度记录
- 已写入初始计划，下一步开始检查最新提交与任务列表。
- 已检查最新提交 `ca766e9 [T5002b2aR] Review ordinary indirect-call token boundary`，提交标题本身未声明需要先修的遗留 bug。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务是 `T5002b2b`：把显式 `incoming_resume_token_ref` 扩到 callee resume entry。
- 已检查相关代码路径：
  - `crates/scoopc/src/llvm/codegen/call/resume.rs`
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
  - `crates/scoopc/src/llvm/codegen/closure/mod.rs`
- 现状判断：ordinary callee replay call 已把 replay token 直接传给 resume helper，但 callee resume entry 仍把该值当“特例 state 参数”消费，且 entry 自身没有按统一 contract 显式 publish incoming token；这会让 ordinary resumed path 继续保留独立 ABI 形状。

## 本次任务执行方案
1. 调整 callee resume entry 的声明、定义与调用 helper，让 hidden 参数语义明确收口为 `incoming_resume_token_ref`，不再在 helper 内部以“特例 state 参数”旁路表达。
2. 在 callee resume entry 入口显式 publish incoming token，再从该 token 读取 callee suspend state 并做 resume-site dispatch，使 ordinary resumed path 和其它 ordinary boundary 的 token contract 对齐。
3. 补 LLVM 回归，至少覆盖：
   - callee resume entry 本体会 publish incoming token；
   - replay call IR 明确把保存的 replay token 作为显式 incoming token 传入 resume entry；
   - resumed tail 继续使用显式 outcome/propagation 路径。
4. 运行定向测试与必要质量检查；若发现既有缺陷，先判断是当前子任务内可闭合，还是必须前插新的前置子任务。
5. 更新 `TODO.md` / `PLAN.md`、提交本次任务，然后停止。

## 最新进展
- `T5002b2b1` 代码已完成：replay helper / resume entry 统一改为显式 `incoming_resume_token_ref` 语义，resume entry entry block 会先 `publish` incoming token 再做 resume-site dispatch。
- 已通过定向 LLVM 回归 `cargo test -p scoopc suspend_ir_stores_callee_resume_token_on_frame_and_replays_via_resume_thunk -- --nocapture`。
- 已通过 `cargo clippy --all-targets -- -D warnings`。
- 在尝试做 nested-handle immediate-resume 的 end-to-end 验证时，确认还存在新的 blocker：第二次 outward suspend 穿过 `NestedHandleBoundary` 后会直接把 outer payload 当成最终 answer，跳过 inner callee tail；该问题已转成新的前置子任务 `T5002b2b2`，本次不继续跨任务线修补。
