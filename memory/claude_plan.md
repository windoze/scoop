# 本轮执行计划（初始版）

## 说明

按要求先写入可审阅的分析摘要与执行计划，再开始读取仓库信息并执行任务。这里记录的是面向实施的分析摘要、约束、决策依据与分步计划，不包含冗长的内部推演。

## 目标

本轮只完成 `TODO.md` 中**第一个未完成任务**，并在完成后停止。

## 约束与执行原则

1. 先检查最新提交信息，若其中提到已有问题，则这些问题优先纳入本轮范围并先修复。
2. 然后读取 `TODO.md`，定位第一个未完成任务。
3. 若该任务过大，需要先拆分：
   - 更新 `PLAN.md`
   - 更新 `TODO.md`
   - 使新的第一个子任务成为本轮执行目标
4. 实施时不得采用规避性方案、临时垫片、仅针对夹具的 hack，必须符合规范。
5. 若发现规范缺口、实现边界缺失或阻塞性 bug：
   - 在 `TODO.md` 中新增前置修复任务
   - 调整依赖顺序
   - 更新 `PLAN.md`
   - 提交并停止
6. 代码变更后必须进行相关验证，至少包括必要测试；若适用，还要跑 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
7. 完成后：
   - 更新 `TODO.md`
   - 更新 `PLAN.md`
   - 提交 git commit
   - 停止，不继续下一个任务

## 初始分步计划

1. 检查仓库状态与最新提交说明，确认是否存在最新提交中明确提到的待修复问题。
2. 读取 `TODO.md` 与 `PLAN.md`，识别第一个未完成任务及其上下文。
3. 判断该任务是否可在一轮内完整交付；如果不能，则先拆分并回写计划文件。
4. 阅读相关代码、测试、规范与最近改动，定位实现入口与潜在依赖。
5. 实施代码修改，必要时同步补充测试与文档/注释。
6. 运行格式化、测试、lint，修复发现的问题直到通过。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，记录完成情况或阻塞原因。
8. 生成一次清晰的提交，并结束本轮。

## 需要重点核查的内容

- 最新提交是否声明了已知缺陷、遗漏修复、后续待办。
- 第一个未完成任务是否依赖尚未完成的语言特性、运行时能力、标准库支持或诊断修复。
- 当前工作树是否已有用户未提交改动，避免误覆盖。
- 根目录 `README.md` 是否因本轮改动需要同步更新。

## 当前判断

- 最新提交说明本身没有单独挂出必须先修复的“已知问题”，但在代码与计划里明确留下了一个迁移边界：
  - 统一 state-machine plan/simplification 已接到 `codegen_handle_expr` 入口分类；
  - 但 `NoSuspendSites` 仍不可信，当前 no-perform 早退继续依赖 `block_may_perform`；
  - 原因是 plan 尚未覆盖若干“隐藏 unwind / hidden suspend”路径，例如：
    - `x as T` 失败触发的 `Raise.raise(RuntimeError.ClassCastFailed)`；
    - class ctor / property initializer / `init {}`；
    - object / companion object 的 once-init 访问。
- 进一步审计后确认：`T2003u4` 原始范围过大，且 immediate-resume / escape-continuation / mixed-arm 在“0 matching perform 时如何退化”为 no-perform 上仍存在模式差异，不能在一轮里安全完成。

## 拆分决策

将 `T2003u4` 拆为以下子任务，并只执行第一个：

1. `T2003u4a`：non-resuming / no-suspend 入口切到统一状态机输入
   - 补 unified plan 对隐藏 unwind 边界的识别（cast / class ctor / object init）
   - 让纯 non-resuming handle 的 no-perform 决策直接信任 unified plan，而不是旧的 `block_may_perform`
   - 保留 immediate-resume / escape-continuation / mixed-arm 的旧 gate，留待后续子任务继续迁移
2. `T2003u4b`：single-arm immediate-resume / escape-continuation 主 emitter 切到统一状态机输入
3. `T2003u4c`：mixed-arm / site-matrix / multiple-resuming 主 emitter 切到统一状态机输入

## 本轮实现计划（更新版）

1. 更新 `TODO.md` / `PLAN.md`，把 `T2003u4` 拆成 `T2003u4a`～`T2003u4c`，并把 `T2003u4a` 设为当前首个未完成任务。
2. 扩展 `HandlePlanContext` 与 `HandleStateMachinePlan`：
   - 记录 ctor call-site side table、object init 元数据；
   - 为 hidden unwind / init boundary 引入统一的 suspend-site 表示。
3. 调整 non-resuming 模式的 `codegen_handle_expr` 入口：
   - 对“仅 non-resuming arms 且 unified plan 判定 `NoSuspendSites`”的场景，直接走 `codegen_handle_expr_no_perform`；
   - 对 resuming / mixed 路径暂时保留旧 `block_may_perform` gate。
4. 补测试：
   - state machine plan 单元测试：覆盖 `as`、class ctor、object init 的 hidden site 识别；
   - LLVM run-pass：补 object init + try/catch 的端到端回归（cast 与 class ctor 已有现成覆盖）。
5. 运行格式化、测试、LLVM fixture、clippy。
6. 回写 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 完成状态，提交后停止。

## 进度记录

- 已完成：初始计划写入；检查最新提交、`TODO.md`、`PLAN.md`；确认首个未完成任务为 `T2003u4`；完成拆分设计。
- 进行中：准备把 `T2003u4` 拆为 `T2003u4a`～`T2003u4c`，并执行 `T2003u4a`。
- 已完成：`T2003u4a` 主体改动已落地，包括 unified plan 对 `as`/class ctor/object init/nested handle boundary 的 hidden suspend 建模，以及 pure non-resuming handle 入口改为在 `NoSuspendSites` 时直接信任 unified plan。
- 新发现并处理中：全量 fixture 暴露 `effect_escape_continuation_resume_later_exit.scoop` 回归。根因判断为 `try/catch`（lower 后是 `Raise.raise` 的 non-resuming handle）包裹 `Continuation.resume(...)` 时，unified plan 没把 builtin `resume` 识别为 hidden suspend/unwind site，导致错误走了 no-suspend fast path。
- 刚完成：在 `state_machine_plan.rs` 中把 builtin `Continuation.resume(...)` 分类为 `RuntimeRaise("Continuation.resume")` suspend site，并补了对应的 plan 单测，下一步重新跑定向与全量验证。
- 已完成：定向验证全部通过。新增单测 `plan_dump_marks_continuation_resume_as_runtime_raise_site` 通过；回归 fixture `effect_escape_continuation_resume_later_exit.scoop` 已恢复期望输出；此前新增的 object-init/cast/class-init/nested-handle 相关单测与 run-pass 路径也保持稳定。
- 已完成：全量验证通过：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 已完成：`TODO.md` / `PLAN.md` 将 `T2003u4a` 标记为完成；下轮应从 `T2003u4b` 开始，继续把 single-arm immediate-resume / escape-continuation 主 emitter 切到统一状态机输入。
