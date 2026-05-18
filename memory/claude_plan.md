# 执行计划

说明：这里记录可审计的执行计划、决策摘要和进度更新；不会记录私密推理链。

## 初始计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未以 `[DONE]` 标记的任务。
2. 检查最近提交信息，仅在它明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 阅读当前任务涉及的源码、测试、规格或夹具，确认任务要求、依赖和验证命令。
4. 若任务可直接完成，则做最小正确实现，并补充或更新相关测试/夹具。
5. 运行当前任务要求的验证，以及必要的定向测试；若出现与当前任务相关的失败，优先修复根因。
6. 更新 `TODO.md`：将完成的任务标题加上 `[DONE]`，并填写完成记录。仅当阶段级计划变化时才更新 `PLAN.md`。
7. 按要求检查工作区差异，提交本次任务相关的全部变更，然后停止，不继续下一个任务。

## 当前状态

- 已识别第一个未完成任务：`P7-B2.8：B-08 internal/B-11 member store 与 pure/plain statement route contract`。
- 最近提交为 `[P7-B2.7] Retire extern runtime boundary UMB rows`，未显示与当前任务直接相关的未完成问题。
- 已实现初版 verifier/codegen 迁移：B-08 member-store contract 进入 MIR/materialized MIR validation，B-11 local val/assignment/while statement boundary 进入 HIR completeness validation；对应 LLVM `UnsupportedMainBody` 站点已替换为内部 invariant。
- 已完成 P7-B2.8 并更新 TODO：B-08/B-11 active count 均为 0，active inventory 633 -> 615，retired ledger 651 -> 669。

## P7-B2.8 执行计划

1. 读取 `PLAN.md` 对 P7-B2 的阶段要求，以及 `audit/strategies/B-08.md`、`audit/strategies/B-11.md` 和对应 category/fixture 文档。
2. 运行 `umb-audit list --bucket B-08` 与 `--bucket B-11` 锁定当前 active IDs、文件位置和 expected_class。
3. 定位 B-08/B-11 的 `LlvmEmitError::UnsupportedMainBody` 站点，确认已有 MIR/HIR verifier 覆盖情况。
4. 对缺失的 member store receiver/place/value invariant 与 pure/plain statement route boundary 增加 verifier 或内部 invariant，不用 fixture-only workaround。
5. 删除或替换对应 codegen fallback，并同步 active inventory、retired ledger、bucket/strategy 文档、fixture coverage 和 stale count baseline。
6. 运行任务指定验证及必要的定向测试，修复与当前任务相关的失败。
7. 将 `P7-B2.8` 标记为 `[DONE]` 并填写完成记录，随后提交本轮全部相关变更。

## 验证记录

- `cargo test -p scoopc mir::materialize -- --nocapture`：通过，54 passed。
- `cargo test -p scoopc pipeline::hir_stage -- --nocapture`：通过，30 passed。
- `cargo test -p scoopc audit:: -- --nocapture`：通过，23 passed。
- `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`：通过，7 passed。
- `cargo run -p scoop -- test tests/fixtures/umb_fix/B-08-member-store/`：通过，4 passed。
- `cargo run -p scoop -- test tests/fixtures/umb_fix/B-11-pure-boundary/`：通过，3 passed。
- `cargo run -p scoopc --bin umb-audit -- list --bucket B-08`：通过，entries 0。
- `cargo run -p scoopc --bin umb-audit -- list --bucket B-11`：通过，entries 0。
- `cargo run -p scoopc --bin umb-audit -- diff`：通过，in sync，615 entries。
- `cargo run -p scoopc --bin umb-audit -- stats`：通过，active=615、retired=669、initial=1284。
- `cargo fmt`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
