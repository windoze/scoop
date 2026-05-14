# Claude Plan

## Planning Note

I will not record private chain-of-thought verbatim. This file tracks a concise reasoning summary, execution plan, and progress updates for the current invocation.

## Current Goal (P7-T02 — in progress)

Audit all user-visible failure paths and complete the final writeback so that the project ends in a state where:

- legal input → correct output;
- illegal input → explicit, stable error;
- everything else is treated as a compiler bug.

## Plan

1. Confirm P7-T02 is the first incomplete task (P7-T01 is `[DONE]`; P7-T02 only depends on it). No historical issue is currently blocking it.
2. Re-read `PIPELINE_GAPS.md` §0 / §9 and `PLAN.md` §0 / §5 / P7 to recover the contract guards I must verify.
3. Enumerate production-path failure-path hits with `rg 'UnsupportedMainBody|Unsupported[A-Za-z_]+|todo!|panic!|unreachable!' crates/scoopc/src`, then classify each hit into one of:
   - `internal bug sentinel`: contract-violation / impossible-state assertion that callers cannot reach with legal input;
   - `test-only helper`: only reached from `#[cfg(test)]` paths;
   - `should-have-been-frontend-diagnostic bug`: user-reachable on legal input — these must be fixed and tracked in `TODO.md`, not papered over.
4. Confirm the `FrontendReject` surface phrasing: `crates/scoopc/src/diagnostic` (or wherever the user-visible diagnostic text lives) must say “非法输入 / 当前语言 contract 不接受”, not “尚未支持”. Adjust any wording that still reads as "not yet supported".
5. Run the required validation matrix:
   - `cargo test --all --all-targets`
   - `cargo run -p scoop -- test`
   - `rg 'UnsupportedMainBody|Unsupported[A-Za-z_]+|todo!|panic!|unreachable!' crates/scoopc/src`
   - `cargo test -p scoopc llvm_tests`
6. Write the audit conclusions back into:
   - `PIPELINE_GAPS.md` (final user-visible failure surface narrative);
   - `PLAN.md` (only if phase-level conclusion changes);
   - `TODO.md` P7-T02 完成记录 (改动范围 / 核心决策 / 验证结果 / `PLAN.md` `PIPELINE_GAPS.md` 对应闭合);
   - flip the title from `[TODO]` to `[DONE]`.
7. The final summary in `TODO.md` and `PIPELINE_GAPS.md` must explicitly state whether the three conclusions hold:
   - 合法输入 → 正确输出;
   - 非法输入 → 明确错误;
   - 其它 → 编译器 bug。
8. Commit on the `eff` branch with a `[P7-T02]`-prefixed message and stop.

## Open Decision Points

- The classification audit may surface a hit that is genuinely a bug (i.e. user-reachable on legal input). If so, I must fix it before marking the task `[DONE]`, or insert a prerequisite task into `TODO.md` and stop.

## Progress Updates

1. Identified P7-T02 as the first incomplete task; P7-T01 already `[DONE]`. No historical bug currently blocks execution.
2. Audited the four `STALE_USER_VISIBLE_UNSUPPORTED_MARKERS` sites. Each one is post-upstream-validation: untyped struct primary-ctor params are gated by `typecheck::check_file_headers`'s `MissingTypeAnnotation`; the three `WhenPat::Variant` guards are gated by parser invariants and the `enum_instance_from_type` resolve contract. Replaced all four with documented `unreachable!` / `expect`.
3. Rephrased `LlvmEmitError::UnsupportedMainBody` user-visible message from "暂不支持的 main 代码生成节点：{kind}" to "编译器内部不变量被打破（compiler bug）：LLVM 主 codegen 收到本不应抵达的节点：{kind}（这表示上游 contract drift，不是合法语言特性）", aligning the user-facing wording with the §0 "contract-violation / impossible-state assertion" framing.
4. Updated the audit baseline (`pipeline_user_visible_failure_policy.rs`): emptied `STALE_USER_VISIBLE_UNSUPPORTED_MARKERS`; added `POST_UPSTREAM_VALIDATION_GUARDS` documenting the upstream gate for each of the four sites; expanded `INTERNAL_BUG_SENTINEL_HITS` from 20 → 23 entries (added `lower.rs:1181`, `when_pat.rs:209`, `when_pat.rs:258`, and shifted `lower.rs:2365/2967/3212` to `2363/2965/3210` to reflect line-count changes); added a new `pipeline_user_visible_failure_policy_documents_upstream_guards` test.
5. Validation:
   - `cargo test --all --all-targets`: 1078 / 0 / 0 (lib + e2e + ancillary); `pipeline_user_visible_failure_policy::*` 7/7, `pipeline_gap_audit::*` 5/5.
   - `cargo run -p scoop -- test`: 1269 fixture rows PASS, exit 0.
   - `cargo test -p scoopc --features llvm --lib llvm::`: 230 / 0.
   - `cargo clippy --all-targets --features llvm -- -D warnings`: clean.
   - `rg 'UnsupportedMainBody|Unsupported[A-Za-z_]+|todo!|panic!|unreachable!' crates/scoopc/src`: 2229 hits; production-side bucket fully matches `INTERNAL_BUG_SENTINEL_HITS` + `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` baselines (asserted by tests).
6. Wrote the final-state summary into `PIPELINE_GAPS.md` (new "P7-T02 终态" section at the top) and the four required completion-record sections in `TODO.md`. Three final conclusions explicitly stated as held: 合法输入→正确输出, 非法输入→明确错误, 其它→编译器 bug.
7. Marked P7-T02 `[DONE]` in `TODO.md`. Ready to commit on the `eff` branch with a `[P7-T02]` prefix and stop.
