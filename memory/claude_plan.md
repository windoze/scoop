# 执行计划

说明：本文件记录可审计的执行计划与进度更新；不记录私有推理细节。

## 当前目标

- 按 `TODO.md` 的顺序完成第一个标题未带 `[DONE]` 的任务，然后停止。
- 若遇到阻塞当前任务的缺陷或缺失功能，先修复；若无法在本次正确完成，则在 `TODO.md` 中加入最小必要前置任务并提交后停止。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其完成标准、依赖和验证要求。
2. 查看最近提交信息，判断是否有直接关联当前任务的未完成事项。
3. 根据当前任务检查必要的代码、测试、规格或夹具，确认实现范围。
4. 进行最小且完整的实现；若任务被具体前置问题阻塞，更新 `TODO.md` 以显式记录前置任务和依赖。
5. 按要求更新或新增测试/夹具，避免规避规格或只为测试通过的特殊处理。
6. 依次运行格式化、lint、相关测试；需要时运行完整测试与夹具套件。
7. 将完成的任务标题在 `TODO.md` 中加 `[DONE]`，更新完成记录；仅在阶段计划真实变化时更新 `PLAN.md`。
8. 检查工作区差异，提交本次任务相关所有改动。
9. 停止，不继续下一个任务。

## 进度

- 已创建初始执行计划，下一步读取 `TODO.md` 选择当前任务。
- 已识别当前任务：`TODO-3.md` 的 `T3-04A`。最近提交 `[T3-04R] Schedule fallback guard follow-up` 直接指向该任务，因此纳入当前范围。下一步审阅 P6 LLVM、intrinsic metadata、LIR verifier 与 dependency gate 的相关代码。
- 已完成首轮实现：P6 handoff 改为携带 HIR 发布的 intrinsic call contract，移除 LLVM handoff/codegen 中的 `top_level_fun_call_sites` 与 source-span dispatch side table；direct/effect-lowered/MIR 调用路径改为消费 published intrinsic call contract；LIR verifier 增加 CandidateSet、dispatch candidate、vtable/itable target 的 source signature/ABI 校验；LLVM reachability 改为 declaration-only target 可达且不再静默跳过 unpublished candidate；dependency gate 已补充相关残留守卫。下一步运行格式化和编译检查并修复错误。
- 验证进展：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py` 已通过。`python3 tools/run_fixtures.py` 仍失败，剩余阻塞集中在 source-body/class-ctor legacy scalar 与 `ToString.toString` call metadata 未结构化发布，示例包括 `run_pass_cone/cross_file_ctor_named_default_basic` 输出 `:` 而非 `10:7`/`10:9`。已在 `TODO-3.md` 插入前置任务 `T3-04A0`，当前 `T3-04A` 保持未完成并依赖该前置任务。
