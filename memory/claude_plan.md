# 当前执行计划

> 说明：本文件记录可审计的执行计划、关键决策和进度更新；不包含私有推理链路。

## 初始计划

1. 读取 `TODO.md`，严格按文档顺序定位第一个标题未带 `[DONE]` 的任务。
2. 检查最新提交是否明确提到与该任务直接相关的未完成问题；仅在它阻塞当前任务时纳入当前工作或作为前置任务记录。
3. 阅读当前任务要求、依赖、验证方式和相关代码，不做开放式历史问题扫查。
4. 如果任务可直接完成，进行最小正确实现，并补充或更新必要测试/fixture。
5. 运行任务要求的验证命令；若发现与当前任务直接相关的失败，优先修复根因，不使用规避方案。
6. 完成后更新 `TODO.md`：在任务标题前加 `[DONE]`，并补全 completion record。
7. 仅当阶段级计划或依赖结构发生变化时更新 `PLAN.md`。
8. 检查 git 状态和差异，提交本次任务涉及的全部未提交变更，然后停止，不进入下一任务。

## 进度记录

- 已写入初始执行计划；下一步读取 `TODO.md` 定位第一个未完成任务。
- 已定位第一个未完成任务：`P0-T03R`，目标是 review `P0-T03` 对 Scoop `const` surface、const evaluator 和跨阶段 const hooks 的删除结果。
- 最新提交为 `6ddbed26 [P0-T03] Remove Scoop const evaluator`，与当前 review 任务直接相关；本轮会将该提交的删除结果作为 review 对象。

## P0-T03R 执行步骤

1. 复查任务列出的指定位置：lexer/parser/AST/resolve/session/HIR lowering/lib/sysroot/B-24 fixture 路径。
2. 执行额外搜索 `comptime|ConstEval|const fun|const val|Modifier::Const|Keyword::Const|is_const`，分类确认活跃命中是否属于允许的 Rust/LLVM/测试常量/历史记录。
3. 重新运行 P0-T03 的验证命令；若全量命令过长，按任务要求设置足够 timeout。
4. 如发现旧 surface/evaluator/hook 残留，直接修复并重新验证；不以缩小 fixture 或规避路径完成任务。
5. review 通过后更新 `TODO.md` 和 `TODO-1.md` 的 `P0-T03R` 状态与完成记录。
6. 提交本任务所有变更后停止。

## P0-T03R 复查进展

- 已复查 `syntax/token.rs`、`syntax/lexer.rs`、`parser/decls.rs`、`ast/mod.rs`、`resolve/mod.rs`、`session/mod.rs`、`hir/lower/types.rs`、`lib.rs`、实际 sysroot core 文件 `sysroot/lib/scoop.core/src/core.scoop` 和 `tests/fixtures/umb_fix/B-24-reflection-comptime/`。
- `crates/scoopc/src/comptime/` 已不存在。
- 搜索 `pub mod comptime|ConstEval|Modifier::Const|Keyword::Const|is_const|const fun|const val` 于 `crates/`、`sysroot/`、`tests/fixtures/` 未发现活跃旧 Scoop const surface 命中。
- 额外 `comptime` 命中目前分类为：旧 package/statement parse-fail 回归、annotation retention policy 命名、测试中的人工 TODO reason、sysroot/comment 中的 compile-time metadata 描述；未发现 const evaluator 或 `const fun` / `const val` 语义复活。
- 下一步运行格式化、cargo check/test、fixture 与 clippy 验证。
- 已完成验证：`cargo fmt`、P0-T03 的 no-default check/test 命令、指定 fixture 命令、`cargo clippy --all-targets -- -D warnings` 均通过。
- 已更新 `TODO.md` 与 `TODO-1.md`，将 `P0-T03R` 标记为 `[DONE]` 并写入 completion record。
- 已检查 git 状态、目标文件差异和最近提交记录；变更仅涉及 `TODO.md`、`TODO-1.md` 与本进度文件。
- 下一步提交本任务变更。
