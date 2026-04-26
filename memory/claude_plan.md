## 当前计划（初始化）

约束说明：
- 按用户要求，本文件在任何 shell 命令执行前先建立。
- 这里记录的是可审计的执行计划与关键判断摘要，不包含逐字内部推理。

目标：
- 只完成 `TODO.md` 中第一个未完成任务，然后停止。

初始步骤：
1. 查看最新一次 git 提交，确认是否明确提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有分解、依赖和阶段状态。
4. 检查工作区状态，识别是否存在与当前任务直接冲突的未提交修改；若有，谨慎避让，不回退用户改动。
5. 评估该任务是否过大：
   - 若可直接完成：继续实现。
   - 若过大：先把任务拆分，更新 `PLAN.md` 和 `TODO.md`，然后只执行拆分后的第一个子任务。
6. 实现当前目标任务，同时留意任何既有缺陷、规约不匹配或实现边界问题。
7. 运行与改动相关的测试，并补齐必要测试。
8. 更新 `TODO.md`、`PLAN.md`、本文件，记录完成情况或阻塞依赖。
9. 提交 git commit，提交信息对应当前任务编号与内容。
10. 停止，不继续处理下一个任务。

执行原则：
- 如果在探查、测试或实现时发现既有问题，必须优先修复，或将其作为前置任务插入 `TODO.md` 后停止。
- 不接受绕过实现、不接受夹具特判、不接受规约偏离。
- 编辑前先读取上下文；编辑后必须验证。

状态：
- 已完成：初始化计划写入。
- 已完成：检查最新提交；未发现提交信息中显式声明必须先修复的新既有缺陷。
- 已完成：定位第一个未完成任务为 `T5000e3d`。
- 已完成：评估 `T5000e3d` 规模；当前判断无需再拆分 `TODO.md` / `PLAN.md`。
- 已完成：运行定向与全量编译/测试，确认当前变更集已通过 `cargo test -p scoopc`、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 已完成：确认并收口当前任务的核心问题：
  - compilation-unit / LLVM frontend lowering 已把非 intrinsic direct-call callee 物化为实例 FQN；
  - LLVM special-case dispatch 仍有一部分只按模板名建模；
  - 需要把 backend 边界收口为“普通静态 direct-call 直接消费实例身份，少数 sysroot/builtin/vtable/itable special-case 只做窄模板名归一化”，而不是恢复 backend 现场猜测。
- 已完成：验证当前代码已删除 `try_resolve_monomorphized_*` 主路径，并以 `materialize_direct_call_targets` 模式位把实例目标物化限制在 compilation-unit / explicit-instance lowering，typed dump / generic-template lowering 继续保留 template target。
- 已完成：补齐并验证回归测试，覆盖 compilation-unit lowering、typed dump 与 materialized generic sysroot direct-call builtin dispatch。
- 已完成：更新 `TODO.md`、`PLAN.md` 与本文件，记录 `T5000e3d` 的实现结果、修复点与验证结果。
- 进行中：按 `PROMPT.md` 要求提交当前改动，然后停止。
- 新发现待修复问题：
  - 无。当前未再发现需要插入到 `T5000e3d` 之前或内部的新前置缺陷任务。

## 当前任务：T5000e3d

任务目标：
- 让 LLVM codegen 直接消费已实例化 target identity；
- 删除 backend 现场按 mangled FQN 猜测 monomorphized target 的主路径；
- 保持 generic template / dump 路径不提前单态化。

关键判断摘要：
1. `TopLevelFunCallBinding` 已由 typecheck 为 standalone/member/extension direct-call 记录最终声明目标与 type/effect 实参。
2. LLVM backend 当前仍在 `codegen_top_level_fun_call_impl()` 中通过
   `try_resolve_monomorphized_member_fqn()` /
   `try_resolve_monomorphized_standalone_fun_fqn()` 现场推断目标。
3. 若把 direct-call callee 改写放到“typed dump / generic-template lowering”中，会破坏 generic MIR template 边界；
   因此只能在 compilation-unit / LLVM frontend 使用的 lowering 路径启用。
4. 仅靠 `source_path + call_span` side table 不足以覆盖 generic caller 的多个实例；
   更稳妥的方式是在生成可 codegen 的 HIR 时，直接把非 intrinsic 且已 concrete 的 direct-call callee 改写为最终实例 FQN。

细化执行步骤：
1. 为 HIR lowering 增加明确模式位：
   - compilation-unit / LLVM frontend lowering：启用 direct-call target materialization；
   - `lower_for_dump` / `lower_typed_for_dump` / generic-template-only：禁用。
2. 在 HIR lowering 中新增 helper：
   - 读取并重放 `TopLevelFunCallBinding`；
   - 对 active type/effect bindings 做替换；
   - 仅当 target 非 intrinsic 且 type/effect 实参已 concrete 时，构造稳定实例 FQN。
3. 在 standalone/member/extension direct-call lowering 位置使用该 helper：
   - 让可 codegen 的 HIR 直接携带最终实例 FQN；
   - 不改写 intrinsic target，避免破坏后端 intrinsic dispatch。
4. 删除 LLVM backend 中 `try_resolve_monomorphized_*` 主路径与相关辅助推断逻辑。
5. 补充测试：
   - compilation-unit lowering 会为 direct-call 写入已实例化 callee FQN；
   - typed dump / generic-template 路径保持 generic callee；
   - LLVM/codegen 回归仍通过。
6. 运行验证并更新 `TODO.md`、`PLAN.md`、本文件，然后提交。

当前修正策略（更新）：
1. 保留 HIR lowering 对普通 direct-call target 的实例 FQN 物化，不回退到 backend 现场猜测。
2. 在 LLVM call dispatch 中补一个“已实例化 direct-call FQN → template/base FQN”的窄归一化 helper：
   - 仅用于 sysroot/builtin special-case dispatch、vtable/itable slot 识别等需要模板名的路径；
   - 普通静态 direct-call 仍使用完整实例 FQN 在 `fun_index` 中命中具体实例。
3. 让现有从手工 lowered HIR 进入 `emit_minimal_main_ir_from_lowered_hir` 的 effect/codegen 测试重新通过，并补一条针对 generic sysroot direct-call 的回归测试。

收尾状态（2026-04-27）：
1. 上述修正策略已在当前工作区实现并通过验证。
2. `TODO.md` / `PLAN.md` 已更新为将 `T5000e3d` 标记完成，下一条任务切换为 `T5000e3dR`。
3. 下一步只剩按 `PROMPT.md` 要求提交当前改动，然后停止。
