# 本轮执行计划

## 说明

按要求先记录计划与决策摘要。这里记录的是可审计的执行计划、检查项与阶段性结论，不包含逐字内部推理。

## 目标

本轮仅完成 `TODO.md` 中第一个未完成任务；如果遇到前置缺陷、规范不匹配或任务需要拆分，则先更新 `TODO.md` / `PLAN.md`，提交后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否提到了现存问题、回归、已知缺陷或待修事项。
2. 阅读 `TODO.md`，确定第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务顺序是否一致。
4. 如果第一个未完成任务过大或存在隐藏前置依赖，则将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。

## 执行原则

1. 不使用规避方案、临时兼容层或只为夹具/测试通过而做的偏离规范实现。
2. 如果发现规范缺口、实现缺失、诊断错误、运行时/编译器缺陷或其他阻塞问题：
   - 先精确定义问题；
   - 在 `TODO.md` 中添加/重排前置任务；
   - 在 `PLAN.md` 中记录阻塞原因；
   - 提交这些文档变更后停止。
3. 在动手改代码前，先定位相关模块、调用路径、测试覆盖点与现有约束。
4. 代码改动完成后至少运行与任务直接相关的测试；若仓库允许，应补充运行更广范围校验，尤其是：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 任务相关的定向命令

## 交付清单

1. 实现第一个未完成任务或完成任务拆分/阻塞重排。
2. 更新 `TODO.md` 与 `PLAN.md`。
3. 在本文件补充关键进展与结论。
4. 用清晰的 Git 提交信息提交本轮所有修改。
5. 停止，不处理下一个任务。

## 进展记录

- 已创建本计划文件，下一步将检查最新提交、`TODO.md` 与 `PLAN.md`。
- 已检查最新提交 `b22d3077b23b9e7582010ed827c5fb172d7b61fe`。提交标题未单独声明新的必须先修遗留问题；当前工作区仅有本文件改动。
- 已定位 `TODO.md` 中第一个未完成任务：`T2003r3d2`。
- 已确认 `PLAN.md` 已记录 `T2003r3d2` 的作用域：补齐 immediate+escape mixed 在 nested `while` deeper indirect site 的合法 lowering，并将 `tests/fixtures/build/effect_resume_mixed_escape_while_indirect_is_error.scoop` 转正。
- 下一步：
  1. 读取失败 fixture 与对应 LLVM lowering/scan/matrix/unified-emitter 代码。
  2. 找出当前仍触发 `unsupported_main_body` 的 gate。
  3. 实现该合法形状的 lowering，并补/迁移测试。
  4. 运行定向测试、全量测试与 `clippy -D warnings`。
  5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。
