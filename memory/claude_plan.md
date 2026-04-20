# 本轮执行计划

说明：按要求维护执行计划、关键决策与进度记录。此文件记录的是可审计的计划与结论摘要，不包含逐字内部推理。

## 初始计划

1. 检查最新提交内容，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`、`PLAN.md`，识别第一个未完成任务，并判断是否需要拆分为更小子任务。
3. 如任务可直接执行，实施代码修改；如存在前置缺口或规范不匹配，先更新 `TODO.md` / `PLAN.md` 反映依赖关系。
4. 运行相关测试与必要的质量检查，至少覆盖受影响范围；若条件允许，补充更广的回归验证。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况、风险和后续状态。
6. 使用清晰的提交信息提交本轮改动，然后停止。

## 进度记录

- 已创建本计划文件，待开始仓库检查。
- 已检查最新提交 `164d5e2 [T4011a] Lower when variant payload subpatterns`；提交信息未额外声明需要先修复的既有问题。
- 已阅读 `TODO.md` / `PLAN.md` / `ISSUES.md`，确认当前首个未完成任务为 `T4011b`：在统一 payload matching 主线上收口“无 binder 的 payload or-pattern”。
- 已完成实现：LLVM enum `when` 条件判别现会让 `WhenPat::Or` 下的每个 variant 分支复用完整 payload 子模式匹配主线，不再只比较 tag。
- 已补充回归：
  - parser 单测：`parse_when_variant_payload_or_pattern`
  - typecheck fixture：`when_or_pattern_variant_payload_binder_is_error.scoop`
  - run-pass fixture：`when_or_pattern_variant_payload_basic.scoop`
- 已完成定向验证：
  - parser 单测通过。
  - 临时 fixtures root `cargo run -q -p scoop -- test --fixtures /tmp/t4011b-fixtures` 通过（`fixtures: ok (2)`）。
  - 最小 probe `Hit(0) | Miss()` 与 `Hit(0) | Hit(1)` 的误命中已修复，退出码分别变为 `2` / `8`。
  - 既有回归 `when_or_pattern_and_guard_basic.scoop` 与 `when_variant_payload_nested_tuple_basic.scoop` 继续通过。
- 已完成全量验证：
  - `cargo run -q -p scoop -- test` -> `fixtures: ok (1107)`
  - `cargo test --all -- --test-threads=1` -> 通过
  - `cargo clippy --all-targets -- -D warnings` -> 通过
- 已同步项目文档：
  - `TODO.md` 已将 `T4011b` 标记为完成并记录验证命令。
  - `PLAN.md` 已记录本轮实现结论，并将下一项推进到 `T4011R`。

## 当前执行计划（细化）

1. 阅读 `T4011b` 相关实现位置与现有回归，构造最小复现，确认当前失败点。
2. 修改 resolver / typecheck / HIR / LLVM 中与 `WhenPat::Or` 相关的实现，确保：
   - 无 binder 的 payload or-pattern 走统一 payload matching 主线。
   - 带 binder 的 or-pattern 继续报错。
   - 不放开 bare variant sugar。
3. 新增或更新 parse / typecheck / run-pass 回归，覆盖 payload 命中、wildcard payload、mismatch 路径。
4. 运行定向验证，再运行全量任务要求的测试与质量检查。
5. 更新 `TODO.md`、`PLAN.md`、本文件，提交本轮改动并停止。

## 收尾状态

- 当前代码与文档已就绪，下一步仅剩整理提交并停止在本轮任务边界。
