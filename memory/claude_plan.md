## 当前执行计划

说明：按要求先记录可公开的执行计划摘要；不写入内部推理细节。执行过程中如计划调整或关键步骤完成，会继续更新本文件。

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，作为本次唯一执行目标。
2. 检查最近一次提交是否存在与该任务直接相关且明确未完成的事项；若有，将其视为任务范围内内容或在 `TODO.md` 中补成前置依赖。
3. 阅读任务描述、依赖、验证要求，以及必要的相关代码与测试，确认实现边界。
4. 实现该任务；若遇到会阻塞任务达成且未被跟踪的真实缺口，在 `TODO.md` 中以最小必要前置任务形式补入，并停止继续向后推进。
5. 运行任务要求的验证，以及必要的 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`；若失败则先修复再重试。
6. 完成后更新 `TODO.md`：将该任务标题标记为 `[DONE]`，并填写/更新完成记录；仅在阶段计划实际变化时更新 `PLAN.md`。
7. 检查工作区变更，按仓库提交风格创建一次提交，然后停止，不继续下一个任务。

## 进度

- [x] 已写入初始计划
- [x] 已定位首个未完成任务
- [x] 已确认相关上下文与约束
- [x] 已完成实现
- [x] 已完成验证
- [x] 已更新任务记录
- [ ] 已完成提交

## 当前任务

- 目标任务：`G3-T04R`（Review `EffectCtx` / handler graph，确认不再退回 ambient context）
- 直接依据：`TODO.md` 中 `G3-T04` 已标记 `[DONE]`，其后的 `G3-T04R` 尚未标记完成，因此它是当前顺序上的首个未完成任务。
- 下一步：
  1. 查看最近一次提交信息，确认是否存在与 `G3-T04R` 直接相关且明确未完成的事项。
  2. 阅读 `G3-T04R` 指定的实现位置与 `G3-T04` 刚完成的改动面，核对 `EffectCtx` 是否是 continuation / call / handle 的显式输入，outward dispatch 是否从 ctx 链出发，是否还残留 ambient handler stack 语义。
  3. 运行 `cargo check -p scoopc` 与定向 grep，判断本 review 是否仅需补完成记录，还是需要在 review 任务内顺手修复直接否定结论的问题。

## 当前结论摘要

- 已确认工作区内未提交源码改动对应 `G3-T04` 的实现面，属于上次中断后遗留的未提交状态；本次会与 `G3-T04R` 一并原子提交。
- review 结论：未发现会推翻 `G3-T04` 的直接问题。
- 关键观察：
  1. `EffectCtx` / handler node 已作为 managed object layout 落地在 `crates/scoopc/src/llvm/codegen/effect_ctx.rs`。
  2. `effect_lowered/body.rs` 中 `CurrentEffectCtx`、`HandleSavedEffectCtx`、`HandleArmEffectCtx` 已接入 frame，并在 handle body / arm / outward dispatch 路径中显式切换和消费。
  3. outward dispatch 运行时路径使用 `dispatch_handle_boundary_from_ctx(...)` 从 `EffectCtx.handler_top_ref` 扫描 handler node graph；`handle_dispatch_nesting_depth(...)` 仅剩编译期 contract 消歧用途，不是 runtime source of truth。
  4. continuation capture 当前通过“捕获整帧”携带 `CurrentEffectCtx` 与 handle ctx slots；resume 通过恢复 captured frame 继续使用该 ctx。
  5. `cargo check -p scoopc` 的失败前沿仍是后续 G4/G6/G7 缺口，没有回到 G3 范围内的 TLS / handler-context 残余问题。
