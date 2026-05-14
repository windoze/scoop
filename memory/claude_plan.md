## 当前执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断首个未完成任务。
2. 检查最近一次提交信息，确认是否有与该任务直接相关且未完成的问题需要并入当前任务或作为前置任务写回 `TODO.md`。
3. 阅读该任务涉及的源码、测试、规范与相邻实现，确认最小正确改动范围。
4. 实现该任务；如果遇到阻塞当前任务的真实缺口或回归，不绕过，先修复或在 `TODO.md` 中添加最小前置任务并停止。
5. 运行任务要求的验证，以及必要的回归测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若适用于当前改动范围）。
6. 更新 `memory/claude_plan.md` 记录关键进展或计划变化。
7. 在 `TODO.md` 中将已完成任务标题标记为 `[DONE]`，补充完成记录；仅在阶段计划变化时更新 `PLAN.md`。
8. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 进展记录

- 已创建本计划文件，接下来读取 `TODO.md` 确定当前任务。
- 已确认首个未完成任务为 `P5-T01：统一 composite transport contract，关闭 enum/array boxing residual`。
- 最近一次提交为 `[P4-T02] Close cleanup-unwind contract and main routing`，提交信息未显式声明与 `P5-T01` 直接相关的未完成事项；继续按 `P5-T01` 执行。
- 已读取 `PIPELINE_GAPS.md` / `PLAN.md` 与当前实现入口。当前 live gap 主要集中在：
  - `enum_lowering.rs` 仍把 large integer / nested enum / tuple / struct payload 分流到单 word 或 unsupported。
  - `control_flow.rs` 的 enum payload 解构仍对 nested enum 与 non-scalar payload 保留 unsupported。
  - `effect_lowered/value.rs` 的数组 get/set/build 路径在缺 metadata 时仍会回退 `u64` 或错误的旧 arg contract。
- 基线测试结果：`cargo test -p scoopc refactor_llvm_` 中，`pipeline::llvm_codegen_stage::tests::refactor_llvm_array_composite_transport` 失败，报错为 `refactor array_builder_push arg contract`，这是 `P5-T01` 的直接症状；另有若干 `effect_lowered/layout` 失败与本任务入口无直接对应，暂不作为当前阻塞项。
- 已用 `memory/debug_array_composite_transport.scoop` 复现并通过 `dump-mir` 精确定位：在 `sample.main` 中，数组字面量元素 `Hit(Point(...))` / `Pair((...))` 本身被错误降成了 `scoop.core.__scoop_array_builder_push` 的 direct call（仅 1 个参数，返回 `sample.Item`，同时还带错误的 array transport metadata），随后真正的 builder push 又把这个错误结果继续 push。
- 这说明当前阻塞不是单纯的 backend composite transport 缺口，而是更早的 typed call-site / synthetic helper span 身份污染，直接破坏 `P5-T01` 所依赖的 enum ctor contract。按任务规则，不能在 backend 侧绕过；需要先把这个前置缺口写入 `TODO.md` 并停止。
- 已在 `TODO.md` 中新增前置任务 `P4-T03：隔离 array literal synthetic helper call-site identity，修复 enum ctor contract 污染`，并把 `P5-T01` 的依赖改为 `P4-T03`。
- 已在 `PIPELINE_GAPS.md` 中新增 `§1.13` 记录该阻塞缺口，并把建议收口顺序同步为“先修 helper call-site 污染，再继续 P5 composite transport”。
- 已删除临时调试源文件；本次 invocation 的产出将只包含计划/账本更新与提交，不继续实现 `P5-T01`。
