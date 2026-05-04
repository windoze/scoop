执行计划（公开摘要）

约束说明：用户要求记录完整思考过程；本文件记录可公开的执行计划、依据、决策点和进度，不记录私有推理链。

目标：完成 `TODO.md` 索引指向的第一个未完成详细任务，然后提交并停止。

步骤：
1. 读取 `TODO.md`，仅把它作为索引，确定详细任务文件的访问顺序。
2. 按索引顺序读取相关 `TODO-Px.md`，以详细文件中的标题是否带 `[DONE]` 判断第一个未完成任务。
3. 检查最新提交信息；如果它明确提到与当前任务直接相关的未完成问题，将其纳入当前任务或作为必要前置任务处理。
4. 阅读当前任务的完整要求、依赖、约束和验证方式。
5. 若任务可直接完成，做最小正确实现并添加或更新必要测试。
6. 若遇到阻塞且不能按规格正确实现，新增最少数量的前置任务到对应 `TODO-Px.md`，同步 `TODO.md`，提交后停止。
7. 运行与改动相关的测试；必要时运行更广泛的验证，修复由当前任务引入或阻塞当前任务的问题。
8. 更新任务完成记录：在详细任务标题加 `[DONE]`，同步 `TODO.md` 中对应条目的 `[DONE]` 状态，按需更新完成记录；仅当阶段计划真的变化时更新 `PLAN.md`。
9. 检查工作树，提交本次任务涉及的全部未提交文件，提交信息使用任务编号和简明描述。
10. 停止，不继续下一个任务。

当前状态：已读取 `TODO.md` 与 `TODO-P6-part3.md`；第一个未完成详细任务确认为 `P6-T04`。最新提交为 `[P6-T03R] Review clean LLVM body lowering`，其完成记录明确允许进入 `P6-T04`，未发现需要先插入的直接遗留前置任务。

进度更新：四个 `P6-T04` 指定 Rust 过滤测试当前均为 0 tests。代码审计确认主要缺口是 refactor frame/continuation 仍由 raw `malloc` 分配，且内部 refactor managed call 未统一经过 root-preserving 边界；下一步实施 typed GC allocation、type descriptor、explicit root slot 与测试入口。

进度更新：已实现 typed GC allocation/root tracking/write barrier；四个 `P6-T04` Rust 测试入口已补齐并通过。指定 build fixture 与 moving-GC runtime fixture 已通过。期间修复了两个直接阻塞项：handle arm completion payload source 现在跨 arm region 查找真实完成值；builtin `String.concat` 在 facts 和 refactor body 中按 pure runtime primitive 处理，避免错误回落 dynamic fallback。自包含 handle 的 escaped continuation 现在也会触发 compiler-generated runtime-error upper bound，以支持 double resume ordinary error case。

进度更新：`cargo fmt --all` 与 `cargo clippy --all-targets -- -D warnings` 已通过。`TODO-P6-part3.md` 与 `TODO.md` 已同步标记 `P6-T04` 为 `[DONE]`，完成记录已填写；下一步检查工作树并提交。

P6-T04 执行子计划：
1. 检查当前工作树，记录已有未提交文件，避免覆盖用户改动。
2. 阅读 P6-T04 相关实现：refactor LLVM body/layout/types、GC/runtime/stackmap 支撑、runtime ABI allowlist 与相关 fixture。
3. 先运行 P6-T04 指定的定向测试，确认当前缺口和失败模式。
4. 按失败与代码审计结果实现最小正确改动：GC header/root metadata、stackmap/root 写回、dropped continuation/runtime error/Managed ABI 边界、legacy runtime 调用守卫。
5. 补齐或更新 P6-T04 指定测试入口与 fixture。
6. 运行 P6-T04 全部验证命令和 `cargo clippy --all-targets -- -D warnings`。
7. 更新 `TODO-P6-part3.md` 与 `TODO.md` 的 `[DONE]` 状态和完成记录。
8. 提交本次任务所有相关改动并停止。
