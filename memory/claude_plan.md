# 本轮执行计划（初始）

## 目标

按要求只完成 `TODO.md` 中第一个未完成任务；若发现前置问题、规范缺口或任务过大，则先调整 `TODO.md`/`PLAN.md`，提交后停止。

## 初始执行顺序

1. 检查最新一次 Git 提交，确认提交信息是否提到已知遗留问题。
2. 如最新提交提到遗留问题，先定位并修复这些问题，再继续。
3. 阅读 `TODO.md`，确定第一个未完成任务。
4. 阅读 `PLAN.md`，核对该任务的上下文、依赖与当前计划。
5. 判断该任务是否足够小且能在本轮完整完成。
6. 若任务过大或存在缺失前置能力：
   - 细化任务为更小子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的顺序与依赖；
   - 提交这些规划调整后停止。
7. 若任务可执行：
   - 实现任务；
   - 补充或调整测试；
   - 运行相关验证（至少包括受影响测试；如有必要运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`）；
   - 修复发现的问题直到通过。
8. 更新进度文档：
   - 在 `TODO.md` 中将本任务标记为完成；
   - 在 `PLAN.md` 中记录当前状态和后续影响；
   - 在本文件中补充本轮关键决策与完成情况。
9. 使用清晰的 Git 提交信息提交本轮变更。
10. 停止，不继续下一个任务。

## 当前已知约束

- 不能依赖变通方案、兼容层、仅夹具修复或偏离规范的实现。
- 如实现过程中暴露规范不匹配，必须先把该问题转化为 `TODO.md` 中更靠前的显式任务，再停止。
- 不回退或覆盖用户已有未说明改动。

## 执行记录

- 已创建本计划文件，后续在识别到具体任务、计划变化、关键步骤完成后继续更新。
- 已检查最新一次 Git 提交：`[T3010b2b0a0] Stop hidden-suspend ordinary helpers`。提交信息未显式提到需要先修复的遗留 issue，因此继续按 `TODO.md` 顺序执行。
- 已读取 `TODO.md` / `PLAN.md`，识别到当前第一个未完成任务为 `T3010b2b0a`：修正 hidden-suspend ordinary callee 在 unified state-machine caller 侧被误判为 plain `Call`。

## 当前任务理解（T3010b2b0a）

- 已确认 `T3010b2b0a0` 处理的是 ordinary helper 自身在 hidden-suspend boundary（如 object property access / class ctor init）返回 active 后还会继续执行的问题。
- 当前剩余缺口位于 caller 侧 unified state-machine 的 plan builder / suspend-call 分类：
  - `HandlePlanContext::known_fun_effects` 只看显式 function effect row；
  - object value/property access、class ctor init、runtime raise 等 hidden suspend 来源没有折叠进 callee 元数据；
  - 导致 caller 在构建 state machine 时把这类 helper 调用当作普通 `HandleStateOp::Call`，而不是会进入 dispatch / cleanup / resume 合同的 suspend boundary。

## 当前细化计划

1. 阅读 `T3010b2b0a` 相关生产代码：
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
   - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
   - 与 ordinary-frame active propagation / hidden suspend 分类相关的辅助代码。
2. 阅读或运行现有定向 fixtures，确认当前失败形态与已有覆盖缺口。
3. 设计并实现 caller-side hidden-suspend 分类修复：
   - 优先从统一语义元数据入手，而不是按源码形状打补丁；
   - 覆盖 object value/property access、class ctor init、runtime raise 三类隐藏 suspend 来源。
4. 补充或调整 fixture / 单测，至少覆盖 “helper -> object property access -> object init raise” 场景，并验证 caller tail 不再执行。
5. 运行相关验证：
   - 任务要求中的定向 `cargo run -p scoop --features llvm -- run ...`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo clippy --all-targets -- -D warnings`
6. 若验证过程中暴露更前置的规范缺口，则立即更新 `TODO.md` / `PLAN.md` 并停止；否则完成任务、更新文档并提交。

## 本轮关键验证与结论

- 已验证 ordinary 路径的既有 fixture：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_property_init_raise_helper_try_catch_basic.scoop`
  - 输出与预期一致，说明 `T3010b2b0a0` 的 helper 自终止修复仍有效。
- 已构造临时 `handle` 复现并运行：
  - top-level helper + object property hidden suspend：caller-side dispatch 正常，不出现 `body_unreachable`。
  - top-level helper + class ctor hidden suspend：caller-side dispatch 正常，不出现 `body_unreachable`。
  - local closure 包一层 helper：caller-side dispatch 正常，不出现 `body_unreachable`。
- 继续验证 member 路径时发现更前置 blocker：
  - 最小 repro：`object Helper { fun run(): Int { ... } }`。
  - 即使不经过 `handle`，普通 `Helper.run()` 也会触发 LLVM verifier 错误：
    `ptr @__scoop_object_instance__Helper` 被传给期望 `ptr addrspace(1)` receiver 的 `@Helper.run(...)`。
  - 这表明当前无法对 `T3010b2b0a` 的 member 路径做 spec-correct 验证；必须先修复 object 单例值的 LLVM 表示 / receiver ABI。

## 计划调整

- 依据用户规则，本轮不继续尝试以 workaround 完成 `T3010b2b0a`。
- 已将新的前置问题写入：
  - `TODO.md`：新增 `T3010b2b0a0b`，放在 `T3010b2b0a` 前。
  - `PLAN.md`：记录该 blocker、复现结果与新的执行顺序。
- 本轮目标改为：提交这次计划调整，然后停止，等待下一轮先处理 `T3010b2b0a0b`。
