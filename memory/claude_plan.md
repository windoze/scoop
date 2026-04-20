# 执行计划记录

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在进入任务实现前，先检查最新一次提交是否提到了需要先修复的既有问题；若有，先修复这些问题。
- 如果首个未完成任务过大或被前置缺陷阻塞，需要先拆分任务或补充前置任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 所有输出与记录均使用中文。

## 初始执行步骤

1. 查看最新一次 Git 提交，确认提交信息与变更中是否提到必须先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与 `TODO.md` 是否一致。
4. 结合任务与代码现状判断：
   - 若任务可直接完成：开始实现、补测试、运行相关验证。
   - 若任务过大：拆分为更小子任务，并更新 `PLAN.md` / `TODO.md`。
   - 若存在规范缺口、实现缺陷或前置依赖：先把缺口转成更靠前的任务，更新 `PLAN.md` / `TODO.md`，本轮只处理新的首个任务或在受阻时提交规划调整。
5. 完成实现后：
   - 运行必要测试与 `cargo clippy --all-targets -- -D warnings`（若影响范围要求）。
   - 更新 `TODO.md`、`PLAN.md`、本文件。
   - 提交 Git commit，并停止。

## 记录方式

- 我会在关键节点更新本文件：完成检查、确定首任务、发现阻塞、开始实现、完成测试、准备提交。
- 这里记录的是可审阅的执行思路、决策依据与步骤，不包含逐字逐句的内部推理草稿。

## 当前进展（2026-04-20）

- 已检查最新提交 `6bb6ecee2e0c7cab16a342ed318e5405a13b326c`，提交信息为 `[T4011a] Insert payload-matching blocker before or-pattern`。
- 该提交没有留下“已知但未修复的代码问题清单”，核心动作是把一个真实前置缺口显式插入 `TODO.md` / `PLAN.md`，因此当前应先执行这个 blocker，而不是继续推进原先更高层的 or-pattern 任务。
- 已确认 `TODO.md` 中第一个未完成任务为 `T4011a`：先收口 `when` enum variant payload 的无 binder 子模式匹配。

## 当前任务的细化计划

1. 复现 `T4011a` 描述的最小失败场景，确认当前是：
   - 单分支 `Some(0)` / `One((0, _))` 在 LLVM codegen 报 `when variant arg pattern` unsupported；
   - 现有 or-pattern 仅比较 tag，未递归判别 payload。
2. 阅读相关实现：
   - `typecheck/when_pat.rs`
   - `resolve/scopes.rs`
   - `hir/lower/*` 中与 `when` / pattern lowering 相关代码
   - `llvm/codegen/*` 中 `bind_when_pat` / enum tag 判别路径
3. 判断缺口是否还能继续细分：
   - 若“单分支 payload 子模式匹配”本身仍过大，则继续拆分并回写 `TODO.md` / `PLAN.md`；
   - 若已有统一抽象可扩展，则直接实现 `T4011a`。
4. 实现时坚持统一主线：
   - 先让单分支 enum variant payload pattern 支持 literal / wildcard / tuple / nested variant 的无 binder 子模式匹配；
   - 不在 `WhenPat::Or` 上做绕过底层缺口的特判。
5. 补充回归：
   - payload 命中；
   - payload mismatch；
   - nested payload 子模式；
   - 避免把 `Some(1)` 错判成 `Some(0)` 命中。
6. 运行定向测试、相关 fixture 集、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`（若改动面要求）。
7. 完成后更新 `TODO.md`、`PLAN.md` 与本文件，提交 commit，然后停止。

## 当前发现

- 已用两个最小 probe 复现当前失败：
  - `Some(0)` 对 `Some(1)` 的单分支 payload literal 模式；
  - `One((0, _))` 的 tuple payload 子模式。
- 两者当前都会在 LLVM 阶段报：
  - `scoop::llvm::unsupported_main_body: when variant arg pattern`
- 代码定位结果：
  - `typecheck/when_pat.rs` 已递归接受 enum variant payload 子模式，因此 blocker 不在 parser/typecheck。
  - `llvm/codegen/control_flow.rs` 的 enum `when` 仍主要停留在“按 tag 分派 + 仅支持 binder/_/.. payload”的阶段：
    - `codegen_when_pat_cond_for_enum_with_tag` 只比较 variant tag，不递归检查 payload；
    - `bind_when_pat` 在遇到 payload 中的 literal / tuple / nested variant 时直接报 `when variant arg pattern`。
- 当前判断：`T4011a` 不需要再拆分，可以在现有 `when` codegen 上增量收口。
- 实现边界：
  - 本轮只修复“单分支 enum variant payload 的无 binder 子模式匹配”；
  - 不主动把 `Some(0) | None()` 这类 payload or-pattern 一并标记完成，保持 `T4011b` 作为下一项任务。

## 完成记录

- `T4011a` 已完成。
- 实现摘要：
  - `crates/scoopc/src/llvm/codegen/control_flow.rs` 中，顶层 enum variant 且 payload 含非平凡子模式的 `when` arms 现会切到链式判别 CFG。
  - 单分支 enum variant pattern 的条件生成不再只比较 tag，而是会在 tag 命中后递归检查 payload 子模式。
  - `bind_when_pat` 现通过共享 payload 提取 helper 递归复用 tuple / nested variant 绑定逻辑，不再对 literal / tuple / nested variant payload 子模式直接报 unsupported。
  - niche `Option<T>` payload 额外补上了“payload 本身仍是 niche enum”的提取路径，因此 `Option<Option<Bool>>` 形式的 nested variant 也可走统一 matching 主线。
- 新增回归：
  - `tests/fixtures/run-pass/when_variant_payload_literal_mismatch_basic.scoop`
  - `tests/fixtures/run-pass/when_variant_payload_nested_tuple_basic.scoop`
  - `tests/fixtures/run-pass/when_variant_payload_nested_variant_basic.scoop`
- 验证结果：
  - 新增 3 条 run-pass fixture 均按预期退出码通过。
  - 既有 `when_switch_basic.scoop` 与 `when_or_pattern_and_guard_basic.scoop` 继续通过。
  - `cargo run -q -p scoop -- test` 通过（`fixtures: ok (1105)`）。
  - `cargo test --all -- --test-threads=1` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。

## 后续登记

- 在设计更广的 nested-variant probe 时，额外暴露出一个不阻塞本轮完成的独立缺口：
  - boxed enum payload object / type descriptor 遇到 builtin enum field 时仍可能报 `struct field type`；
  - 一般 nested enum payload（不止 niche-nested）仍未完全收口。
- 该缺口已登记为 `TODO.md` 中的后续任务 `T4011S`，位置放在 `T4011R` 之后，不影响下一轮先执行 `T4011b`。
