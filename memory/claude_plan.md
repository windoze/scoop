# Claude Plan

## 执行原则

- 先读取 `TODO.md`，按文件顺序识别第一个标题未带 `[DONE]` 的任务。
- 只完成第一个未完成任务；完成、验证、记录并提交后停止。
- 不做开放式历史问题扫描；仅处理当前任务相关阻塞或验证中暴露且未排期的失败。
- 不写入私有思维链；本文件记录可审查的执行计划、关键决策、进度和验证结果。

## 初始执行计划

1. 读取 `TODO.md`，确定第一个未完成任务及其要求、依赖、验证条件和完成记录格式。
2. 查看最近提交，判断是否明确提到与该任务直接相关的未完成问题。
3. 根据任务范围读取相关源文件、测试和规范材料，避免无关 triage。
4. 如存在阻塞当前任务的缺失功能、规格不匹配或未排期失败，最小化更新 `TODO.md` 记录前置任务并提交后停止。
5. 如可直接执行，则做最小正确实现，补充或调整相关测试/fixture。
6. 按要求运行格式化、lint、相关测试；必要时运行完整测试与 fixture 套件。
7. 更新 `TODO.md`：在任务标题前加 `[DONE]`，填写完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
8. 检查 git 状态与 diff，提交本次任务相关的全部变更，然后停止。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-3.md`：第一个未完成任务是 `T3-04K：收口 T3-04R 十一次审查发现的 source-payload ctor / reflection / intrinsic / MIR fact synthesis / verifier / gate 残余缺口`。
- `T3-04K` 依赖 `T3-04J`；`TODO-3.md` 中 `T3-04J` 已标记 `[DONE]`。
- 本次执行单元是完成 `T3-04K`，验证后在 `TODO-3.md` 标记 `[DONE]` 并提交，然后停止，不进入 `T3-04R`。

## T3-04K 执行计划

1. 检查最新提交与工作树状态，确认是否有与 `T3-04K` 直接相关的未提交/未完成工作；不回滚或触碰无关改动。
2. 围绕任务列出的实际 helper 和模式进行定向搜索：`class_ctor_source_selection`、`synthesize_class_ctor_arg_mapping`、`current_class_ctor_source_call_contract`、`source_class_ctor_call`、`legacy_reflection_arg_ty`、`published_or_builtin_named_intrinsic_entry_name`、`unique_published_hir_direct_exact_root`、`source_signature_target_from_abi_contracts`、`collect_direct_call_source_signature_facts`、`AbiMangler.fun_symbol(&declaration_key)`、`published_value_box_member_impl`、unknown-target `filter_map`、`BodylessDirect` / `DynamicFallback` verifier escape。
3. 阅读命中实现，区分生产 fallback、合法已发布 fact 消费、测试辅助和 dependency gate 守卫；只修改阻塞 `T3-04K` 完成条件的生产路径或守卫缺口。
4. 将残余 fallback 改为 LIR owner + `SiteId` / target-bound source signature / ABI / dispatch/layout facts 的消费路径；缺 fact 改为 fail-fast，不用 FQN/string/source path+span/唯一候选合成绕过。
5. 补齐 effect/LIR verifier 与 `tools/dependency_gate.py`，确保任务列出的残余 helper、等价模式和 unknown target 静默降级不能回归。
6. 更新必要的单测、fixture golden 或新增回归覆盖；如果验证暴露未排期且相关的失败，优先修复，不能完成则在 `TODO.md` / `TODO-3.md` 增加最小前置任务并提交后停止。
7. 按要求运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
8. 验证通过后更新 `TODO-3.md` 中 `T3-04K` 标题为 `[DONE]`，填写完成记录；如 `TODO.md` 子计划状态未变则不改 `PLAN.md`。
9. 检查 `git status`、`git diff`、`git log --oneline -10`，仅提交本任务相关文件，提交信息使用 `[T3-04K] ...`，然后停止。

## T3-04K 进度记录

- 计划已更新，尚未开始代码修改。
- 已检查最新提交与工作树：最新提交 `ade99b42 [T3-04R] Add T3-04K review prerequisite` 与当前任务直接相关；当前仅有本计划文件修改，另有未跟踪 `FACT_REFACTOR.md`，本次不触碰该无关文件。
- 已完成定向搜索，确认仍需修改的生产残留包括：`class_ctor_source_selection` / `synthesize_class_ctor_arg_mapping` ctor 合成，`current_class_ctor_source_call_contract` / `source_class_ctor_call` path/span 查询，`published_or_builtin_named_intrinsic_entry_name` FQN intrinsic fallback，`unique_published_hir_direct_exact_root` source-call/source-signature scan，`source_signature_target_from_abi_contracts` / `collect_direct_call_source_signature_facts` MIR backend fact 合成，`published_value_box_member_impl` value-box 文本拼装，以及 dispatch unknown target `filter_map` 静默丢失。
- 已完成第一轮代码收口：删除 ctor 参数映射合成与 P6 path/span ctor 查询；named intrinsic 不再使用 FQN fallback；HIR direct call 不再扫描 source-call/source-signature facts；MIR backend facts 不再合成 direct-call source signature / declaration ABI / named intrinsic fallback；value-box 改从发布的 source signature + ABI target facts 选择；dispatch unknown target 改为 fail-fast；dependency gate 已补充对应旧 helper 名称守卫。
- 验证中发现删除 MIR source-signature target 合成后，bodyless scalar/delegate/string-substrate 与 declaration-only interface dispatch 需要显式 target-bound publication；已改为从 source callable identity、HIR declaration identity 或 builtin substrate identity发布 target-bound source signature/ABI，并让 dispatch 先验证 bodyless candidates 具备 source target facts再处理实例候选。
- 已补齐最后一轮 fixture 暴露的 source-payload ctor、reflection、named intrinsic、value-box 和 generic direct-call兼容发布缺口。
- 最终验证已通过：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`cargo build -p scoop -p scoopc`；`python3 tools/dependency_gate.py`；`python3 tools/spec_fixtures.py check`；`python3 tools/run_fixtures.py`（1664 checks）。
