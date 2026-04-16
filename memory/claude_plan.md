# 本轮执行计划（摘要版）

说明：按安全与协作要求，这里记录的是可审计的执行思路摘要与分步计划，不写入逐字内部推理。

## 目标
- 先检查最新提交是否提到任何既有问题；如有，先修复这些问题。
- 读取 `TODO.md`，定位第一个未完成任务。
- 如该任务过大，则拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 只完成当前首个未完成任务（或拆分后的首个子任务），补充测试、更新文档与计划、提交 git commit，然后停止。

## 分步计划
1. 查看最新一次 git commit 的标题与正文，确认是否提到待修复的既有问题。
2. 读取 `TODO.md`、`PLAN.md`，确认当前优先级最高的未完成项，以及是否已有相关分解计划。
3. 评估任务规模与依赖；若存在前置缺失、规格不匹配或任务过大，则先更新 `TODO.md` / `PLAN.md` 反映依赖与拆分关系。
4. 实现当前应执行的任务，尽量限制改动范围并保持模块清晰。
5. 运行相关验证：至少包含受影响测试；如果改动范围足够大，再补充 `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 等必要检查。
6. 根据结果修复问题，直到当前任务完成或明确发现新的前置阻塞。
7. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md` 记录完成状态或阻塞原因。
8. 使用清晰的提交信息创建 git commit，然后停止，不继续下一个任务。

## 执行约束
- 不回退用户已有改动。
- 不以 workaround、shim、仅夹具通过的方式宣称完成。
- 若发现规范缺口或实现边界阻塞当前任务，先把该问题转化为更靠前的 `TODO.md` 任务，再提交并停止。

## 当前进展
- 已检查最新提交 `75290cd88e745967dada48ae6245cf5bd61e7830`，提交说明未额外声明新的既有问题；当前工作区仅有本文件修改。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T3010b2b0a`：修正 hidden-suspend ordinary callee 在 unified state-machine caller 侧被误判为 plain `Call`。
- 已知前置已完成：`T3010b2b0`、`T3010b2b0a0`、`T3010b2b0a0b`。当前需要重点回查 top-level helper、member 路径、local function value / closure 包装 helper 等 caller-side hidden-suspend 分类。
- 已完成临时最小复现验证：
  - `handle { Helper.run() }`（member direct call）不再执行 caller tail；
  - `handle { thunk() }`，其中 `thunk` 为包一层 `Helper.run()` 的 local closure，也不再执行 caller tail；
  - 直接把顶层函数名赋给局部函数值（`val thunk = helper`）当前前端仍不支持，这是既有语法/推导边界，不属于本任务新增回归。
- 已确认一个关键实现事实：step function 发射期间会临时清空 `current_fun_return_ty`，因此 ordinary call 的 active-check 不会在 unified state-machine 内兜底；若 caller-side 真被误降成 plain `Call`，新增 run-pass fixture 必然能复现 caller tail 继续执行。也就是说，本任务可以通过 fixture/单测严格锁定，不需要“猜”分类是否正确。
- 已完成仓库改动：
  - 新增 3 个 run-pass fixture，覆盖 top-level helper、member helper、local closure/function-value 三条 caller-side hidden-suspend 路径。
  - 在 `state_machine_segments.rs` 新增 2 条分类单测，分别锁定 member helper 为 `call-state-machine-callee`、local closure/function-value 为 `call-may-suspend`。
  - 已更新 `TODO.md` / `PLAN.md`，将 `T3010b2b0a` 标记为完成，并记录当前验证结论与后续首个 blocker。
- 已完成验证：
  - `cargo test -p scoopc segment_dump_classifies_hidden_suspend_ -- --nocapture`
  - 新增 3 个 fixture 的 `cargo run -p scoop --features llvm -- run ...`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`：首个失败点仍是已知后续 blocker `effect_escape_continuation_finally_arm_raise.scoop`（`T3010b2b1`），未出现更早回归。

## 下一步
1. 检查当前 diff，仅保留与 `T3010b2b0a` 直接相关的文件。
2. 用任务编号创建 git commit。
3. 停止，等待下一轮执行 `T3010b2b0R`。
