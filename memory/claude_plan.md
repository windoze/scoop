# 执行计划与推理摘要

## 约束说明

- 本文件记录可审阅的执行计划、关键判断依据、进展更新与后续调整。
- 出于安全与协作边界考虑，这里不会逐字记录私有思维链，但会提供完整、可检查的推理摘要与操作步骤。
- 本次调用目标：只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始步骤

1. 检查最新一次 git 提交，确认是否提到任何已知问题、回归或待修复事项。
2. 如果最新提交中明确提到已有问题，则优先修复这些问题，并完成必要测试。
3. 读取 `TODO.md`，定位第一个未完成任务。
4. 读取 `PLAN.md`，核对当前计划、依赖顺序和任务背景。
5. 判断该任务是否足够小且可以在本轮完整完成。
6. 如果任务过大或存在未满足前置条件，则先更新 `PLAN.md` 与 `TODO.md`，把任务拆分或重排，并执行拆分后的第一个子任务。

## 执行策略

1. 先尽量以最小读取范围获取上下文：最新提交信息、`TODO.md`、`PLAN.md`、相关模块与测试。
2. 若发现规范不匹配、实现缺口、测试依赖缺失或现有 workaround，按要求将其转化为显式任务，而不是绕过。
3. 对要修改的代码进行精确实现，同时补充或调整测试。
4. 运行与改动直接相关的测试，再运行更广泛的检查，至少覆盖：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如任务涉及格式或特定工具，再补充对应命令
5. 更新文档：
   - `TODO.md` 标记当前任务完成，或在阻塞场景下重排任务
   - `PLAN.md` 反映状态变化、依赖调整与结论
   - 必要时补充 `README.md` 或内联注释
6. 提交 git commit，提交信息应清晰描述本次完成的任务。
7. 停止，不继续处理下一个任务。

## 当前状态

- 已创建计划文件。
- 已检查最新提交：`[T4003S] 插入顶层 val 读取前置任务`，提交本身未包含额外代码修复，仅把“普通顶层 val 可执行读取”显式前置。
- 已读取 `TODO.md` / `PLAN.md`，确认当前首个未完成任务为 `T4003S`：收口普通顶层 `val` 的可执行读取语义。

## 已确认的问题画像

- 当前 HIR lowering 只为两类顶层绑定建立后端 side table：
  - `const val` -> `top_level_consts`
  - `@ThreadLocal/@Global var` -> `top_level_vars`
- 普通顶层 `val` 仍只保留在通用 `hir::Item::Val` 中，没有进入后端专用索引。
- LLVM `codegen_var_ref` 遇到 `ValueRef::TopLevel` 时，只会依次识别：
  1. object 单例值
  2. `const val`
  3. 顶层静态 `var`
  4. 否则报错 `top-level value ref`
- 因此，普通顶层 `val` 在运行期表达式位置不可读取，这正是当前 blocker。

## 进一步影响面

- 若仅在 `codegen_var_ref` 增加一个 ad-hoc 分支而不补 side table，会继续缺少：
  - 初始化函数/一次性求值语义
  - 顶层 `val` initializer 中调用函数的可达性收集
  - effect 状态机对“读取顶层 `val` 可能触发隐藏初始化调用”的识别
- 因此该任务需要同时修改：
  - HIR lowering / `LoweredHir` side table
  - LLVM codegen 的顶层 immutable value 初始化与读取路径
  - reachable function collector
  - effect state machine 的隐藏 suspend 分类与 suspendability 分析

## 细化执行计划

1. 新增“普通顶层 immutable value”的 HIR side table，并在 lowering 时收集命名的非 `const` 顶层 `val`。
2. 让 `LoweredHir`、LLVM codegen 输入和相关辅助逻辑都携带这张 side table。
3. 为普通顶层 `val` 实现统一的运行期表示：
   - module-local backing global
   - once guard
   - 按需生成的 init function
   - 表达式位置读取时先确保初始化，再加载结果
4. 扩展 reachable function collector，使顶层 `val` initializer 中引用的函数/构造器也会被纳入 codegen。
5. 扩展 effect 状态机分析，使读取顶层 `val` 与 object init 一样被视为可能触发隐藏初始化调用。
6. 新增/更新回归：
   - 普通顶层 `val` 被 `main` 读取
   - 一个普通顶层 `val` 读取另一个普通顶层 `val`
7. 运行定向 fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
8. 更新 `TODO.md` / `PLAN.md`，提交本轮完成结果并停止。

## 执行结果

- 已完成 HIR side table 扩展：
  - `LoweredHir` 新增 `top_level_immutable_values`
  - lowering 现收集命名的非 `const` 顶层 `val`
- 已完成 LLVM 主线实现：
  - 普通顶层 immutable value 现在使用 backing global + once guard + init function
  - `codegen_var_ref` 读取普通顶层 `val` 时会先触发一次性初始化，再返回已存储值
- 已补齐关联基础设施：
  - reachable function collector 会递归扫描顶层 immutable value initializer
  - effect state-machine 会把顶层 `val` 读取识别为隐藏的一次性初始化边界
- 已新增回归：
  - `tests/fixtures/build/top_level_val_read_minimal_ok.scoop`
  - `tests/fixtures/run-pass/top_level_val_runtime_read_basic.scoop`
- 已完成验证：
  - `cargo run -p scoop -- build tests/fixtures/build/top_level_val_read_minimal_ok.scoop -o /tmp/top_level_val_read_minimal_ok.out`
  - `/tmp/top_level_val_read_minimal_ok.out` 退出码为 `42`
  - `cargo run -p scoop -- test --fixtures /tmp/t4003s-run-pass`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前剩余步骤：
  1. 更新任务状态与计划文档
  2. 检查 git diff
  3. 提交本轮变更并停止
