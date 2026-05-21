# 当前执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序来源，完成第一个标题未带 `[DONE]` 的任务后停止。
- 本次任务为 `P4-T01R：Review scoopc_effect_facts crate 与事实模型`。
- 最近提交 `1fe7e259 [P4-T01] Add effect facts fact crate` 是本次 review 的直接对象。

## 执行步骤

1. 读取 `TODO.md` / `TODO-5.md`，确认 `P4-T01R` 的 review 范围、依赖和验证命令。
2. 复查 `scoopc_effect_facts` crate、workspace manifest、`scoopc` adapter、`scoopc_ids` stable identity、dependency gate 与 README。
3. 确认 fact crate 只依赖基础 crate，不引用 facade、stage、backend、MIR/LIR 内部类型或其它 fact crate。
4. 重新运行 P4-T01 的验证命令，并额外运行 `cargo tree -p scoopc_effect_facts`。
5. 若发现阻塞项，在本 review 内修复；若无阻塞项，更新 `TODO.md` / `TODO-5.md` 完成记录。
6. 检查 diff/status，提交本次 review 记录并停止。

## 当前状态

- 已确认 `scoopc_effect_facts` 的 workspace 依赖只有 `scoopc_ids`、`scoopc_types` 及基础依赖 `scoopc_span`。
- 已确认 dependency gate 将 `scoopc_effect_facts` 纳入 fact crate 检查，并拒绝 fact crate 依赖 facade、stage/backend crate 或其它 fact crate。
- 已检查 published facts 数据模型覆盖 snapshot binding、callable/body/site facts、step schema、continuation schema、dump 与 verifier skeleton。
- 已确认 `MaterializedEffectFacts::to_published_effect_facts(...)` 作为当前生产 adapter 会对适配后的独立 fact 产品运行 verifier。
- 已运行验证：`cargo fmt`；`cargo check -p scoopc_effect_facts`；`cargo test -p scoopc_effect_facts`；`cargo test -p scoopc --no-default-features effect_facts_stage`；`cargo run -p scoop_tools -- dependency-gate`；`cargo tree -p scoopc_effect_facts`；`cargo clippy --all-targets -- -D warnings`；`git diff --check`。
- Review 结论：未发现 `P4-T01R` 阻塞项；`P4-T01` 遗留的 nested `MirStageOutput` 与 mutable MIR snapshot 输入仍按计划留给 `P4-T02` / `P4-T03`，未被视为长期合法边界。
- 已将 `P4-T01R` 在 `TODO.md` 和 `TODO-5.md` 标记为 `[DONE]` 并填写完成记录；下一步提交本次 review 变更。
