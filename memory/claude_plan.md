# 本轮执行计划（可见摘要）

注意：这里记录的是可见的执行计划、决策依据摘要与进度，不包含内部隐藏推理。

## 目标

按照 `TODO.md` 的顺序，只完成第一个未完成任务；如果发现前置问题或依赖阻塞，则先处理阻塞并更新计划文件、`TODO.md`、`PLAN.md`，然后提交并停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否明确提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖和可能的拆分方式。
4. 如任务过大或存在缺失前置能力，则把任务拆成更小子任务，并同步更新 `TODO.md` 与 `PLAN.md`。

## 执行步骤

1. 实现当前要做的首个未完成任务。
2. 运行相关格式化、lint 和测试，至少覆盖：
   - 与改动直接相关的测试
   - 必要时运行工作区级检查，如 `cargo test --all`、`cargo clippy --all-targets -- -D warnings`
3. 修复测试或 lint 中发现的问题，直到结果满足要求。
4. 更新进度文档：
   - 在 `TODO.md` 标记任务完成，或在阻塞场景下重排任务顺序
   - 在 `PLAN.md` 记录当前状态与后续安排
   - 回写本文件，记录关键进展与计划调整
5. 提交当前改动并停止，不进入下一个任务。

## 风险检查

1. 若最新提交暴露了未解决问题，则这些问题优先于 `TODO.md` 任务处理。
2. 若任务依赖尚未实现的语言特性或库支持，则不强行实现任务，而是调整 `TODO.md` / `PLAN.md` 体现依赖关系。
3. 不回退用户已有改动；若工作区存在无关脏改动，只在理解影响后规避冲突。

## 进度记录

- 已检查最新一次提交：
  - 提交为 `443ea8b Update plan`
  - 提交说明本身没有额外描述待修复 bug
  - 该提交新增/重写了 `ISSUES.md`、`TODO.md`、`PLAN.md`，因此本轮按这些文件定义的缺口推进
- 已读取 `TODO.md` / `PLAN.md`：
  - 当前首个未完成任务是 `T2001`：统一 `handle` arm 形态与 typecheck/HIR 不变量
  - 当前判断：任务边界清晰，先不拆分；如实现中发现规模过大或缺失前置能力，再回写拆分结果
- 已完成的实现：
  - `crates/scoopc/src/typecheck/expr/infer.rs`
    - 删除 mixed-arm 的统一 early reject
    - `handle` 结果类型改为按真实可返回路径确定，`Nothing` 路径不再把结果类型锁死
    - 为重复 handler head 增加稳定的不可达诊断
  - `crates/scoopc/src/hir/mod.rs`
    - 更新注释，明确 HIR 保留三类 arm 的语义形态与 binder 信息
  - 新增 fixtures：
    - `tests/fixtures/typecheck/handle_mixed_arm_kinds_ok.scoop`
    - `tests/fixtures/typecheck/handle_mixed_arm_return_type_mismatch_is_error.scoop`
    - `tests/fixtures/typecheck/handle_mixed_arm_resume_mode_unreachable_is_error.scoop`
    - `tests/fixtures/hir/handle_mixed_arm_kinds.scoop`
    - `tests/fixtures/hir/handle_mixed_arm_kinds.hir`
- 已完成的定向验证：
  - 临时 `typecheck/` fixtures：`fixtures: ok (3)`
  - 临时 `hir/` fixtures：`fixtures: ok (1)`
- 已完成的工作区级验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test` → `fixtures: ok (906)`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 当前状态：准备更新 `TODO.md` / `PLAN.md`，标记 `T2001` 已完成，然后提交并停止。
