# Claude Plan

## 约束说明
- 不写出不可审计的内部推理细节；这里记录可检查的执行计划、关键判断依据、进度与变更。

## 初始执行计划
1. 读取 `TODO.md`，把它当作索引而不是任务正文。
2. 按索引顺序读取对应的 `TODO-Px.md`，定位第一个在详细任务文件中尚未明确记录完成的任务。
3. 检查最近一次提交是否直接提到与该任务相关且未收尾的问题；若是，则将其并入当前任务范围，或在相应 `TODO-Px.md` 中登记为当前任务的前置依赖。
4. 阅读该任务涉及的代码、测试、规范与相邻任务记录，确认约束、依赖、验收方式与是否存在阻塞。
5. 若无阻塞，直接实现该任务；若发现无法按规范完成的真实前置问题，则只新增最小必要前置任务，更新详细 TODO 与 `TODO.md` 索引，并停止在该步。
6. 对实现运行必要验证，优先执行任务要求的针对性测试；若改动影响较广，再补充更高层验证，并确保无警告要求可满足。
7. 更新 `TODO-Px.md` 中当前任务的完成记录；仅在任务编号、标题、顺序或引用变化时同步 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
8. 检查工作区改动，避免覆盖我未创建的无关修改；按仓库既有风格创建一次非交互式 git 提交，然后停止。

## 进度记录
- 已写入初始计划，下一步开始读取任务索引与详细任务文件。
- 已读取 `TODO.md` 与 `TODO-P0.md`，确认首个未完成详细任务为 `P0-T02`：建立并行 pipeline dispatcher 壳层。
- 已检查最新提交：`[P0-T01R] Review shared effect pipeline selector`，未发现提交信息中明确挂起且必须先于 `P0-T02` 处理的未收尾问题。
- 当前工作区仅存在本次新增的 `memory/claude_plan.md` 修改；下一步审查 `scoop`/`scoopc` 现有命令路由与 `Session` 入口。
- 已确认当前 `dump-ast` / `dump-hir` / `dump-mir` / `dump-ir`、`build` 内部 parse/LLVM 发射，以及 `scoopc --emit-*` 仍直接命中 legacy API，尚无并行 dispatcher 壳层。
- 选定实现方案：新增 `crates/scoopc/src/effect_refactor_pipeline/` 顶层模块，包含 `legacy` / `refactor` 两侧 stage entry 与统一 boundary wrapper；P0 中 `refactor` 入口先在阶段边界整体委托给 legacy 闭包，不把分支渗入低层业务模块。
- 计划把 `scoop` 的 `dump-*`、`build`/`run`/`test` 实际触发的 parse/HIR/MIR/LLVM 路径，以及 `scoopc --emit-llvm` / `--emit-obj` 路径改为经由该模块；并补充 dispatcher/命令层定向测试，再执行 smoke、定向测试与 clippy。
- 已完成实现：新增 `scoopc::effect_refactor_pipeline` 顶层 dispatcher 模块，并把 `dump-*`、`build` 的 parse/LLVM 发射、`fixtures` 的 parse/HIR/MIR fixture 路径，以及 `scoopc --emit-*` 入口改为通过 stage wrapper 进入。
- 定向测试通过：
  - `cargo test -p scoop --no-default-features cli`
  - `cargo test -p scoop --no-default-features dump_ast_command_uses_refactor_ast_dispatcher`
  - `cargo test -p scoopc --no-default-features session`
  - `cargo test -p scoopc --no-default-features driver_cli`
  - `cargo test -p scoopc --no-default-features effect_refactor_pipeline`
- smoke 结果：
  - `dump-ast` / `dump-hir` / `dump-mir` 的 legacy vs refactor 输出一致；
  - `dump-ir` 的文本在 legacy vs legacy 重复运行时也会漂移，说明当前 `MaterializedMir` Debug 输出本身跨进程不稳定；本次仅确认 legacy/refactor 两条命令都成功经由 dispatcher 返回相同退出状态，这不是本次壳层改动引入的回归。
- 额外验证通过：`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。
