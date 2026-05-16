# 当前执行计划

## 目标

完成 `TODO.md` 中第一个标题未带 `[DONE]` 前缀的任务，验证后更新任务记录并提交；完成一个任务后停止。

## 执行步骤

1. 读取 `TODO.md`，按文件顺序定位第一个未完成任务，确认其要求、依赖、验证方式与完成记录格式。
2. 检查最新提交信息，若明确提到与当前任务直接相关的未完成问题，将其纳入当前任务或按要求记录为前置任务。
3. 阅读当前任务涉及的相关代码、测试、fixture 和文档，只围绕当前任务建立必要上下文。
4. 如发现当前任务被具体缺陷、缺失语言特性或规格不匹配阻塞，更新 `TODO.md` 插入最小必要前置任务，保持当前任务未完成，提交后停止。
5. 若无阻塞，实施当前任务所需的最小正确代码与测试改动，不使用 workaround 或弱化规格行为。
6. 运行任务指定验证命令及必要的相关测试；如失败，诊断并修复，直到相关验证通过或确认存在必须记录的阻塞前置任务。
7. 更新 `TODO.md`：在当前任务标题前加 `[DONE]`，补全完成记录、测试命令和关键实现说明；仅当阶段级计划变化时才更新 `PLAN.md`。
8. 运行最终状态检查，提交所有与当前任务相关且当前工作会话产生/继承的未提交文件，提交信息使用任务编号前缀。
9. 停止，不继续处理下一个任务。

## 进度记录

- 已创建本计划文件，下一步读取 `TODO.md` 定位首个未完成任务。
- 已定位首个未完成任务：`P2-T01`（补 `kindOf<T>` / `descOf<T>` 与 `ARRAY_ELEM_KIND_*` 常量）。最新提交为 `[P1-T02] Add automatic prelude imports`，未显示需先处理的相关未完成事项。
- 下一步聚焦现有反射 intrinsic、const eval 与类型 layout/GC kind 判定路径，避免引入 workaround。
- 已完成首轮实现：在 `sysroot/core.scoop` 增加 `kindOf<T>`、`descOf<T>` 与三项 `ARRAY_ELEM_KIND_*` 常量；在 const 解释器中接入 `kindOf/descOf`；在 typed intrinsic 分类和 MIR lowering 中接入运行期调用折叠；新增 comptime owner tests。
- 已确认当前 `ConstValue` 没有 descriptor global forward-reference 表示，`descOf<T>` 暂按任务允许的 fallback 对所有类型返回 `0`，后续会在完成记录中写明 composite descriptor 真值待后续任务回填。
- 新增 `reflection_kind_desc_basic.scoop` run-pass fixture 后发现顶层 const initializer 仍走 LLVM HIR-compatible codegen；已补 `codegen_sysroot_kind_of` / `codegen_sysroot_desc_of`，单条 fixture 现已通过。
- 已补齐 payload-less / payload enum 在 const、MIR、LLVM HIR-compatible runtime folding 中的 `kindOf` 分类，并扩展 owner test/fixture 覆盖。
- 验证完成：`cargo test -p scoopc kind_of -- --nocapture`、`cargo test -p scoopc desc_of -- --nocapture`、新增单条 fixture、`cargo run -p scoop -- test`、`cargo test --all --all-targets`、`cargo clippy --all-targets -- -D warnings` 均通过。
