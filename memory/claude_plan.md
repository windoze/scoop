# 当前执行计划

说明：按系统安全约束，这里记录的是可审计的执行计划、判断依据和进度日志，不包含不可审计的逐字内部推理。

## 初始计划

1. 检查最新一次 git 提交的信息，确认是否显式提到任何已知遗留问题。
2. 阅读 `TODO.md`，找出第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务的上下文、依赖和现有拆分情况。
4. 如首个未完成任务过大或存在前置缺口：
   - 将任务拆分为更小子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的排序与依赖；
   - 本轮只执行拆分后的第一个子任务，或在被前置问题阻塞时仅提交计划调整。
5. 实现本轮目标任务，必要时补充或调整代码结构。
6. 运行相关验证：
   - 优先运行与改动直接相关的测试；
   - 如任务影响范围足够大，再补充更广的测试；
   - 按要求检查无警告构建与 `clippy` 状态。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记完成；
   - 在 `PLAN.md` 中记录当前状态；
   - 持续更新本文件中的进度日志。
8. 提交本轮改动，提交信息与任务编号对齐。
9. 停止，不继续下一个任务。

## 进度日志

- 已创建本文件并写入初始计划。
- 已检查最新提交 `13fd69b219ebe3afd71a2309bc2891fb8ff3a07d`（`[T3016b0R] Fix nested when consumer replay`）。提交信息未声明新的未修复遗留问题；当前工作区仅有本文件变更。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T3016b`：修正 escaped continuation resumed-body tail replay 在 block/if/while mixed direct+indirect 路径中的 prefix 丢失回归。
- 当前判断：`T3016b` 已在前一轮拆分后具备独立边界，先不继续拆分；先按任务验收项定向复现四条 fixture，再根据复现结果决定是否需要补充子任务。
- 下一步：
  1. 直跑 `T3016b` 的四条验收 fixture，记录失败模式是否一致。
  2. 阅读 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`state_machine_segments.rs`、`state_machine_transform.rs` 中与 `resume_path` / resumed-body rebuild / materialization 相关逻辑。
  3. 在统一 state-machine 合同内修复 prefix replay 缺口，补定向测试，再跑任务要求的验证。
- 已完成四条 fixture 的定向复现与期望对比：
  - `effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`、`..._if_multi.scoop`、`..._direct_indirect_while.scoop`、`..._while_multi.scoop` 均表现为“第二次（或后续）`resume(...)` 直接恢复间接 callee，自身 resumed-body segment 中 direct 之后、indirect 之前的 prefix 没有 replay”。
  - 具体现象：输出缺少期望中的 `after_direct` / `label` / `first` 等 prefix 行，然后直接进入 `fetch_resume`。
- 当前实现判断：
  - 不是 `fetch`/ordinary callee 自身恢复坏了；`fetch_resume` 与 indirect tail 本身是能恢复的。
  - 根因在 escaped continuation：site2 逃逸出来的 continuation 仍保留默认 `resume_state = after-site`，后续 `k.resume(...)` 直接跳回 site2 的 after-site，而不是先回到“当前 resumed-body segment 的 replay 入口”。
  - 仅把 continuation 的 resume target 改成 owner state 不够，因为 owner state 头部往往还有前一个 site 的 `ResumeAfterSite`，会错误消费当前新的 resume payload。
- 修复方案（准备实施）：
  1. 在 plan 阶段为“以 `ResumeAfterSite` 开头、并再次 suspend 的 resumed-body state”生成一个专供 escaped continuation 使用的 replay state：它复用当前 state 的后缀，但去掉开头那个旧 site 的 `ResumeAfterSite`。
  2. 给 suspend-site 元数据增加可选的 `escape_resume_target`，并贯穿 plan → segments → unified machine 合同。
  3. `EscapeContinuation` arm 在把 continuation 绑定给局部 `k` 前，根据当前 continuation 的 `resume_state_tag`，若命中带 `escape_resume_target` 的 site，则把 tag 改写成该 replay target；`ImmediateResume` arm 保持现有 after-site 语义不变。
  4. 重新计算 capture sets，并补结构测试锁定 replay target 与 continuation retarget 合同。
- 已完成实现：
  - `state_machine_plan.rs`：为 suspend-site 增加显式 `owner_state` 与可选 `escape_resume_target`；在 `materialize_resume_fragments()` 后生成 synthetic escape replay state，并重算 capture sets。
  - `state_machine_segments.rs` / `state_machine_transform.rs`：把 `escape_resume_target` 贯穿到 segment/unified contract，同时允许同一 suspend site 同时拥有正常 owner state 和 escape replay state。
  - `state_machine_emitter.rs`：`EscapeContinuation` arm 绑定 continuation 时按当前 `resume_state_tag` 重定向到 `escape_resume_target`；真正 replay `SuspendCall` 前若存在 captured callee suspend state，则先把当前 resume payload 写回 callee state。
  - fixtures：将 `effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`、`..._if_multi.scoop`、`..._while_multi.scoop`、`effect_multi_escape_direct_indirect_while.scoop` 改回 `EXPECT: pass`。
  - tests：新增结构测试 `source_plan_assigns_escape_replay_target_for_mixed_direct_indirect_call_site`。
- 已完成验证：
  - `cargo check -p scoopc`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_if_multi.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_while_multi.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_direct_indirect_while.scoop`
  - `cargo test -p scoopc source_plan_assigns_escape_replay_target_for_mixed_direct_indirect_call_site -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已同步更新 `TODO.md` / `PLAN.md`，本轮目标任务 `T3016b` 已标记完成；下一项为 `T3016bR`。
- 已完成提交：`[T3016b] Replay mixed direct-indirect continuation prefix`（`d280808`）。
- 当前工作区已清理完毕，`git status --short` 为空。
- 本轮按要求到此停止，不继续执行 `T3016bR`。
