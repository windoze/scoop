# 本轮执行计划

## 目标

完成 `TODO.md` 中首个未完成任务 `T4013`：删除 `inline` 关键字，并把 `@Inline` 收口为唯一的内联提示 surface；如果在实现或验证过程中暴露已有问题，则先修复该问题，或把它整理成阻塞前置任务后停止。

## 最终结论

- 最新提交 `8af63361196264a6b9fab71c95ac3fa683c45bd0` 没有留下新的需先修遗留问题。
- `TODO.md` 中首个未完成条目 `T4013` 已在本轮完成。
- 当前 `TODO.md` 的下一个未完成条目已更新为 `T4013R`。

## 已完成工作

1. parser / AST / typecheck：
   - 删除了 `Modifier::Inline` 与 `FunSigOwned.is_inline`。
   - parser 对旧 `inline` modifier 现在发出 `scoop::parse::inline_modifier_removed`，并给出迁移到 `@Inline` 的提示。
   - lambda 内 `return` 已统一回到“只能离开立即包裹的命名函数体内”的规则，不再存在 inline 例外。
2. `@Inline` surface：
   - 编译器 built-in annotation 识别已补上 `@Inline`。
   - `@Inline` 当前只允许用于函数、且不接受参数。
   - `sysroot/core.scoop` 已补齐 `@Target(AnnotationTarget.Function) annotation class Inline`。
3. 文档与任务状态：
   - `SCOOP_FULL_SPEC.md` 第 7.2 节已切换为 `@Inline`。
   - `TODO.md` 已把 `T4013` 标记为完成，并把下一步更新为 `T4013R`。
   - `PLAN.md`、`ISSUES.md` 已同步到“`@Inline` 交叉项与 legacy inline 残留均已收口”的状态。
4. fixtures / regression：
   - parse fixture 已新增 removed-syntax 回归并更新 `modifiers_basic` AST golden。
   - typecheck fixture 已新增 `@Inline` 正向 / 负向回归，以及“`@Inline` 不再放宽 lambda return”的回归。

## 验证结果

- `cargo check -p scoopc`
- `cargo run -p scoop -- test --fixtures tests/fixtures/parse`，结果 `fixtures: ok (123)`
- `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`，结果 `fixtures: ok (394)`
- `cargo run -p scoop -- test`，结果 `fixtures: ok (1197)`
- `cargo test --all`
- `cargo run -p scoop_tools -- spec-fixtures check`
- `cargo clippy --all-targets -- -D warnings`

## 提交注意事项

- 提交时排除当前工作区里与本任务无关的 `run_agent.sh` 改动。
- 本轮提交完成后立即停止，不继续执行 `T4013R`。

## 进度记录

- 已完成：实现、文档同步、全量验证。
- 下一步：提交本轮变更并停止。
