## 当前执行计划

说明：不写入内部推理细节；此文件记录可执行计划、关键决策、进度与阻塞，便于检查本次任务推进情况。

1. 读取 `TODO.md`，确认详细任务文件索引与顺序。
2. 按索引顺序读取对应 `TODO-Px.md`，定位第一个未完成的详细任务。
3. 检查最近提交是否存在与该任务直接相关且未完成的问题；若存在，将其视为当前任务的一部分，或在对应 `TODO-Px.md` 中登记为前置任务。
4. 阅读当前任务涉及的代码、测试、规范与相关实现，确认约束、依赖、验收条件与现状。
5. 若任务可直接完成：实现最小正确改动，不引入规避性方案。
6. 若发现阻塞该任务的真实缺口：在对应 `TODO-Px.md` 中插入最小必要前置任务，并同步 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
7. 运行与任务直接相关的验证：先小范围测试，再运行必要的格式化、lint、以及任务要求的测试命令；修复出现的问题。
8. 在对应 `TODO-Px.md` 中记录完成情况；如任务索引、标题、顺序或文件引用变化，同步更新 `TODO.md`。
9. 检查工作区变更，仅提交与本次任务相关的文件；按仓库约定创建一次 git commit。
10. 停止，不继续处理下一个任务。

## 进度记录

- 已初始化本次执行计划。
- 已读取 `TODO.md` 与 `TODO-P0.md`、`TODO-P1.md`，确认 `P0` 全部完成。
- 已定位首个未完成详细任务：`P1-T02R`（Review surface parse contract，确认 continuation / `Unit` sugar 仍是普通调用语法）。
- 已检查最近提交主题为 `[P1-T02] Lock resume and Unit call AST shapes`；提交主题未显式记录与 `P1-T02R` 直接相关的未完成事项，因此继续执行本 review。

## 当前任务执行细化

1. 阅读 `P1-T02R` 要求列出的文件：新增 parse fixtures / `.ast`、`crates/scoopc/src/parser/tests.rs`、`crates/scoopc/src/parser/expr.rs`、`crates/scoopc/src/ast/mod.rs`。
2. 运行指定搜索，确认不存在新的 AST 特例节点或 parser 关键分支。
3. 运行 `P1-T02R` 要求的定向测试与 smoke 命令。
4. 若 review 通过：在 `TODO-P1.md` 的 `P1-T02R` 完成记录中写入结论与验证结果，并提交。
5. 若 review 发现问题：优先直接修复；若存在必须先引入的新前置任务，则按要求更新对应 `TODO-Px.md` 与 `TODO.md` 后提交并停止。

## 当前 review 结论

- `tests/fixtures/parse/continuation_resume_member_call_basic.ast`、`continuation_resume_unit_call_basic.ast`、`unit_single_param_zero_arg_call_basic.ast` 均保持普通 `Call` / `MemberAccess` / `UnitLit` 组合，没有新的 AST 特例节点。
- `crates/scoopc/src/parser/tests.rs` 中的三个定向结构断言全部通过，直接验证了 `k.resume(x)`、`k.resume()`、`f()`、`f(())` 的 AST 形状与参数个数。
- `crates/scoopc/src/parser/expr.rs` 的普通 postfix 路径仍统一走 `parse_member_access_expr(...)` + `parse_call_expr(...)`；未发现针对 `k.resume(...)` 或 `k.resume()` 的普通 surface 特判。
- 额外发现：parser 中仍保留一个 `peek_ident_text("resume")` 分支，但它只用于旧 `-> resume { ... }` 已移除语法的迁移诊断 `HandleImmediateResumeRemoved`，不参与普通 member-call surface 解析；该行为与既有 removed-syntax 设计基线一致，不构成当前任务的阻塞项。
- 已完成本任务要求的 parser 测试、legacy/refactor parse fixture 验证、refactor `dump-ast` smoke 与 `clippy` 复验。
- 下一步：检查工作区状态，仅提交本次 `P1-T02R` review 相关文档更新后创建 commit。
