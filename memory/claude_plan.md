# 执行计划

## 约束与说明
- 按要求先记录高层执行计划，再开始读取仓库状态和运行命令。
- 这里记录的是可审阅的执行计划与进展摘要，不包含内部详细推理。
- 本次调用目标：先检查最新提交是否提到需要先修复的问题；再定位 `TODO.md` 中第一个未完成任务；只完成一个任务后停止。

## 初始步骤
1. 查看最新一次 Git 提交，确认提交说明或内容中是否提到了需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`、相关代码与测试，判断该任务是否可以在本轮完整完成。
4. 如果任务过大，则把它拆分成更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`；本轮只执行拆分后的第一个子任务。

## 执行步骤
1. 实现目标任务所需代码修改。
2. 运行相关测试、格式化、`clippy` 或其他必要检查；若发现问题立即修复。
3. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞原因。
4. 使用清晰的提交信息提交本轮更改。
5. 停止，不继续处理后续任务。

## 进展
- 已创建计划文件，下一步开始检查最新提交与任务列表。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`。最新提交没有留下独立于任务列表之外的额外未修问题；本轮首个未完成任务确认为 `T3010b2b1b`。
- `T3010b2b1b` 是当前 effect 主线的前置 blocker，目标是修复 unified state-machine 路径在 nested arm indirect call 中命中 `value coercion` 的 expected-context / coercion 缺口。
- 下一步：先复现 `tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 的失败，再定位 `state_machine_emitter` 与 expected-context/coercion 相关代码，判断是缺失上下文传递、错误 fallback，还是遗漏了统一读取路径。
- 已复现目标失败：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 稳定报 `暂不支持的 main 代码生成节点：value coercion`。
- 已定位到统一 emitter 的两个缺口：
  1. `HandleStateOp::DeclareAnonymousVal` 没有像普通 `BindLocal` 一样走 `codegen_initializer_expr(target_ty, target_hir_ty)`，匿名绑定会丢失声明类型提供的 expected context。
  2. `emit_state_ops` 里语句与 tail value 共用 `last_value`，但 `val/assign` 没有显式把语句结果收口为 `Unit`，同时 state 的最终产出值也没有按 terminator 统一附带 expected context。
- 计划中的代码修改：
  1. 为 state op 发射增加“该 state 最终产出值的 expected type”辅助逻辑，只在最后一个真正产出值的 op 上施加。
  2. 让匿名 `val` 初始化复用与普通 `val` 相同的 initializer codegen 路径。
  3. 将 `BindLocal` / `DeclareAnonymousVal` / `Assign` 等语句 op 的 `last_value` 显式设为 `Unit`，与普通 block 语义对齐。
- 继续定位后确认：当前任务被一个更前置、且原 `TODO.md` 未显式跟踪的 bug 阻塞。inner handle 的 synthetic resume slot `__resume_site0` 与 outer local 复用了同一个 `SymbolId`；进入 inner handle 时，`seed_outer_scope_frame_slots` 会把 outer `_ : Unit` 误当成该 synthetic resume slot（期望 `Int`）写入 frame，先于真正的 unified expected-context 逻辑触发 `Unit -> Int` coercion。
- 已按规则停止进一步代码修改，准备只更新 `TODO.md` / `PLAN.md` / 本文件，前移新的 blocker 任务，并提交文档性变更后结束本轮。
