# 执行计划（公开摘要）

说明：按要求先记录计划与关键决策摘要；出于安全与协作边界，这里不写逐字内部思维过程，只保留可审阅的执行步骤、假设、风险和状态更新。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；若最近一次提交提到已有问题，则先修复该问题；若在探查、测试、实现过程中发现任何既有缺陷或规格不匹配，也必须先修复或将其以前置任务形式插入 `TODO.md`，然后停止。

## 初始步骤

1. 检查最近一次提交信息与变更摘要，确认是否显式提到待修复问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务依赖。
4. 结合相关代码、测试、规格与当前工作树状态，判断该任务是否能在本轮完整完成。

## 执行策略

1. 如果最近提交提到既有问题：
   - 先复现并修复该问题。
   - 运行相关测试与必要的回归测试。
   - 更新 `TODO.md` / `PLAN.md` / 本文件。
   - 提交后停止。
2. 如果第一个未完成任务过大：
   - 将其拆分为更小、可验证的子任务。
   - 更新 `PLAN.md` 与 `TODO.md` 的顺序和依赖。
   - 选择新的第一个子任务执行。
3. 实现当前目标任务：
   - 修改代码。
   - 补充或调整测试。
   - 运行格式化、相关测试，以及尽量覆盖到的质量检查（至少包括与改动相关的测试；若可行则跑 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`）。
4. 如果执行时发现既有缺陷/规格缺口：
   - 不绕过，不缩小语义。
   - 先修复；若本轮无法直接修复，则在 `TODO.md` 中插入前置任务并调整依赖，然后提交并停止。

## 文档更新要求

在以下时点更新本文件：

- 确认了“最近提交是否包含待修复问题”之后。
- 确认了首个未完成任务之后。
- 如果任务被拆分或重排。
- 完成关键实现步骤后。
- 完成测试与最终结论后。

## 交付标准

1. 当前目标任务完整实现，且无已知回归。
2. `TODO.md` 中对应任务被标记完成，或若被阻塞则正确重排并保持为待办。
3. `PLAN.md` 已同步最新状态。
4. 本文件已记录关键进展。
5. 使用清晰提交信息提交一次 git commit。

## 当前状态

- 状态：已完成初始核对。
- 最近提交：`38ae428d [T4016T5] Enable atomic field lvalues`
  - 结论：提交标题与提交体未额外提及需先处理的既有问题；按既定顺序继续 `TODO.md` 首个未完成任务。
- 首个未完成任务：`T4016T6`
  - 目标：把 core `Task` object model 从 per-task `Mutex` 改成轻量 atomic claim field。
  - 直接后继：`T4016T7` 会在此基础上重写 `Task.step()` 的 claim/release 协议，因此本轮需要只完成对象布局与创建路径改造，不提前混入完整 trap/并发语义收口。
- 代码现状摘要：
  - `sysroot/core.scoop` 里的 `Task<T>` 仍带 `__lock: scoop.sync.Mutex` 字段。
  - `sysroot/task.scoop` 仍在 `Task.step()` 中通过 `task.__lock.lock()/unlock()` 串行化状态迁移。
  - `T4016T5` 已打通对象字段上的 atomic lvalue 编译主线，因此理论上可直接把 claim 字段放进 `Task` 对象并由 ordinary Scoop 代码读写。
- 下一步：
  1. 阅读 `sysroot/core.scoop` / `sysroot/task.scoop` 的任务实现细节。
  2. 查找现有 atomic intrinsic / helper 的实际可用 surface 与调用方式。
  3. 判断 `T4016T6` 能否直接完整落地；若出现基础缺口，则先作为既有问题处理或重排任务。

## 新发现 blocker（执行中）

- 在尝试实现 `T4016T6` 并运行 `cargo run -p scoop -- test --fixtures tests/fixtures/build` 时，编译流程没有进入 task 语义验证，而是在 LLVM codegen 阶段因 `SourceFile::slice` 对 UTF-8 非字符边界做字符串切片而 panic。
- 目前定位：
  - 触发栈落在 `MainCodegen::int_literal_bits_from_source_span_if_present -> SourceMap::slice -> SourceFile::slice`。
  - 更具体地，cross-file class ctor 调用在 `codegen_class_ctor_invoke_inner(...)` 中把 `current_source_id` 切到 callee/class source 后，继续对已求值 ctor args 做 `store_local_value(...)`；该路径仍可能尝试基于“当前 source + span”回读整数文本。
  - 当 caller/callee 分属不同源码，且 callee 源文件前部含中文注释等非 ASCII 文本时，会把 caller 的 span 错绑到 callee source 上，最终在 `SourceFile::slice` 上触发 UTF-8 boundary panic。
- 结论：
  - 这是一个既有前置缺陷，阻塞 `T4016T6`。
  - 本轮应先把该缺陷作为新的前置任务插入 `TODO.md` / `PLAN.md`，并优先修复；`T4016T6` 暂不继续。

## blocker 修复结果

- 已新增并完成前置任务：`T4016T5a`
- 实际修复：
  1. `crates/scoopc/src/llvm/codegen/gc.rs`
     - 新增 `store_local_value_exact(...)`，用于“已完成类型对齐”的值直接落槽，避免重复走 source-backed integer literal 反查。
  2. `crates/scoopc/src/llvm/codegen/mod.rs`
     - cross-file class ctor 参数本地化与 ctor-parameter-property 写回改用 exact-store helper，不再把 caller 的整数字面量 span 绑到 callee source 上回读文本。
  3. `crates/scoopc/src/source.rs`
     - `offset_to_line_col` 与 `SourceMap` span 校验现在会拒绝非 UTF-8 字符边界的 offset/span，避免同类 source mismatch 直接 panic。
  4. 回归：
     - 新增 source 单测覆盖非字符边界 offset/span；
     - 新增 LLVM 单测 `cross_file_class_ctor_literal_codegen_uses_correct_source_with_utf8_comments`，直接覆盖“跨文件 class ctor + 整数字面量参数 + 中文注释”路径。

## 验证结果

- `cargo fmt`
- `cargo test -p scoopc --features llvm`
- `cargo run -p scoop -- test`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

## 当前收尾状态

- `T4016T5a` 已完成并写回 `TODO.md` / `PLAN.md`。
- 下一个未完成任务重新回到 `T4016T6`。
- 本轮不继续实现 `T4016T6`；下一次调用从它开始。
