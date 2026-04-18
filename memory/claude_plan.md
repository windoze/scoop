# 当前执行计划

## 约束说明

- 本文件会记录本轮任务的工作计划、关键决策、已完成步骤与必要调整。
- 这里提供的是可审计的执行思路与计划，不包含冗长的原始推理草稿；后续如计划变化或关键步骤完成，会持续更新。
- 本轮目标是：先处理最新提交中提到的既有问题，再完成 `TODO.md` 中第一个未完成任务；只完成一个任务后停止。

## 初始执行计划

1. 检查最新一次 Git 提交的提交信息与改动上下文，确认是否明确提到已知问题、未完成项或回归风险。
2. 检查当前工作树状态，识别是否存在用户未提交改动，避免覆盖无关修改。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，核对当前任务上下文、依赖关系与是否已有拆分计划。
5. 判断该首个未完成任务是否可以在本轮完整完成。
6. 如果任务过大或被实现缺口阻塞：
   - 在 `PLAN.md` 中细化为可执行子任务。
   - 在 `TODO.md` 中调整顺序、补充依赖、把当前应执行的首个子任务放到最前。
   - 提交这些规划性修改并停止。
7. 如果任务可执行：
   - 实现任务所需代码修改。
   - 补充或调整测试，确保行为符合规范而不是依赖变通方案。
   - 运行相关格式化、测试与 `clippy` 检查，修复所有出现的问题。
8. 完成后更新文档状态：
   - 在 `TODO.md` 中标记该任务完成。
   - 在 `PLAN.md` 中记录当前状态与后续影响。
   - 如本轮有关键实施决策，也同步记录到本文件。
9. 检查是否需要补充 `README.md`、代码注释或模块拆分，以满足本次改动质量要求。
10. 使用清晰的 Git 提交信息提交本轮变更，然后停止，不继续处理下一个任务。

## 关键检查点

- 不接受 workaround、shim、fixture-only hack 或任何偏离规范但“测试能过”的实现。
- 一旦发现当前任务依赖的语言特性、运行时能力或已有实现存在缺口，必须先把该缺口作为前置任务写入 `TODO.md`/`PLAN.md`，提交后停止。
- 不回退或覆盖无关用户修改。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交 `022ae16bfbb8594cf11b82ef52d9b55c7356f107`（`[T4003TR] Review 局部 destructuring 顶层复用主线`）：
  - 提交仅修改 `PLAN.md`、`TODO.md` 与本计划文件，没有直接引入新的生产代码改动。
  - 提交内容记录的是上一轮 review 结论与“未发现新的前置 blocker”，未出现需要在进入下一任务前先修复的新增代码问题。
- 已读取 `TODO.md` 与 `PLAN.md`，确认首个未完成任务为 `T4004a`：打通顶层 `val` pattern binder 的符号安装、类型收集与静态门禁。
- 进一步复查后确认原 `T4004a` 过宽：
  - 顶层 parser 当前连 `val (a, b) = ...` 都不接受，最小 probe 直接在 parse 阶段报“期望变量名，遇到 `(`”。
  - 原任务还同时包含“无整体类型注解时从 initializer 推断整体类型”和“跨文件 top-level value type 可见性”，已经超出单轮前端接入的合适范围。
- 已据此把原任务细化为：
  - `T4004a1`：顶层 pattern 的 parser / resolver 索引接入，以及显式整体类型注解路径（同文件静态引用）。
  - `T4004a2`：initializer 驱动推断与跨文件 binder 类型可见性。
- 当前要执行的首个子任务：`T4004a1`。
- `T4004a1` 实施已完成，关键结果如下：
  - 顶层 `val` parser 已支持 tuple / struct / enum pattern，并接受 `val <pattern>: Type = initializer` 形式的整体类型注解。
  - 顶层 `var` destructuring 继续在 parser 阶段按与局部路径一致的规则拒绝。
  - `ValBinding` 已提供统一的 `bound_idents()` helper；resolver index 与 block scope 检查现都通过同一入口收集 binder，顶层 pattern binder 会注入 value namespace。
  - `check_top_level_val_header` 现允许“带整体类型注解的顶层 pattern binding”，并继续对“无整体类型注解的顶层 pattern binding”报 `missing_type_annotation`。
  - `collect_top_level_value_types` 现可把顶层 pattern 的整体类型分发到各 binder；顶层 initializer typecheck 也会把 binder 类型写回 side table，为后续 lowering 复用。
  - 已新增回归：
    - parser 单测 `parse_top_level_val_destructuring_with_type_annotation`
    - parser 单测 `top_level_var_destructuring_is_rejected`
    - `tests/fixtures/typecheck/top_level_val_pattern_annotated_same_file_ok.scoop`
    - `tests/fixtures/typecheck/top_level_val_pattern_missing_type_is_error.scoop`
- 已完成验证：
  - `cargo test -p scoopc top_level_`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (329)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 待执行收尾：
  - 已将 `TODO.md` / `PLAN.md` 标记为 `T4004a1` 已完成、下一项转到 `T4004a2`。
  - 待提交本轮修改并停止。

## T4004a 执行分解

1. 修改顶层 `val` parser，使其支持 `val <pattern>: Type = init`，并对顶层 `var` destructuring 继续给出明确拒绝。
2. 修改 resolver/index，把顶层 pattern binder 注入 value namespace，保证同文件后续顶层声明与函数体能解析这些名字。
3. 修改 typecheck 头检查与 `top_level_types` 收集，使“显式整体类型注解 -> binder 类型分发”主线成立；无整体类型注解的顶层 pattern 先保持报错，留给 `T4004a2`。
4. 新增 parse / typecheck 回归，覆盖同文件的 tuple / struct / enum 顶层 binder 读取，以及顶层 `var` destructuring 继续报错。
5. 跑定向测试，再跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md` / `PLAN.md` / 本文件，标记 `T4004a1` 完成并记录结论。
7. 提交本轮修改，然后停止。
