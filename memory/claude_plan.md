## 当前执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断第一个未完成任务。
2. 查看最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；若有，则按要求视为当前任务范围或其前置依赖。
3. 阅读该任务在 `TODO.md` 中的详细要求、依赖、验证标准，并检查相关代码与测试位置。
4. 实施该任务的最小正确改动；如果遇到阻塞当前任务的真实缺口或规范不匹配，则先在 `TODO.md` 中补入最小前置任务并停止。
5. 运行该任务要求的验证命令，以及必要的回归检查、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、相关测试。
6. 更新 `memory/claude_plan.md` 记录执行进展与关键决策。
7. 若任务完成，则把对应任务标题标记为 `[DONE]`，补全完成记录；仅当阶段计划发生变化时更新 `PLAN.md`。
8. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## 当前状态

- 已创建初始计划文件。
- 已读取 `TODO.md`，首个未完成任务为 `P0-T02R`：Review 审计脚手架与测试基线，确认后续任务不会被旧字符串绑定卡住。
- 最近一次提交主题为 `[P0-T02A] Record completion in memory plan`，未显式引入与 `P0-T02R` 直接相关的新未完成问题。

## P0-T02R 执行步骤

1. 复核 `crates/scoopc/src/llvm/tests.rs` 中 stable-id 审计 helper、source inventory 与相关行为测试，确认 external symbol/object 审计入口与旧命名解绑策略已经落地。
2. 复核 `crates/scoopc/src/cone/scoopir/schema.rs`、`pre_specialize.rs`、`visibility.rs`、`annotations.rs` 的基线测试，确认它们只承担健康 schema 防回归，不承担后续 schema 重写任务。
3. 重跑 P0-T01 / P0-T02 / P0-T02A 对应的测试命令与 grep 审计，记录 symbol、linkage、path-stability、dense-id 泄漏的基线入口。
4. 若 review 通过，则更新 `TODO.md` 将 `P0-T02R` 标记为 `[DONE]` 并填写完成记录；若发现阻塞后续任务的真实缺口，则按要求在 `TODO.md` 插入最小前置任务并停止。
5. 完成后再次更新本文件，记录关键发现、验证结果与最终提交状态。

## 当前复核结果

- 已完成的验证：
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_audit -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc external_symbol -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc stable_id_source_inventory -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc path_free -- --nocapture`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo test -p scoopc`
  - `LLVM_CONFIG_PATH="/opt/homebrew/Cellar/llvm@21/21.1.8/bin/llvm-config" cargo clippy -p scoopc --all-targets -- -D warnings`
- grep 审计关键摘要：
  - `TypeId\(` 2587 命中，`BasicBlockId\(` 45 命中，`module\.add_function\(.*None\)` 101 命中。
  - 当前重点 pattern 命中：`stable_template_symbol_suffix` 7、`source_path.*decl_span` 5、`scoop\.lambda\$[0-9]+` 2、`scoop\.lambda_resume\$[0-9]+` 1、`scoop\.lambda_env\$[0-9]+` 1、`__schema[0-9]+` 0、`__k[0-9]+` 0、`t[0-9]+__` 0。
- review 结论：`P0-T02R` 目前不能完成。
  - `stable_id_audit_*`、`external_symbol_*`、`stable_id_source_inventory_*` 与四个 `*_path_free`/JSON 基线测试本身工作正常。
  - 但复核发现 `crates/scoopc/src/llvm/tests.rs` 与 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 仍有 stable-id 敏感测试直接依赖当前 callable symbol 文本（例如 `@a.entry(`、`@a.keep(`、`@a.run(`、`@sample.effectEntry(`、`__scoop_refactor_direct_invoke__sample_effectEntry`）来定位函数或调用点。
  - 这会在后续 P1/P2 调整 user ABI / private symbol 命名时把无关语义测试一起打断，因此已在 `TODO.md` 中新增前置任务 `P0-T02B`，并把 `P0-T02R` 改为依赖该任务。
- `PLAN.md` 未改动：当前变化只影响 P0 内部任务顺序，不改变阶段级计划目标。
