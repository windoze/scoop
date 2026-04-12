# 执行计划（公开摘要）

说明：这里记录的是可公开的执行计划、检查项、决策依据摘要和进度，不包含内部推理细节。

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务，并在完成后停止。

## 约束与执行顺序

1. 先检查最近一次提交，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 若该任务过大，则拆分为更小子任务，并同步更新 `PLAN.md` / `TODO.md`。
4. 实现当前应执行的任务，禁止用规避方案绕过规范缺口。
5. 运行相关验证，至少覆盖：
   - 受影响范围的测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如有必要，补充 fixture / spec 检查
6. 更新文档状态：
   - 在 `TODO.md` 标记完成，或在阻塞时重排依赖顺序
   - 在 `PLAN.md` 记录当前状态
   - 持续更新本文件记录进展
7. 提交 git commit，然后停止，不继续下一个任务。

## 初始检查清单

- [x] 查看最近一次提交的 message 和改动摘要
- [x] 阅读 `TODO.md`
- [x] 阅读 `PLAN.md`
- [x] 判断首个未完成任务是否可在本轮完整落地

## 风险检查

- 若发现规范与实现不一致，必须先把缺口写入 `TODO.md`，调整依赖顺序，再决定是否停止。
- 若存在用户已有未提交改动，避免覆盖或回退。
- 若测试或 lint 暴露既有问题，先判断是否属于当前任务前置阻塞。

## 进度记录

- 2026-04-12：初始化执行计划文件。
- 2026-04-12：已检查最近一次提交 `[T2003c0c2b3b3] Support no-immediate while-body direct escape sites`；提交消息与改动摘要未暴露额外未跟踪前置问题。
- 2026-04-12：已定位原首个未完成任务为 `T2003c0c2b3c`。经审计确认其同时覆盖 top-level multiple indirect、nested block、if branch、while body 四类 no-immediate indirect replay，单轮实现与回归面过大。
- 2026-04-12：已将 `T2003c0c2b3c` 拆分为 `T2003c0c2b3c1`～`T2003c0c2b3c4`，本轮执行新的首个子任务 `T2003c0c2b3c1`（top-level multiple indirect escape sites）。
- 2026-04-12：`T2003c0c2b3c1` 已完成。实现内容包括：
  - 新增 no-immediate top-level multiple indirect lowering 与 `pc` 状态机；
  - 让 escape-only multiple indirect 也复用这条新路径，修正旧 single-site indirect 分流；
  - 新增回归：`effect_multi_escape_indirect_multi`、`effect_multi_escape_custom_nonresuming_indirect_multi`。
- 2026-04-12：已完成验证：
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 2026-04-12：下一轮首个未完成任务已变为 `T2003c0c2b3c2`（nested block indirect escape sites）。
