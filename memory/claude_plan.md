# 执行计划

说明：按要求记录“可外显”的执行计划、关键判断与进度更新；不写入私有推理细节。

## 初始计划

1. 检查最新一次提交信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 如该任务过大，拆分为更小的可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前轮应完成的首个任务或首个子任务。
5. 运行相关测试、格式化与静态检查，至少覆盖：
   - `cargo fmt --check`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   如范围较大，会先跑与改动直接相关的测试，再决定是否补充全量验证。
6. 更新文档状态：
   - 在 `TODO.md` 中标记当前任务完成，或在无法直接完成时调整其顺序。
   - 在 `PLAN.md` 中记录当前状态、拆分结果或阻塞关系。
   - 持续更新本文件，记录关键步骤是否完成以及计划是否变化。
7. 检查 `README.md`、注释和代码组织是否需要配套修正；只处理与本次任务直接相关且必要的部分。
8. 使用清晰的提交信息提交本轮变更，然后停止，不继续下一个任务。

## 进度记录

- [x] 已写入初始计划
- [x] 已检查最新提交
- [x] 已识别首个未完成任务
- [x] 已判断是否需要任务拆分
- [x] 已完成实现
- [x] 已完成测试与静态检查
- [x] 已更新 `TODO.md` / `PLAN.md`
- [ ] 已完成提交

## 变更日志

- 初始化本计划文件，后续在关键节点追加更新。
- 已检查最新提交 `c9e2a1908e13573793ee2c50c04758e82e974215`（`[T0148d-2] 补齐 Float 多文件回归`）。提交信息未额外声明需先修复的既有问题。
- 已确认首个未完成任务为 `T0148d-3 Float 字面量收尾：剩余转换、边角语义与审计`。
- 任务复杂度评估：本轮可直接完成，不再拆分 `TODO.md` / `PLAN.md` 主任务结构。
- 审计发现的当前轮直接处理项：
  1. `when` pattern 中出现 Float 字面量时，parser 只有通用错误，且会连带产出额外噪声错误。
  2. comptime 常量求值尚未支持 Float builtin 方法（`toInt` / `toString` / `hash` / `abs` / `isNaN` / `isInfinite`）。
  3. LLVM f-string / 插值字符串当前仅支持 `{Int}` 与 `{String}`；`f"... {1.5}"` 会报 `unsupported_main_body: string interpolation expr type`。
- 审计中确认但本轮不作为直接修复目标的现象：
  - 顶层 `const val` 的一般表达式目前会落到 `scoop::llvm::unsupported_main_body` 的 `top-level value ref`；经对照样例确认，这不是 Float 特有缺口，而是更宽的既有限制。
- 已完成实现：
  1. parser：Float `when` pattern 专门诊断，并避免额外级联 parse 噪声。
  2. comptime：Float builtin 方法在 const 路径可折叠（`toInt` / `toString` / `hash` / `abs` / `isNaN` / `isInfinite`）。
  3. LLVM：f-string / 插值字符串支持 Float；新增 run-pass 覆盖 generic `println(Float)` 与 `NaN` / `Infinity` 文本。
- 已完成验证：
  - `cargo fmt --check`
  - `cargo test --all`
  - `cargo run -p scoop -- test` → `fixtures: ok (861)`
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
