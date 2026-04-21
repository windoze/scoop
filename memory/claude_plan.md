# 执行记录

## 当前阶段

已完成初始化与任务定位，进入 `T4012a` 实施。

## 约束说明

- 按用户要求，先记录计划，再执行任何 shell 命令。
- 这里只记录可审阅的执行计划与决策摘要，不写入不可验证的内部推理细节。

## 初始执行计划

1. 检查最近一次提交的提交信息与改动，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大或依赖缺失，拆分任务并更新 `PLAN.md` 与 `TODO.md`，然后只处理拆分后的第一个子任务。
4. 阅读相关代码、规格、测试与文档，确认实现边界与现状。
5. 实现当前目标任务，必要时补充或整理模块与注释。
6. 运行相关格式化、检查、测试与 lint，至少覆盖：
   - `cargo fmt --check`（必要时先 `cargo fmt`）
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 与当前任务直接相关的更小范围命令（如有）
7. 若发现规范不匹配、缺失特性或既有缺陷阻塞当前任务：
   - 先把该问题转化为更前置的 `TODO.md` 任务；
   - 更新 `PLAN.md` 解释依赖关系；
   - 提交这些计划调整并停止。
8. 若任务完成：
   - 在 `TODO.md` 中标记完成；
   - 更新 `PLAN.md` 记录已完成内容与后续状态；
   - 提交所有变更；
   - 停止，不继续下一个任务。

## 更新规则

- 每完成一个关键步骤，或执行计划发生变化时，更新此文件。

## 已完成的关键步骤

1. 已检查最近一次提交 `14154fefabf612e786e51a3ee74a1e869a1a67bc` 的提交说明与改动摘要；未发现需要在 `TODO.md` 现有顺序之外优先修复的新既有问题。
2. 已阅读 `TODO.md` 与 `PLAN.md`；当前首个未完成的可执行子任务为 `T4012a`：将 annotation 收口为 compile-time markers only，并拒绝复杂 nominal 语义。
3. 已初步审阅 annotation 相关实现与文档：
   - `crates/scoopc/src/typecheck/annotations.rs`
   - `crates/scoopc/src/typecheck/builtin_annotations.rs`
   - `crates/scoopc/src/typecheck/type_env.rs`
   - `crates/scoopc/src/resolve/mod.rs`
   - `sysroot/core.scoop`
   - `SCOOP_FULL_SPEC.md`
   - `ISSUES.md`
4. 已完成 `T4012a` 实现：
   - annotation declaration contract 已收口为 compile-time markers only；
   - typecheck 新增拒绝：非法 `annotation` modifier target、annotation class nominal modifier、type/effect params、`where`、supertypes、type body；
   - 规格与注释文档已同步。
5. 已完成定向回归与全量验证：
   - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
   - `cargo run -p scoop -- test`
   - `cargo test --all`
   - `cargo run -p scoop_tools -- spec-fixtures check`
   - `cargo clippy --all-targets -- -D warnings`

## 当前判断

- `T4012a` 已完成，不需要进一步拆分。
- 现有实现已经拒绝了一部分复杂 annotation class 形态：
  - 只能是 `class`
  - 不支持 supertypes
  - 不支持 type body
  - 主构造参数必须为 `val`
- 本轮补充后，annotation model 的主要剩余工作已切换到 `T4012b`：non-inline built-in annotations 的具体编译器语义。

## 细化执行计划（T4012a）

1. 在 annotation/typecheck 相关代码中补齐并收口 declaration-shape 约束：
   - 明确 `annotation` modifier 只服务于 `annotation class`；
   - annotation class 禁止 type params、effect params、where clause；
   - 保持并复核现有 data-only 限制（无 supertype、无 body、参数必须 `val`）。
2. 补充针对非法复杂 nominal 组合的 fixtures / 单测：
   - 合法 marker annotation case；
   - 非法 supertypes / body / type params / effect params / where clause / 非 class 目标等。
3. 同步规范与相关文档，明确 annotation 是 compile-time markers only，而不是一般 nominal/runtime feature。
4. 运行定向测试与全量质量检查：
   - annotation 相关 fixtures
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 若实现与规范一致且测试通过：
   - 更新 `TODO.md`、`PLAN.md`；
   - 提交变更并停止。

## 当前状态

- `TODO.md` / `PLAN.md` 尚待写回完成状态。
- 下一步：整理 git diff、提交 `[T4012a] ...`，然后停止。
