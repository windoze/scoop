当前计划

1. 已读取 TODO.md，确认第一个未完成任务是 P6-T01R（Review global init/storage LIR facts contract）。
2. 接下来检查最近提交是否明确留下与 P6-T01R 直接相关的未完成事项；只纳入会影响该 review 的内容。
3. 阅读 TODO-6.md 中 P6-T01 与 P6-T01R 的完整要求、完成记录和验证要求，并补充读取必要的 PLAN/设计上下文。
4. 审查已发现当前 review 范围内的 verifier 缺口：global init facts 能发布合同，但 verifier 尚不能捕获 top-level eager contract 与 root 的 storage/initializer drift、eager root 遗漏/重复进入 cone routine、final entry order routine 遗漏/重复，以及 routine/root cone 不一致。
5. 接下来补齐 `scoopc_lir_facts` verifier 与单测，确保这些 drift 在 P6-T02/P6-T03 前暴露。
6. 已运行并通过：`cargo fmt`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features lir_facts_builder`、`cargo test -p scoopc_mir_facts`、`cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`、额外 `scoopc_lir_facts` residual 搜索、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
7. 已更新 TODO.md 和 TODO-6.md：P6-T01R 已标记为 [DONE] 并补充 completion record；本次不需要更新 PLAN.md。
8. 接下来检查 git 状态、diff 和最近提交，确认提交包含当前 review 所需的全部相关未提交文件。
9. 提交后停止，不开始 P6-T02。
