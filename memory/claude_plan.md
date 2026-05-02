# Claude Plan

## 当前目标
- 按 `TODO.md` 索引与对应 `TODO-Px.md` 详细任务列表，定位第一个未完成的详细任务。
- 仅完成该一个任务；若存在阻塞，则新增最小前置任务并同步索引后停止。

## 执行思路摘要
- 先读取 `TODO.md`，再按其中引用顺序检查对应 `TODO-Px.md` 的任务完成记录。
- 锁定第一个未完成详细任务后，阅读其约束、依赖、验证要求，并检查最新提交是否存在与该任务直接相关的未竟事项。
- 在不偏离规格、不引入权宜方案的前提下实现该任务。
- 运行与该任务直接相关的验证；若实现涉及通用编译/测试路径，则补充运行必要的 `cargo fmt`、相关测试，以及可行范围内的 `cargo clippy --all-targets -- -D warnings`。
- 完成后更新对应 `TODO-Px.md` 的完成记录；若任务索引、标题、顺序或新增前置任务发生变化，则同步更新 `TODO.md`；仅在阶段计划发生实质变化时更新 `PLAN.md`。
- 根据最终落地内容创建一次 git 提交，然后停止，不继续处理下一个任务。

## 进度记录
- [x] 初始化计划文件。
- [x] 读取任务索引与详细任务文件，确定当前执行目标。
- [x] 检查最新提交与当前任务是否存在直接相关的未完成事项。
- [x] 实现当前任务或在必要时新增最小前置任务。
- [x] 运行验证并修复问题。
- [x] 更新任务文档与索引。
- [ ] 提交变更并停止。

## 当前任务
- 当前首个未完成详细任务：`P2-T02R`（`TODO-P2.md`）
- 任务性质：review `Continuation` surface 与 typed sugar，确认零参 sugar 仅存在于 typed 阶段，且 refactor 路径不再依赖 resume 多参数特例。

## 下一步
- 创建本次任务的 git 提交，然后停止。

## 当前结论摘要
- 最新提交 `[P2-T02] Align continuation surface and typed Unit sugar` 未显式留下与 `P2-T02R` 直接相关的未完成事项。
- review 结论：`Continuation` surface 与 typed `Unit` sugar 设计一致；zero-arg sugar 仍只在 typed 阶段 canonicalize，AST/parser 继续保留原始 surface 形状。
- 额外搜索未发现 resume 多参数特例残留在 refactor typed 路径中。
- 已完成定向验证：`continuation_resume` / `unit_single_param_zero_arg` 单元测试、相关 fixture smoke、`dump-hir` 复验，以及 `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## 备注
- 本文件记录执行计划、进度与关键决策摘要，不包含内部逐词推理。
