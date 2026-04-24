# 执行计划

## 目标

本轮只完成一项工作：先检查最新提交是否提到需要优先修复的既有问题；若有，先修复该问题。之后读取 `TODO.md`，定位第一个未完成任务，只完成这一项并停止。

## 执行顺序

1. 检查最新提交信息与工作区状态，确认是否存在提交中明确提到的遗留问题，以及是否有用户已存在的未提交修改需要避让。
2. 读取 `TODO.md` 与 `PLAN.md`，确定第一个未完成任务，并理解当前计划与依赖关系。
3. 判断该任务是否过大：
   - 若可直接完成，则继续实现。
   - 若过大，则先把任务拆分为更小的前置子任务，更新 `TODO.md` 与 `PLAN.md`，提交后停止。
4. 若在调查、测试、实现过程中发现任何既有缺陷、规格不匹配、实现边界缺失或回避式方案：
   - 先把该问题当作当前工作范围；
   - 能直接修复就先修复；
   - 若不能直接修复且它阻塞当前任务，则把修复该问题的新任务插入 `TODO.md` 中当前任务之前，更新 `PLAN.md`，提交后停止。
5. 对当前目标任务进行实现，并补充或调整相关测试。
6. 运行必要验证，至少覆盖：
   - 与改动直接相关的测试；
   - `cargo test --all`（若成本或环境异常阻塞，需要在计划中记录具体原因）；
   - `cargo clippy --all-targets -- -D warnings`；
   - 视改动需要运行 `cargo fmt`。
7. 更新文档与任务状态：
   - 在 `TODO.md` 标记该任务完成，或在阻塞时重排任务顺序；
   - 更新 `PLAN.md` 反映当前状态、依赖和后续顺序；
   - 如有必要，更新 `README.md` 或相关内联注释。
8. 使用清晰的提交信息提交本轮改动，然后停止，不继续处理下一项任务。

## 进度记录

- [x] 初始化本轮计划文件
- [x] 检查最新提交与工作区状态
- [x] 读取 `TODO.md` / `PLAN.md`
- [x] 确认并处理最新提交提到的既有问题（如无则记录）
- [x] 确认第一个未完成任务及其依赖
- [x] 实现或拆分该任务
- [x] 运行验证
- [x] 更新 `TODO.md` / `PLAN.md` / 相关文档
- [ ] 提交改动并停止

## 当前判定

- 最新提交为 `ca2138d [T4016T8] Fix cross-thread task handoff GC roots`。提交信息本身未额外指出尚未修复、需要优先插队的新遗留问题；因此继续按 `TODO.md` 顺序推进。
- 当前工作区除本文件外无未提交修改，需要避让的现有用户改动为空。
- 本轮目标 `T4016T9` 已完成；`TODO.md` 中当前第一个未完成条目已推进为 `T4016T4R`。
- `PLAN.md` 当前顺序与 `TODO.md` 一致：`T4016T4R -> T4017a -> T4017b ...`。

## 下一步执行细化

1. 盘点 `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md`、`sysroot/core.scoop`、`sysroot/task.scoop`、`sysroot/unsafe.scoop` 以及相关实现注释中仍残留的旧叙事。
2. 逐项改写为统一合同：
   - core `Task` 是轻量 claim-bit 驱动对象；
   - 不支持 shared / thread-safe 多驱动；
   - cross-thread 仅允许顺序 handoff；
   - `step()` 上的 `Running` / 并发 / reentrant 误用直接 trap；
   - `Pending` 仅表示真实未就绪。
3. 复查是否存在文档与实现不一致的新既有问题；若发现阻塞项，按要求先修复或前插任务。
4. 运行与本任务相关的验证，至少包含文档/规格相关检查与全量测试/静态检查。
5. 更新 `TODO.md`、`PLAN.md`、本文件并提交。

## 变更日志

- 初始化：建立本轮执行框架，后续在关键步骤完成或计划变化时同步更新本文件。
- 已完成初步调查：确认最新提交未引入需要优先插队的新遗留问题，并锁定当前目标为 `T4016T9`。
- 已完成实现：已把 `SCOOP_TASK.md`、`SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md`、`ISSUES.md`、`STDLIB_COMPLETENESS.md`、`sysroot/core.scoop`、`sysroot/task.scoop`、`sysroot/unsafe.scoop` 以及相关编译器/runtime 注释统一到 core `Task` 的 atomic claim-bit / single-driver / sequential handoff 合同。
- 已完成验证：`cargo run -p scoop_tools -- spec-fixtures check`、`cargo test --all`、`cargo run -p scoop -- test`（`fixtures: ok (1169)`）与 `cargo clippy --all-targets -- -D warnings` 均通过。
- 已完成状态同步：`TODO.md` 已将 `T4016T9` 标记为完成，`PLAN.md` 当前顺序已推进到 `T4016T4R`。
