执行计划

- 先读取 `TODO.md`，按标题是否带 `[DONE]` 判定并定位第一个未完成任务。
- 读取该任务相关说明、依赖、验证要求；必要时查看 `PLAN.md` 与最近提交，确认是否存在直接相关的未完成问题。
- 在理解当前任务后，更新本文件，记录选定任务、实现步骤和验证步骤。
- 实施当前任务，不跳到后续任务；如果遇到阻塞的缺失特性或规格不一致，按要求更新 `TODO.md` 并停止。
- 按要求运行格式化、lint、相关测试以及必要的完整测试/fixture 验证。
- 完成后在 `TODO.md` 中给该任务标题加 `[DONE]` 并填写 completion record。
- 提交本次任务相关改动，然后停止。

进度记录

- 已创建初始执行计划，下一步读取 `TODO.md` 定位第一个未完成任务。
- 已读取 `TODO.md` 与 `TODO-3.md`，第一个未完成任务为 `T3-04R：Review T3-04`。
- 本轮执行范围：审查 `T3-04`、`T3-04A0`、`T3-04A`、`T3-04B0`、`T3-04B`、`T3-04C` 所声称关闭的 fact-only / fail-fast / dependency-gate 边界；若发现仍阻塞完成条件的缺口，新增最小前置任务并停止；若未发现阻塞问题，运行指定验证并标记 `T3-04R` 完成。
- 下一步：检查工作区与最近提交，确认是否有直接相关未完成事项，然后搜索生产代码、verifier、dependency gate 与 fixture/golden 覆盖中的残留 side-table/fallback 路径。
- 已完成针对性审查。发现 `T3-04C` 后仍有阻塞 review 的残留：reflection/class ctor/source call-site metadata 仍可通过 source path + span 或 HIR source-site 映射发布/消费；LIR builder 仍可用 root/declaration key/`AbiMangler.fun_symbol` 合成 declaration/layout/call target ABI；intrinsic facts 仍有 root/FQN fallback；`DynamicFallback`/bodyless direct surface 与 effect verifier 仍可能放行缺 target；P6/MIR 仍有 generic/overload string parsing、dispatch side-table/name+arity 恢复；dependency gate 未 tree-wide 锁住等价路径。
- 处理决定：不标记 `T3-04R` 完成；在 `TODO-3.md` 中新增最小前置任务 `T3-04D`，并把 `T3-04R` 依赖改为 `T3-04D`。本次只提交任务列表与计划记录，然后停止。
