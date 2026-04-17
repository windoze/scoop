# 执行计划（初始版本）

## 目标
- 按 `TODO.md` 的顺序只完成第一个未完成任务。
- 在开始实际实现前，先检查最新一次提交是否提到已知问题；若提到且仍存在，则这些问题优先纳入本次处理范围。
- 若首个未完成任务过大，则先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。

## 初始步骤
1. 检查最新一次 Git 提交的提交信息与变更摘要，确认是否明确提到尚未解决的问题、回退、临时方案或后续待修复项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划、任务编号与依赖关系。
4. 结合代码现状判断该任务是否可在本轮完整完成：
   - 如果可以，则直接实现。
   - 如果过大或存在前置缺陷/缺失能力，则先在 `TODO.md` / `PLAN.md` 中拆分或重排，再只处理新的第一个任务。
5. 实现任务并补充/调整测试。
6. 运行必要验证，至少覆盖：
   - 相关定向测试；
   - `cargo test --all`（若成本可接受且与改动相关）；
   - `cargo clippy --all-targets -- -D warnings`（按仓库要求尽量满足无 warning）。
7. 更新文档状态：
   - 在 `TODO.md` 标记完成；
   - 在 `PLAN.md` 反映当前状态；
   - 持续更新本文件记录进度与计划变化。
8. 提交 Git commit，然后停止，不继续下一个任务。

## 当前已知约束
- 目前尚未查看仓库状态，因此此计划是基于任务说明的初始执行计划。
- 在读取 `TODO.md`、`PLAN.md`、最新提交与相关代码后，我会把更具体的实现路径、风险和验证范围补充到本文件。

## 进度记录
- 已创建本文件并写入初始计划。
- 已检查最新提交 `2fa5468 [T3011R] Review frame slot metadata declaration authority`：
  - 提交信息本身没有引入新的未跟踪 issue。
  - `PLAN.md` / `TODO.md` 中提到的 stale `EXPECT: fail` `continuation_resume_continuation.scoop` 已有明确后续任务 `T3017` 跟踪，不属于“最新提交新增但未入列”的问题。
- 已确认当前第一个未完成任务是 `T3012`。
- 已完成的定向复现：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop`：通过。
  - `cargo run -p scoop -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop`：通过。
  - `cargo run -p scoop -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`：失败，报 `暂不支持的 main 代码生成节点：value coercion`。
- 已完成 `run-pass` 下全部 `EXPECT: fail` fixture 的快速扫描：
  - 共 101 个，当前 87 个已经直接通过。
  - 失败类别统计显示：
    - 复合 payload 运输相关：`u64 word from composite value`、`narrow u64 to composite type (not yet supported)`。
    - outer-scope frame seeding：`effect frame seed outer-scope local`。
    - 其他零星类别：`call callee`、`when arm type mismatch`、一个 typecheck 错误、一个未知错误。
    - 与 `T3012` 直接相关的 expected-context/coercion 类别只剩 1 个：`continuation_resume_enum.scoop` 的 `value coercion`。
- 关键判断：
  - `effect/mod.rs` 中 `codegen_continuation_resume_builtin()` 的注释已明确写明：composite payload 仍要等 `T3013` / `T3009b`。
  - 同文件 `coerce_u64_word()` 与 `state_machine_emitter.rs` 的 `narrow_u64_word_to_cg_value()` 对 tagged-union enum 仍只保留 tag，不负责 richer payload transport。
  - 因此 `continuation_resume_enum.scoop` 当前不是 `T3012` 的 expected-context/closure 缺口，而是后续 `T3013` + `T3009b` 的 composite resume payload 工作。
- 当前执行决策：
  - 不对生产代码做 workaround。
  - 更新 `TODO.md` / `PLAN.md`，把 `T3012` 的验收边界收窄到已验证通过的 expected-context / closure / coercion 范围，并把 `continuation_resume_enum.scoop` 明确留给已有的后续任务 `T3013` / `T3009b`。
  - 将 `T3012` 标记完成后提交，本轮停止，不进入 `T3012R`。
