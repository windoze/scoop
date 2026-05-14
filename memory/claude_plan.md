## 当前计划

说明：按安全要求，这里记录的是可执行计划与关键判断摘要，不包含完整内部推理细节。

1. 先读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务；仅围绕该任务展开，不做开放式历史问题排查。
2. 查看最近提交信息，确认是否有与该任务直接相关且明确未完成的问题；若有，按用户规则视为当前任务的一部分或在 `TODO.md` 中登记为前置任务。
3. 阅读该任务涉及的说明、依赖、验证要求，以及必要的相关代码与测试位置，建立最小实现范围。
4. 实现该任务；若遇到会阻塞该任务的真实缺陷、规范不匹配或缺失能力，不做绕过，而是在 `TODO.md` 中以最小前置任务形式登记并调整顺序。
5. 运行与该任务相关的验证；至少覆盖任务要求中的测试，并补充必要回归测试。如需，运行 `cargo fmt`、相关测试、以及 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `memory/claude_plan.md` 记录关键进展；完成任务后在 `TODO.md` 中将任务标题前缀改为 `[DONE]` 并补全完成记录。仅在阶段计划发生变化时更新 `PLAN.md`。
7. 检查工作区改动，保留非本人改动不回退；按仓库提交风格创建一次提交，然后停止，不继续下一个任务。

## 当前进展

- 已确认首个未完成任务为 `P4-T02：收口 cleanup/unwind contract 与 main(args) plain routing`。
- 该任务依赖 `P4-T01`，当前已完成；本次只处理 `TODO.md` 中 `P4-T02` 要求的 cleanup/unwind contract 与 `main(args)` 路由问题。

## 下一步

1. 查看最近提交，确认是否有与 `P4-T02` 直接相关且明确未完成的说明。
2. 阅读 `PIPELINE_GAPS.md` §5.3、§5.4 以及 `PLAN.md` 对应 P4 段落，核对任务闭合条件。
3. 阅读 `crates/scoopc/src/llvm/codegen/effect_lowered/body.rs` 与 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 中相关测试/入口，定位 cleanup/unwind contract 与 `main(args)` 路由的现状。
4. 实现最小且完整的修复，随后执行任务要求中的测试与必要回归验证。

## 当前判断摘要

- 最近提交只明确完成了 `P4-T01`，未在提交信息中附带与 `P4-T02` 直接相关的未完成清单。
- `main(args)` plain routing 代码路径已经存在：`codegen_stage_main_exit_code(...)` 会在 callable 具备 plain ABI 时走 `codegen_refactor_plain_main_exit_code(...)`，`PIPELINE_GAPS.md §5.4` 也已标为 `Closed/Re-scoped`。
- 当前更可疑的未闭合点是 `PIPELINE_GAPS.md §5.3` 仍为 `Partial`；需要通过定向测试与代码核对确认 cleanup/unwind contract 是否已经完整，或是否仍有真实缺口需要修复。

## 已完成步骤

1. 已核对 `PLAN.md` / `TODO.md` / `PIPELINE_GAPS.md`，确认本轮只应处理 `P4-T02`。
2. 已检查最近提交，未发现需要先插入到 `P4-T02` 之前的直接相关未完成前置项。
3. 已阅读 `effect_lowered/body.rs` 与 `llvm_codegen_stage.rs`：
   - `main(args)` 目前已经通过 plain entry 路由。
   - `ResumeUnwind` 已有 verifier、origin/source 校验与返回路径 frame/handle cleanup 逻辑。
4. 已执行定向验证，结果全部通过：
   - `cargo test -p scoopc codegen_gap_inventory`
   - `cargo test -p scoopc pipeline_gap_audit`
   - `cargo test -p scoopc refactor_llvm_resume_unwind_lowering`
   - `cargo test -p scoopc refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry`
   - `cargo test -p scoopc refactor_llvm_main_wrapper_routes_unhandled_outward_to_exit_code`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_raise_cleanup_gc_basic.scoop`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_return_from_function_basic.scoop`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_return_from_function_finally.scoop`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/effect_handle_return_from_function_any_boxing.scoop`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/std_process_args_exit_basic.scoop`
   - `cargo clippy --all-targets -- -D warnings`
5. 已回写 `crates/scoopc/src/llvm/codegen_gap_inventory.rs`、`crates/scoopc/src/pipeline_gap_audit.rs`、`PIPELINE_GAPS.md`、`TODO.md`：
   - 将 `PIPELINE_GAPS §5.3` 从 live blocker 改为 closed guard。
   - 将 `P4-T02` 标记为 `[DONE]` 并补全完成记录。

## 剩余步骤

1. 检查当前工作区 diff / status，确认仅提交本轮改动。
2. 使用仓库风格创建 `P4-T02` 提交。
3. 停止，不继续下一个任务。
