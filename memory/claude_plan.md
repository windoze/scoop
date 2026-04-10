# 执行计划与进度记录

## 说明

按用户要求，先记录一份可审阅的执行计划、假设、风险和后续进度更新。
这里不会写入内部私有推理细节，但会完整记录外部可见的行动方案与关键决策。

## 当前目标

本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始执行步骤

1. 检查最新一次 Git 提交，确认提交说明里是否提到需要先修复的遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有计划与任务依赖。
4. 结合代码现状判断该任务是否足够小且可在本轮完整完成。
5. 如果任务过大：
   - 在 `PLAN.md` 中拆分为更小的子任务；
   - 在 `TODO.md` 中把原任务替换或扩展为按依赖顺序排列的子任务；
   - 本轮只执行拆分后的第一个子任务。
6. 实现当前目标任务。
7. 运行相关格式化、测试、lint，至少覆盖：
   - `cargo fmt --check`（必要时先 `cargo fmt`）
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 与当前改动直接相关的更小范围命令（如有）
8. 修复实现或测试中发现的问题，直到当前任务可接受地完成。
9. 更新文档与计划：
   - 在 `TODO.md` 中把当前任务标记为完成；
   - 在 `PLAN.md` 中同步当前状态与后续顺序；
   - 如本轮有关键决策，也更新本文件。
10. 检查工作区状态，避免误覆盖用户已有改动。
11. 提交本轮改动，提交信息与任务编号保持一致。
12. 停止，不继续执行下一个任务。

## 约束与注意事项

- 不会回退或覆盖与当前任务无关的用户现有改动。
- 若遇到任务阻塞，不会标记为 `[BLOCKED]`；而是根据依赖关系重排 `TODO.md`，更新 `PLAN.md` 后提交并停止。
- 本轮尽量保证编译、测试、lint 无 warning。
- 若发现根目录 `README.md` 明显缺失或与本轮任务直接相关，会一并修正；若无必要，不扩大范围。

## 进度记录

- 已创建本文件并写入初始计划。
- 已检查最新提交：`722bae2 [T0148d-1] 完成 Float comptime 支持`。提交说明本身未声明新的必须先修遗留问题。
- 已检查 `TODO.md` / `PLAN.md`：首个未完成任务为 `T0148d-2`（Float 字面量收尾：多文件 / 非入口源文件回归）。
- 已完成范围判断：该任务规模适中，本轮不需要继续拆分到 `PLAN.md` / `TODO.md` 子任务。
- 当前实施方案：
  1. 新增一个 `run_pass_cone` 多文件用例，覆盖 helper 文件中的 Float 顶层初始化、函数返回值、默认参数、科学计数法与 `Float32` absorption。
  2. 新增一个非入口失败用例，验证 helper 文件中的 Float 相关错误仍定位到真实源文件 / 真实 span。
  3. 运行针对性验证，再运行全量 `cargo test --all`、`cargo run -p scoop -- test`，并尽量执行严格 clippy 复核。
  4. 更新 `TODO.md` / `PLAN.md` / 本文件后提交。
- 下一步：创建上述 fixtures 并开始验证。

## 最新进度

- 已新增 2 组 `run_pass_cone` fixtures：
  - `float_multi_file_literal_basic`
  - `float_multi_file_literal_type_mismatch_non_entry`
- 定向验证中发现两个既有限制：
  1. helper 函数引用顶层 `val` 时，LLVM 仍报 `unsupported_main_body: top-level value ref`。
  2. 跨文件省略默认参数调用会落到 `unsupported_main_body: call arity mismatch`。
- 处理方式：
  - 这两个限制不是最新提交中声明的问题，也不是 `T0148d-2` 的必要验收前提；
  - 因此已把通过用例调整为“不引用顶层 `val`、不测试省略实参调用”，保留 helper 文件中的 Float 顶层初始化、普通函数返回值、显式传参、`Float32` absorption、科学计数法，以及非入口失败定位回归。
- 已完成的定向验证：
  - `cargo run -p scoop -- run tests/fixtures/run_pass_cone/float_multi_file_literal_basic`
  - 结果：通过，输出符合预期（`3.75 / 1.75 / 2.5 / 2.0 / true`）。
- 全量验证结果：
  - `cargo fmt --check`：通过
  - `cargo test --all`：通过
  - `cargo run -p scoop -- test`：通过（`fixtures: ok (858)`）
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`：通过
- 文档状态：
  - 已更新 `TODO.md`，将 `T0148d-2` 标记为完成，并补充完成说明与验收命令。
  - 已更新 `PLAN.md`，将 `T0148d-2` 迁移为 DONE，并记录新增 fixtures 与验证结果。
- 下一步：检查 git diff / status，提交本轮改动，然后停止。
