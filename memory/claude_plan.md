# 本轮执行计划（初始化）

## 说明

按要求，我需要在执行任何命令前先记录执行思路与计划。出于协作与安全边界，这里记录的是可公开的推理摘要、判断依据和操作计划，而不是内部逐字思维流。

## 当前目标理解

本轮需要：

1. 先检查最新一次 Git 提交是否提到了任何既有问题；如果提到了，需要优先修复这些问题。
2. 读取 `TODO.md`，找到第一个尚未完成的任务。
3. 如该任务过大，则拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 只执行当前排在最前面的那个未完成任务。
5. 完成实现、测试、文档更新、提交 Git，然后停止。

## 初始执行步骤

1. 查看最新提交信息与改动摘要，确认是否存在“已知但未修复的问题”。
2. 读取 `TODO.md`、`PLAN.md`，确认首个未完成任务及现有规划状态。
3. 结合代码结构和相关模块，判断该任务是否足够小，是否存在前置缺失或规范不匹配。
4. 若存在阻塞：
   - 在 `TODO.md` 中新增或重排前置修复任务；
   - 在 `PLAN.md` 中说明阻塞原因与依赖关系；
   - 提交并停止。
5. 若可直接执行：
   - 实现任务；
   - 增加或调整测试；
   - 运行必要的格式化、测试、lint；
   - 更新 `TODO.md`、`PLAN.md`、必要文档；
   - 提交并停止。

## 当前已知约束

- 每次调用只完成一个任务。
- 不能以规避、兼容层、测试专用 hack 的方式声称完成任务。
- 若遇到规范缺口或实现边界，必须先显式建任务并调整依赖，再停止。
- 需要尽量保证 `cargo clippy --all-targets -- -D warnings` 无警告。

## 待确认信息

- 最新提交是否明确提到待修复问题。
- `TODO.md` 中首个未完成任务的内容与复杂度。
- 该任务涉及的 crate、运行时或规范文件范围。

## 进展更新（完成上下文审计后）

### 最新提交检查结果

- 最新提交为 `600979be708583c6af2e6cb9c330d06798528232`，标题为 `[T2003r3d4a] Unify multi-resuming resolver contract`。
- 提交说明未额外声明“本提交已知但未修复的问题”；因此无需先按提交备注补修独立遗留项。

### 当前锁定任务

- `TODO.md` 中第一个未完成任务是：
  - `T2003r3d4b [TODO] Effect：unified emitter 支持 1 immediate + N escape`

### 可行性判断

- 该任务在当前轮次内可直接实现，不需要再拆子任务。
- 现有 shared resolver / metadata contract 已就位，缺口主要在两处：
  1. simplification / unified entrypoint 仍把 `1 immediate + N escape` 归类为 `UnsupportedMixedMultipleEscapeWithImmediate`；
  2. mixed multi-resuming leaf 入口仍把 escape arm 数量硬编码为 1，需要推广为“单 immediate arm + 多个 escape arm”。

### 目前观察到的实现要点

- `multi_resuming_mixed.rs` 的后半段其实已经以 `scanned_sites: Vec<MultiResumingEscapeSitePlan>` 为核心，site 级恢复、step trampoline、binder slot、arm body dispatch 都支持“多个 escape site”。
- 真正仍写死“只有一个 escape arm”的位置主要是：
  - leaf 函数签名与入口选择；
  - root 级别的 escape-arm metadata 收集；
  - same-handle runtime frame 只给一个 escape arm 建了单个 frame。
- runtime `scoop_continuation_alloc` 确认会捕获当前 TLS handler stack 顶，因此若 mixed leaf 继续保留 same-handle runtime frame 语义，就必须保证多个 escape arm 的 frame 都位于稳定地址上；这意味着需要把 mixed state 中的单个 `handler_frame` 布局推广为可容纳多个 escape frame 的布局。

## 本轮详细执行计划（已细化）

1. 修改 simplification / unified multi-resuming entrypoint 分类：
   - 让 `stack_reenter == 1 && heap_continuation >= 1` 走 `MultiResuming`；
   - 保留 `N immediate + 1/多 escape` 仍为后续 `T2003r3d4c` 的未实现边界。
2. 修改 unified multi-resuming leaf 入口：
   - 将当前 `1 immediate + 1 escape` leaf 推广为 `1 immediate + N escape`；
   - 让 `nonresuming.rs` 在 `immediate_arms.len() == 1 && !escape_arms.is_empty()` 时接入该 leaf。
3. 推广 mixed leaf 的 root runtime frame 布局与装配：
   - state object 中为每个 escape arm 预留稳定的 handler frame 存储；
   - root 进入时按 arm 顺序压入所有 escape frame；
   - 继续维持 immediate arm / escape arm / finally 执行期间的 same-handle detach / restore 语义。
4. 补回归：
   - 新增或更新 LLVM 定向单测，覆盖 `1 immediate + N escape` 的 direct + indirect representative sample；
   - 新增 run-pass fixture，并带 sibling non-resuming 或 `finally`。
5. 运行最小验收：
   - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
   - `cargo run -p scoop --features llvm -- run <新增 fixture>`
   - `cargo clippy --workspace --all-targets -- -D warnings`
6. 更新 `TODO.md` / `PLAN.md` 标记完成情况，提交 Git，并停止。

## 进展更新（实现与验证后）

### 已落地的代码变更

- 已放开 simplification：`1 immediate + N escape` 不再归类为 `UnsupportedMixedMultipleEscapeWithImmediate`。
- unified multi-resuming 入口现已把“单 immediate arm + 多个 escape arms”接到 mixed leaf。
- mixed leaf 已推广为消费多个 escape arms，并把 same-handle runtime frame 布局从单个 frame 扩到按 escape arm 数量分配。
- 已新增定向单测与 run-pass fixture，代表样例 `effect_resume_mixed_multi_escape_direct_indirect.scoop` 通过。
- 验证过程中顺手修复了两个 pre-existing route gap：
  1. single-resuming zero-match / no-suspend 时，single escape / immediate 现会回退到顺序 no-perform leaf；
  2. `1 escape + sibling non-resuming` 现已能进入 unified heap leaf，不再提前报 route mismatch。

### 新发现的阻塞问题

- 在继续跑 full LLVM fixture suite 时，又暴露出一个更早的 pre-existing heap-leaf replay gap：
  - 夹具：`tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`
  - 现象：同一 escape arm 在 nested block 中先 direct、再 indirect 时，第二次 `resume(...)` 前没有按既有 fixture contract 重放 direct→indirect 之间的 prefix，导致 stdout 偏差。
- 这不是 `T2003r3d4b` 自身 representative sample 的缺口，而是更早的 pure heap leaf 合法子集还没完全收口。

### 计划调整

- 因为 full-suite 验证被上述 pre-existing gap 阻塞，本轮不能把 `T2003r3d4b` 标记为完成。
- 已在 `TODO.md` / `PLAN.md` 中新增前置任务 `T2003r3d4a1`，专门修这个 pure heap leaf replay 缺口。
- 下一次调用应先处理 `T2003r3d4a1`，而不是直接继续把 `T2003r3d4b` 标记完成。

### 当前验证状态

- 已通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_mixed_multi_escape_direct_indirect.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_finally_no_perform.scoop`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 仍失败：
  - `cargo run -p scoop --features llvm -- test`
  - 当前首个失败项：`tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_block_multi.scoop`
