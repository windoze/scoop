# 执行计划与进度记录

## 说明

- 按要求先建立本文件，再开始仓库检查与任务执行。
- 这里记录可审计的执行计划、关键判断依据摘要、已完成步骤和计划变更。
- 不写入私有推理细节，但会持续更新足够详细的行动计划与进度。

## 初始计划

1. 检查最新一次 Git 提交，确认提交信息里是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前项目分解和依赖关系。
4. 如首个未完成任务过大，则先把任务拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后只执行新的第一个子任务。
5. 为当前目标任务收集上下文：定位相关模块、测试、规范或历史实现。
6. 实现任务，过程中若发现既有缺陷、规格不匹配、回避式实现边界或阻塞问题，优先修复；若无法在本轮直接修复，则把它插入 `TODO.md` 作为前置任务，更新 `PLAN.md` 后停止。
7. 运行与该任务相关的测试；若改动影响范围较大，则补充运行更广泛校验。
8. 额外执行质量检查，至少覆盖格式化、相关测试，以及在可行范围内执行 `cargo clippy --all-targets -- -D warnings`。
9. 更新文档与计划：在 `TODO.md` 标记任务完成，在 `PLAN.md` 记录状态和任何调整，并回写本文件的进度。
10. 提交 Git 提交信息，然后停止，不继续处理下一个任务。

## 当前状态

- 已完成：创建计划记录文件。
- 已完成：检查最新 Git 提交；提交信息仅为 `[T4012b3] Implement @Suppress warning-code surface`，未额外声明需要优先修复的既有问题。
- 已完成：检查 `TODO.md` / `PLAN.md`；当前顺序已进入 `T4012c`，其内容是加入 built-in `@Experimental(val feature = "...")` annotation。
- 已确认：工作区存在用户侧未提交改动 `run_agent.sh`，后续不触碰也不回退。
- 已完成：检查 annotation / built-in annotation 现状。结论是 parser 已能把 `@Experimental(feature = "...")` 解析成 annotation 实参中的赋值表达式，因此无需改通用 annotation 语法；本轮只需补 built-in 识别、参数校验、文档与 fixture。
- 已完成：实现 `@Experimental` built-in surface，补入编译器识别、target/参数校验、`sysroot/core.scoop` 声明、`SCOOP_FULL_SPEC.md` 与 `ISSUES.md` 说明，以及新的 parse/typecheck fixtures。
- 已完成：`cargo fmt`、`cargo run -p scoop -- dump-ast tests/fixtures/parse/experimental_annotation_feature_gate_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/parse`、`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` 通过。
- 已完成：更大范围回归通过，包括 `cargo run -p scoop -- test`、`cargo test --all`、`cargo run -p scoop_tools -- spec-fixtures check` 与 `cargo clippy --all-targets -- -D warnings`。
- 已完成：回写 `TODO.md` / `PLAN.md`，将 `T4012c` 标记为完成，并顺手修正 `T4012b` 这个子任务已完成但父条目状态未同步的既有记录问题。
- 进行中：检查工作区并准备 Git 提交。

## 本轮目标（锁定）

### T4012c [TODO] 加入 built-in `@Experimental(val feature = "...")` annotation，作为保留的 feature-gate marker

- 当前判断：任务规模可控，先直接实现，不需要再拆子任务。
- 预期改动面：
  - built-in annotation 元数据与 typecheck 参数校验；
  - `sysroot/core.scoop` 中的 built-in annotation 声明；
  - `SCOOP_FULL_SPEC.md` / 相关文档说明；
  - parse/typecheck/必要 run-pass fixture。
