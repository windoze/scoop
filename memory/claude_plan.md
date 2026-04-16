# 本次执行计划

说明：按安全与协作要求，这里记录可执行计划、关键判断依据摘要与进度，不写出逐字内部推理。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；若发现前置缺陷、规范不匹配或任务过大，则先修复/拆分并更新 `TODO.md`、`PLAN.md`，随后仅处理新的首个可执行任务，最后提交一次 git commit 并停止。

## 执行步骤

1. 检查最新一次 git commit，确认是否提到已有问题；若提到问题，则先定位并修复这些问题。
2. 读取 `TODO.md`，识别第一个未完成任务。
3. 读取 `PLAN.md` 与相关上下文，确认该任务是否已存在依赖、拆分建议或规范约束。
4. 评估首个未完成任务的规模与前置条件：
   - 若可直接完成，则实现该任务。
   - 若过大，则把它拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，并将新的首个子任务作为本轮执行目标。
   - 若发现规范缺口、实现边界或已有 bug 阻塞，则在 `TODO.md` 中新增前置修复任务并调整顺序，更新 `PLAN.md` 说明原因，本轮仅处理调整后的首个任务。
5. 对实现进行验证，至少运行与改动相关的测试；若改动涉及通用构建质量，则补充运行格式化、测试与 `clippy` 检查，修复发现的问题。
6. 更新文档与任务状态：
   - 在 `TODO.md` 标记本轮完成的任务。
   - 在 `PLAN.md` 记录当前状态、后续影响与必要调整。
   - 持续更新本文件，记录关键进展。
7. 检查工作区变更，确保未误改无关内容；保留用户已有改动。
8. 使用清晰的提交信息提交本轮改动，然后停止，不继续下一个任务。

## 当前状态

- 已创建本计划文件。
- 已查看最新提交：
  - HEAD 为 `[T3010b2b0a] Front-load hidden suspend caller-side blocker`
  - 提交正文未额外列出新的历史问题说明。
- 已读取 `TODO.md` / `PLAN.md` 并定位首个未完成任务：
  - `T3010b2b0a`：修正 hidden-suspend ordinary callee 在 unified state-machine caller 侧被误判为 plain `Call`。
- 当前判断：
  - 该任务有明确描述、验收项和依赖，看起来可以直接落到代码定位与复现，不需要先进一步拆分。
- 已完成的额外定位：
  - 直接 fixture `object_init_raise_try_catch_basic.scoop` 与 `class_init_raise_cleanup_property_init_gc_basic.scoop` 当前都能通过。
  - 使用临时程序成功复现了真实缺口：
    - 调用链：`main(try/catch)` -> `helper()` -> `BoomObject.x` -> object init 中 `Raise.raise(...)`
    - 实际输出错误地包含 `helper_unreachable` 与 `main_unreachable`。
  - 已确认根因：
    - `state_machine_plan.rs` 中 `classify_suspend_call()` 对 ordinary callee 只使用 `HandlePlanContext::known_fun_effects`（显式 effect row）与少量局部类型信息；
    - object value/property access、class ctor init、runtime raise 等 hidden suspend source 只在表达式本体上识别，没有折叠进 callee 的可 suspend 元数据，因此 caller 侧仍把 `helper()` 当成 plain `Call`。

## 接下来的实现计划

1. 为 top-level/member 函数构建“call may suspend”元数据：
   - 不再只看显式 effect row；
   - 追加 hidden suspend source（object value/property、class ctor init、runtime raise）；
   - 递归/迭代传播到调用这些来源的 ordinary helper。
2. 为 local function value 增补可 suspend 元数据：
   - 生产路径：给 `CgLocal` / `Env` 增加局部函数值是否可能 suspend 的记录，并在 `val` 绑定、closure capture/param、assignment 等处维护；
   - unified plan builder：维护一份可变的 `known_local_fun_effects`，确保 `handle` 体内新声明或更新的局部函数值也能被后续 call site 正确分类。
3. 新增 run-pass fixture，覆盖“helper -> object property access -> object init raise”路径。
4. 运行针对性与全量验证：
   - 新 fixture；
   - `object_init_raise_try_catch_basic.scoop`
   - `class_init_raise_cleanup_property_init_gc_basic.scoop`
   - `cargo run -p scoop --features llvm -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`

## 当前执行结果

- 已完成的生产代码改动：
  - 为 top-level/member hidden suspend call 分类补了全局可传播元数据；
  - `HandlePlanBuilder` 现在维护可变的 `known_local_fun_effects`；
  - `CgLocal` / `Env` 现在携带局部函数值的 `call_may_suspend` 元数据；
  - 新增了一个 segment-level 单测，确认 hidden-suspend helper 会被分类为 `call-state-machine-callee`。
- 已完成验证：
  - `cargo check -p scoopc` 通过。
  - `cargo test -p scoopc segment_dump_classifies_hidden_suspend_helper_as_state_machine_callee -- --nocapture` 通过。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop` 通过。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/class_init_raise_cleanup_property_init_gc_basic.scoop` 通过。
  - `cargo run -p scoop --features llvm -- test` 的首个失败点仍是 `effect_escape_continuation_finally_arm_raise.scoop`，与 `PLAN.md` 中既有记录一致；本轮保留的中间代码没有把现有 LLVM fixture 基线推进到新的未知失败点。
- 新发现的更前置 blocker：
  - 新增的 run-pass 复现（随后已撤回，避免让 suite 变红）表明当前并非只剩 caller-side 分类问题。
  - 现状是：`main_unreachable` 已不再出现，但 `helper()` 自身仍会在 `BoomObject.x` 返回 active 后继续执行 `helper_unreachable`。
  - 这说明 ordinary-frame propagation 合同还没有覆盖 object value/property access、class ctor init、builtin runtime raise 等 hidden suspend boundary。
- 因此本轮结论：
  - 原首个未完成任务 `T3010b2b0a` 被新的更前置缺口阻塞，不能按原计划标记完成。
  - 已按流程把新前置任务插入 `TODO.md` / `PLAN.md`：
    - `T3010b2b0a0`：先修 hidden-suspend ordinary callee 自身在 boundary 后继续执行的问题；
    - `T3010b2b0a` 顺延到它之后，再处理 caller-side unified state-machine call 分类。
- 下一步：
  1. 确认工作区只保留可接受的中间代码与文档调整。
  2. 提交本轮“发现 blocker 并重排任务顺序”的变更，然后停止。
