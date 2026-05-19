# 执行计划

说明：这里记录可审计的执行计划、决策摘要和进度更新；不会记录私密推理链。

## 初始计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未以 `[DONE]` 标记的任务；不进行开放式历史问题扫荡。
2. 检查最近提交信息，仅在它明确提到与当前任务直接相关的未完成问题时，将其纳入当前任务或作为前置项记录到 `TODO.md`。
3. 阅读当前任务涉及的源码、测试、规格、审计清单或夹具，确认任务要求、依赖、禁止 workaround 的边界和验证命令。
4. 若任务可直接完成，则做最小正确实现；若发现阻塞当前任务的缺失特性或规格不匹配，则在 `TODO.md` 中插入最小必要前置任务并停止。
5. 运行当前任务要求的验证，以及必要的定向测试；若出现与当前任务相关的失败，优先修复根因。
6. 更新 `TODO.md`：完成时将任务标题加上 `[DONE]` 并填写完成记录；仅当阶段级计划或依赖结构变化时更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关的全部变更，然后停止，不继续下一个任务。

## 当前状态

- 已识别第一个未完成任务：`P7-B3.1：B-32/B-31 print/panic/sysroot 与 scalar methods contract`。
- 最近提交为 `[P7-B2.8] Retire member statement UMB rows`，未显示与当前任务直接相关的未完成问题。
- 已锁定 active IDs：B-32 `UMB-0543`..`UMB-0552` 10 条；B-31 `UMB-0575`..`UMB-0580`、`UMB-0584`..`UMB-0587`、`UMB-0591`..`UMB-0592` 12 条。
- 已把 `crates/scoopc/src/llvm/codegen/intrinsics/builtin.rs` 中对应 sysroot print/panic 与 scalar methods fallback 改为 verifier-backed internal invariant；B-30/B-24 相邻 rows 保持 active。
- 已同步 inventory、retired ledger、bucket/strategy 文档、fixture coverage 和 `TODO.md` 完成记录；B-31/B-32 active count 均为 0，active 615 -> 593，retired 669 -> 691。
- 已完成验证并准备检查 diff/status 后提交。

## P7-B3.1 执行计划

1. 用 `umb-audit list --bucket B-32` 和 `--bucket B-31` 锁定 active IDs、source file、expected_class 和当前 fallback 位置。
2. 阅读 B-32/B-31 strategy/category 文档与 `tests/fixtures/umb_fix/B-32-print-panic-sysroot/`、`B-31-scalar-methods/`，确认 fixture 应从 ignored/pending 状态切换到 active 的范围。
3. 定位 `LlvmEmitError::UnsupportedMainBody` 站点，判断哪些属于 sysroot print/panic bridge、哪些属于 scalar extension methods；优先补齐上游 verifier/typecheck/sysroot contract，LLVM 处只保留内部 invariant。
4. 做最小正确 production 改动并同步 audit 数据：active inventory 删除对应 rows，retired ledger 增加 22 个 ID，bucket/strategy/overview/spec coverage/fixture index/stale count 同步。
5. 运行任务指定验证和必要的定向测试，必要时补充 targeted tests，最后执行 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`。
6. 将 `P7-B3.1` 标记 `[DONE]` 并填写完成记录；检查 `git diff`/`git status`，提交本次任务相关变更后停止。

## 验证记录

- `cargo run -p scoopc --bin umb-audit -- list --bucket B-31`：通过，entries 0。
- `cargo run -p scoopc --bin umb-audit -- list --bucket B-32`：通过，entries 0。
- `cargo run -p scoopc --bin umb-audit -- stats`：通过，active=593、retired=691、initial=1284。
- `cargo run -p scoopc --bin umb-audit -- diff`：通过，in sync，593 entries。
- `cargo run -p scoop -- test tests/fixtures/umb_fix/B-31-scalar-methods/`：通过，3 passed。
- `cargo run -p scoop -- test tests/fixtures/umb_fix/B-32-print-panic-sysroot/`：通过，3 passed。
- `cargo run -p scoop -- test tests/fixtures/run-pass/scalar_method_intrinsic_basic.scoop`：通过，1 passed。
- `cargo run -p scoop -- test tests/fixtures/run-pass/string_byte_accessors.scoop`：通过，1 passed。
- `cargo test -p scoopc audit:: -- --nocapture`：通过，23 passed。
- `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`：通过，7 passed。
- `cargo run -p scoop -- test tests/fixtures/umb_fix/`：通过，149 passed。
- `cargo fmt`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
