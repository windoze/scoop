# 本轮执行计划

更新时间：2026-04-18

## 约束与目标

- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在执行任务前，先检查最新提交是否提到已有问题；若有，这些问题优先纳入本轮范围并修复。
- 任何发现的规格偏差、缺失能力或阻塞项，都必须先体现在 `TODO.md` / `PLAN.md` 中，再决定是否继续。
- 实现后必须完成相关测试、更新文档状态，并提交 Git commit。

## 决策摘要

- 先读取最新一次提交信息，确认是否显式提到遗留问题或待修复项。
- 再读取 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并判断任务是否足够小、是否有前置依赖。
- 如果任务过大或被真实缺陷阻塞，则将任务拆分或重排，并同步更新 `TODO.md` 与 `PLAN.md`，本轮只处理新的第一个可执行子任务。
- 如果任务可执行，则直接实现、补充或调整测试、运行必要的格式化/检查/测试命令，直到结果稳定。
- 完成后更新 `TODO.md`、`PLAN.md`、本文件，并创建一次清晰的 Git 提交，然后停止。

## 具体步骤

1. 检查最新提交内容与提交说明。
2. 读取 `TODO.md`、`PLAN.md`，确认当前优先级最高的未完成任务。
3. 评估该任务：
   - 是否可在本轮完整完成；
   - 是否存在前置规格缺口、实现缺口或测试缺口；
   - 是否需要先拆分任务。
4. 若需拆分或重排：
   - 更新 `TODO.md`；
   - 更新 `PLAN.md`；
   - 在本文件记录原因；
   - 处理拆分后排在最前的那个任务。
5. 实现代码改动。
6. 运行格式化、静态检查和与该任务相关的测试；若失败则继续修复直到通过。
7. 更新 `TODO.md`、`PLAN.md` 与本文件的进度记录。
8. 检查工作区差异，确认仅包含本轮应提交内容。
9. 创建 Git 提交并停止。

## 进度记录

- 已创建本计划文件。
- 已读取最新提交、`TODO.md` 与 `PLAN.md`。
- 最新提交说明未显式挂出需要先修复的新遗留 issue；当前 `TODO.md` 中最前的未完成任务为 `T3009b2cR`。
- 本轮目标已收敛为：复审 multi-site ordinary indirect callee 的 resumed-body caller-tail 是否真正统一接回；若复审发现真实生产缺口，则先修复该缺口再完成本轮任务。

## 当前复审步骤（T3009b2cR）

1. 审查最近为 `T3009b2c` 修改的生产代码：
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
   - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
   - `crates/scoopc/src/llvm/codegen/mod.rs`
2. 审查与该任务直接相关的定向测试与 fixture，确认覆盖的是统一 resumed-body caller-tail 合同，而不是特化补丁。
3. 运行与该任务匹配的定向验证：
   - multi-site ordinary indirect callee focused fixture
   - statement-container matrix
   - 相关 IR / 单元测试
4. 若复审发现缺口，直接修复并重复第 1-3 步；否则更新 `TODO.md` / `PLAN.md`，将 `T3009b2cR` 标记完成。

## 复审结果

- 已完成生产代码复审：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs`
- 已确认 multi-site ordinary callee 的核心合同仍是统一的：
  - plan builder 只按 `builder.suspend_sites` 生成 `resume_sites`；
  - fresh path 只保存 `site_tag + union locals`；
  - resume path 只读取 `site_tag`，并经共享 `codegen_callee_resume_dispatch` 分派到对应 `resume_tail`。
- 已确认 top-level helper 与 closure body 共用同一套 suspend/resume 入口；function-value callee 继续通过 closure body codegen 复用该机制。
- 已检索生产代码，未发现 fixture/helper 名称或 branch 数量驱动的特判回流。

## 已完成验证

- `cargo test -p scoopc ordinary_multi_site_callee_materializes_resume_site_dispatch -- --nocapture`
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_multi_site_callee_branch.scoop`
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_statement_container_matrix.scoop`
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_escape_indirect_callee_suspend_matrix.scoop`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

## 当前状态

- 未发现需要在 `T3009b2cR` 内额外修复的生产缺口。
- `TODO.md` / `PLAN.md` 已同步完成，`git diff --check` 通过。
- 下一步：创建本轮提交并停止。
