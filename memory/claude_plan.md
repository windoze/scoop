# 当前执行计划

## 约束与记录方式

- 按用户要求，本次先记录计划，再执行任何仓库检查命令。
- 不记录逐字内部思维；改为记录可审计的任务理解、判断依据、执行步骤和变更进展。
- 目标是只完成 `TODO.md` 中第一个未完成任务；如任务过大，则先拆分并更新 `PLAN.md` / `TODO.md`，随后只完成拆分后的第一个子任务。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到已知遗留问题；如果有，先处理这些问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 评估该任务是否可以在本轮完整交付：
   - 若可以，直接实现。
   - 若不适合一次完成，拆分为更小子任务，并更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
4. 阅读相关代码、测试和文档，确认影响范围。
5. 实现改动，必要时补充或调整测试。
6. 运行格式化、测试与 lint，至少覆盖：
   - `cargo fmt --check` 或 `cargo fmt`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 若任务只影响局部，也会优先运行更小范围命令加快迭代，但最终仍以相关验证充分为准
7. 更新文档与计划文件：
   - 在 `TODO.md` 中标记任务完成，或在受阻时按依赖顺序重排任务
   - 在 `PLAN.md` 中记录当前状态、拆分依据或阻塞说明
   - 在本文件补充关键进展
8. 检查 `git status`，确认只包含预期改动，不回退用户已有修改。
9. 使用清晰的提交信息提交本轮结果，然后停止。

## 关键检查点

- 先处理“最新提交中提到的遗留问题”，再进入 `TODO.md` 任务。
- 不一次推进多个任务。
- 不使用破坏性 Git 命令回退未知改动。
- 发生阻塞时，保留任务为 `TODO`，只调整顺序和计划说明，然后提交并停止。

## 进展日志

- 已创建本计划文件，待开始仓库检查。
- 已检查最新提交 `a9d11cd06c4aed83103b35055c852ecd34e4c9c3`：提交信息为 `[T0153] 支持 receiver function value 调用`，未额外注明需优先修复的遗留问题。
- 已定位 `TODO.md` 中首个未完成任务：`T0154 [TODO] LLVM：higher-order 间接调用支持 aggregate 返回值`。
- 已阅读 `TODO.md` / `PLAN.md` 上下文，判断 `T0154` 可以直接在本轮完成，无需先拆分到 `PLAN.md` / `TODO.md`。

## 当前任务：T0154

### 任务理解

- 现状：
  - 顶层直接调用对 aggregate 返回值有部分支持，但 higher-order 间接调用路径（closure / function value / `FunPtr`）仍显式拒绝 `Tuple/Struct/Enum` 返回值。
  - `codegen` 里的注释已明确指出正确修复方向是把这类返回值转为 sret，而不是继续依赖 `gc-leaf-function` 等局部绕过。
- 目标：
  - 让 closure/function value/`FunPtr` 的间接调用支持 aggregate 返回值。
  - 保持与现有 statepoint/GC 约束兼容，避免再次落入 aggregate `gc.result` 问题。

### 预计实现步骤

1. 审计并抽出 higher-order 间接调用签名构造逻辑，确认哪些返回类型需要走 hidden sret 参数。
2. 修改 closure lambda 本体的 LLVM 签名与 body 返回逻辑：
   - aggregate 返回值改为 `void + sret*`；
   - 调整 env/receiver/params 的 LLVM 形参索引；
   - 在返回点把结果写入 sret 槽位。
3. 修改 function value / `FunPtr` 间接调用：
   - aggregate 返回值时在调用点分配结果槽；
   - 以 sret 指针作为首参发起 indirect call；
   - 调用后从结果槽加载回 `CgValue`。
4. 为必要的函数定义/调用点添加 sret attribute，确保 LLVM 21 下 ABI 与优化/重写 pass 看到的是一致签名。
5. 新增 fixtures：
   - closure 返回 tuple；
   - higher-order mapper 返回 struct；
   - `@Unsafe` `FunPtr` 返回 aggregate（优先选择 tuple/enum 中最稳妥的一种表示）。
6. 跑格式化、测试、clippy。
7. 更新 `TODO.md` / `PLAN.md` / 本文件，提交本轮改动并停止。

## 本轮完成情况

- 已完成 higher-order aggregate 返回值的核心实现：
  - `closure` / `function value` / `FunPtr` 间接调用支持 hidden sret；
  - lambda 本体现在设置 GC strategy，避免带分配 closure 在 GC stress 下失去 stackmap/statepoint；
  - aggregate 默认值路径改为统一的 zero-initializer，覆盖 effect unwind / Raise / callee-suspend / block early-exit 等返回分支。
- 已新增 runtime 测试辅助：
  - `runtime/c/scoop_test.c` 新增 `scoop_test_get_make_int_pair_funptr`（返回一个 sret 风格 tuple helper 的函数地址）；
  - `runtime/c/scoop_runtime_api.h` 已登记新导出符号。
- 已新增并手动验证 3 个 run-pass fixtures：
  1. `higher_order_aggregate_return_closure_tuple`
  2. `higher_order_aggregate_return_struct_mapper`（含 `SCOOP_GC_STRESS=1` 手动验证）
  3. `unsafe_funptr_aggregate_return_tuple`
- 已完成格式化与全量验证：
  - `cargo fmt`
  - `cargo check -p scoopc`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
