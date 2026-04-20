# 执行计划与进度记录

## 说明

本文件记录可公开的执行计划、关键决策与进度更新，不包含内部推理细节。

## 初始计划

1. 检查最新一次提交信息，确认是否提到需要先修复的已知遗留问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务范围过大，则把任务拆解为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务或子任务。
5. 运行相关测试与必要的质量检查，修复发现的问题。
6. 更新 `TODO.md`、`PLAN.md` 与本文件中的进度记录。
7. 提交本轮变更，提交后停止，不继续处理下一个任务。

## 当前状态

- 已完成仓库初查：
  - 最新提交为 `[T4010a2a] Box non-scalar enum payload variants`。
  - 提交说明里未单独挂出新的“必须先修”的遗留 issue。
  - `TODO.md` 中首个未完成任务为 `T4010a2b`：明确 enum payload 的 `with` copy-update 语义并接入统一 lowering / codegen。

## 当前执行重点

1. 阅读 `with` typecheck / lowering / codegen 以及 enum payload 相关实现。
2. 运行最小 probe，复现 `T4010a2b` 当前缺口。
3. 判断任务是否可直接完整收口；若不可控，则按要求拆分并更新 `TODO.md` / `PLAN.md`。
4. 若可直接收口，则完成实现、测试、文档更新与提交。

## 进度更新

- 已决定直接完成 `T4010a2b`，不再拆分。
- 已确定本轮语义：
  - enum `with` 路径以 variant 名开头，例如 `result with { Ok.point.x: 1 }`；
  - 运行时保留原 variant，只重建命中的当前 variant payload，其它 variant 原样返回；
  - lowering 主线改写为 `when + variant ctor`，复用既有 enum 解构/codegen。
- 已完成第一轮代码接线：
  - AST 新增 enum copy-update side table；
  - typecheck 已开始写回 “enum prefix -> concrete variant/field 形状”；
  - HIR lowering 已开始读取 side table 并接入 enum `with` 重建主线。
- 已完成一次定向编译验证，现有 tuple `with` lowering 单测仍为绿色。
- 已完成实现与验证：
  - 新增 enum `with` typecheck / run-pass / lowering 回归；
  - 已同步更新 `SCOOP_FULL_SPEC.md`、`TODO.md`、`PLAN.md`；
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`、`cargo run -q -p scoop -- test --fixtures tests/fixtures/hir`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 均已通过。
- 下一步：
  1. 复查工作区 diff 与任务状态。
  2. 提交本轮变更。
  3. 停止，不继续处理 `T4010b`。
