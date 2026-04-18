# 本次执行计划

## 目标
- 按 `TODO.md` 的顺序只完成第一个未完成任务。
- 在开始具体实现前，先检查最新提交是否提到已有问题；若有，先修复这些问题。
- 全程同步更新本文件，记录当前计划、关键决策、阻塞项与完成状态。

## 当前已知约束
- 只能在本次调用中完成一个任务；完成后需要更新 `TODO.md`、`PLAN.md`，提交 Git commit，然后停止。
- 如果首个未完成任务过大，需要先拆分为更小子任务，并把拆分结果写回 `TODO.md` / `PLAN.md`。
- 如果发现规范缺口、实现缺口或已有 bug 阻塞当前任务，不能绕过，必须先把前置修复任务加入 `TODO.md` 并调整顺序。
- 质量门槛包括相关测试通过，以及 `cargo clippy --all-targets -- -D warnings` 无警告。

## 执行步骤
1. 检查最新一次 Git 提交，确认是否明确提到遗留问题、已知缺陷或后续必须先处理的问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 结合代码库现状评估该任务是否可在一次迭代中完整落地。
4. 如果任务过大：
   - 设计更小的可执行子任务。
   - 更新 `PLAN.md` 与 `TODO.md`，让第一个子任务成为新的当前任务。
   - 继续执行这个新的首项任务。
5. 实现当前任务，必要时先修复其依赖的真实问题。
6. 运行最小且充分的验证：
   - 相关单测 / 集成测试 / fixture 测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 如任务影响范围较大，再补充 `cargo test --all` 或等价验证。
7. 更新文档状态：
   - 在 `TODO.md` 标记当前任务完成，或在阻塞时调整任务顺序并补充前置任务；
   - 更新 `PLAN.md`；
   - 更新本文件的进度记录。
8. 生成一次清晰的 Git 提交，然后停止。

## 进度记录
- 已创建本文件并写入初始计划。
- 已检查最新提交：`2c99e26b3df865ec984a711a59e1dcc2571fd878` 的主题为 `[T3103a] Record final execution status`，未新增需要先修复的遗留问题说明。
- 已读取 `TODO.md`，确认首个未完成任务为 `T3103a`：收口 `@Safe` / `@Unsafe` 与 `do` / closure 的绑定规则。
- 已完成现状勘察：
  - parser 仍在 `parse_unsafe_block_expr` / `parse_safe_block_expr` 中向后兼容接受 `@Unsafe { ... }` 与 `@Safe { ... }`；
  - AST 只有 `UnsafeBlock` / `SafeBlock`，尚无“annotated closure”的显式承载；
  - typecheck 会为 `UnsafeBlock` / `SafeBlock` 调整 unsafe context，但 lambda 本身没有 safe-region 标记；
  - `sysroot/string.scoop`、`SCOOP_RUNTIME.md` 与多组 fixtures 仍在使用 bare annotated block 旧写法。
- 评估结论：`T3103a` 可以在本轮直接完成，不需要先拆分。计划如下：
  1. 为 AST / HIR lambda 增加最小的 `@Safe` 注解承载，并保持现有 dump/debug 兼容。
  2. 调整 parser：
     - `@Safe do { ... }` → `SafeBlock`
     - `@Unsafe do { ... }` → `UnsafeBlock`
     - `@Safe { ... }` → safe lambda
     - `@Unsafe { ... }` → 稳定 parser 诊断，明确提示改用 `@Unsafe do { ... }`
  3. 调整 typecheck / lowering，使 safe lambda body 在 unsafe context 下按 safe region 校验。
  4. 迁移仓库中仍把 bare annotated block 当作 local block 使用的 sysroot / fixtures / 注释示例到 `do` 形式。
  5. 新增/更新 parser 与 fixture 回归，验证 safe closure、unsafe block 诊断与 `do` 语法迁移。
  6. 运行格式化、测试与 clippy，最后回写 `TODO.md` / `PLAN.md` / 本文件并提交。
- 执行中发现阻塞：
  - 在把 `unsafe_nogc` 样例尝试迁移到 `@Unsafe do { ... }` 时，暴露出更前置的既有类型检查缺口：statement-position 普通调用没有稳定触发 `@Unsafe` / `@Extern` / `@NoGC` 门禁。
  - 代表样例：`unsafe_call_requires_unsafe_is_error.scoop`、`extern_call_requires_unsafe_is_error.scoop`、`extern_call_after_unsafe_block_still_requires_unsafe_is_error.scoop`。
  - 这不是 `T3103a` 自身的语法细节，而是其前置依赖；不先修复就无法安全完成 bare annotated block → `do` 的全面迁移。
- 已按阻塞规则处理：
  - 撤回本轮尚未完成且未能通过验证的代码/fixture 修改，恢复仓库基线；
  - 在 `TODO.md` 前插入新的首项任务 `T3103a0`，专门修复 statement-position 普通调用的 unsafe/extern/`@NoGC` 门禁；
  - 更新 `PLAN.md`，把当前前端主线下一项改为 `T3103a0`。
- 当前收尾动作：
  1. 复查 `TODO.md` / `PLAN.md` 的重排是否正确。
  2. 提交本次“阻塞重排”变更。
