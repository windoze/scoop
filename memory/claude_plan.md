## 当前轮次执行计划

说明：按要求先记录执行计划与进展日志。这里记录可审计的行动计划、假设、风险与状态，不写逐字内部推理。

### 目标

完成 `TODO.md` 中第一个未完成任务；若被现有缺陷或缺失特性阻塞，则先把阻塞项整理进 `TODO.md` / `PLAN.md`，提交后停止。

### 初始步骤

1. 检查最新一次 Git 提交：
   - 阅读提交信息与变更摘要。
   - 判断是否显式提到尚未修复的问题、已知缺陷或后续待修事项。
   - 如果存在属于“提交中已知但未修复的问题”，优先修复。
2. 阅读 `TODO.md` 与 `PLAN.md`：
   - 找到第一个未完成任务。
   - 判断任务是否可在本轮完整实现。
3. 如任务过大或存在前置依赖：
   - 将任务拆成更小子任务。
   - 更新 `PLAN.md`。
   - 更新 `TODO.md`，保证顺序和依赖正确。
   - 本轮只执行拆分后的第一个子任务。
4. 实现当前任务：
   - 阅读相关代码、测试与规范。
   - 修改实现并补充/调整测试。
5. 验证：
   - 运行与改动相关的测试。
   - 运行格式化/静态检查（至少覆盖受影响范围；若可行则执行项目要求的完整检查）。
   - 发现问题立即修复并复测。
6. 文档与任务状态更新：
   - 在 `TODO.md` 标记完成，或在阻塞场景下重排任务。
   - 在 `PLAN.md` 记录当前状态与后续影响。
   - 同步更新本文件中的进展日志。
7. 提交：
   - 使用清晰的 Git commit message。
   - 提交后停止，不继续下一个任务。

### 风险检查清单

- 不用规避方案掩盖规范缺口。
- 如果发现规格不匹配，先把修复任务前置到 `TODO.md`。
- 不回退用户已有改动。
- 如果工作树存在无关修改，只在理解其影响后与其共存。

### 进展日志

- 已创建本计划文件，尚未开始仓库检查。
- 已检查最新提交 `6ec6158`（`[T4010b] Update execution record`）及其 diff；该提交仅更新执行记录，没有新增“已知但未修复”的代码缺陷说明需要先处理。
- 已阅读 `TODO.md` / `PLAN.md`，当前第一个未完成任务为 `T4010b1`：收口值类型 computed property 的 getter lowering / codegen。
- 当前判断：`T4010b1` 看起来范围明确，先不拆分，先验证最小失败样例并定位涉及的 lowering / codegen 路径。
- 已完成实现主线：
  - HIR lowering 新增值类型 computed property getter side table，并将 `receiver.prop` 在 typed lowering 中改写为 getter 调用。
  - `member_funs` side table 现同时收集值类型 computed property getter。
  - 泛型成员 callable 单态化已从“仅 class member fun”扩展到 nominal member callable，覆盖 struct/enum getter。
  - LLVM 调用目标解析已允许从 value/ref nominal receiver 提取具体 type args。
- 已新增回归：
  - run-pass：`struct_computed_property_getter_basic.scoop`
  - Rust 单元测试：直接断言 typed lowering 已把 computed property 读取改写成 getter call。
- 已完成验证：
  - `cargo test -p scoopc rewrites_value_computed_property_access_to_getter_call`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1094)`）
  - `cargo test --all -- --test-threads=1`
  - `cargo clippy --all-targets -- -D warnings`
- 执行全量 fixture suite 时还发现一条既有测试噪音：`tests/fixtures/parse/with_update_expr.ast` 仍使用旧字段名 `resolved_struct_fqns`；已同步为当前 AST 里的 `resolved_copy_update_tys` / `resolved_copy_update_enums`，避免无关红灯干扰本轮验证。
- 新发现的后续缺口：
  - generic 值类型 member access / computed property 读取在“无 expected-type 帮助”的上下文里仍会把结果类型保留为抽象 `T`（最小复现：`Box(9).readBack == 9`）。
  - 已按依赖顺序把该缺口前插为新的 `T4010b1a`，位于 `T4010b1` 与 `T4010R` 之间。
- 当前状态：
  - `T4010b1` 已完成并可提交。
  - 下一轮首个未完成任务为 `T4010b1a`。
