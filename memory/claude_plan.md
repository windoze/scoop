# 执行计划

## 说明

按要求先记录可公开的执行思路摘要与分步计划，再开始读取任务与执行命令。这里记录的是面向协作的计划与决策摘要，不包含逐词内部推理。

## 初始计划

1. 读取 `TODO.md`，确认第一个标题未带 `[DONE]` 的任务。
2. 检查最近提交信息，判断是否存在与该任务直接相关且明确未完成的问题；若有，则将其视为当前任务组成部分或按要求写入 `TODO.md` 作为前置。
3. 阅读与当前任务直接相关的代码、测试、文档与任务说明，确认约束、依赖、验收方式。
4. 如无阻塞，直接实现该任务；如有真实阻塞，按要求仅新增最小前置任务并更新 `TODO.md`/必要时更新 `PLAN.md`。
5. 运行该任务要求的验证，以及必要的回归测试、格式化、lint/编译检查，修复发现的问题。
6. 完成后更新 `TODO.md`：给任务标题加上 `[DONE]`，补全完成记录；仅在阶段计划变化时更新 `PLAN.md`。
7. 将本次相关改动提交到 git，提交信息使用任务号并准确描述本次完成内容。
8. 停止，不继续处理下一个任务。

## 进度记录

- 已读取 `TODO.md`，确认首个未完成任务为 `P2-T02`：收紧 production MIR verifier，拒绝 `unterminated` 与非 `Unit` 的 `Return { value: None }`。
- 最近提交为 `[P2-T01] Close comptime and top-level val MIR handoff gaps`，提交标题未显式声明与 `P2-T02` 直接相关的未完成前置问题；按当前信息继续执行 `P2-T02`。

## 当前任务摘要：P2-T02

### 目标

1. 让 production MIR verifier 将 `unterminated` 视为 hard failure。
2. 统一 `Return { value: None }` 规则：仅 `Unit` 返回允许省略值；非 `Unit` 必须显式携带返回值。
3. 删除 raw MIR codegen 中对 non-`Unit` `Return { value: None }` 的默认返回值兜底。
4. 为上述 contract 补负向测试，并验证相关 fixture 不回退。

### 预定执行步骤

1. 检查工作树状态，避免误覆盖已有改动。
2. 阅读以下入口并确认当前行为：
   - `crates/scoopc/src/mir/mod.rs`
   - `crates/scoopc/src/mir/placeholder_inventory.rs`
   - `crates/scoopc/src/llvm/codegen/mir_body.rs`
   - 与 `refactor_mir_no_todo*`、`while_break_continue`、`handle_perform` 相关测试。
3. 识别 `unterminated` 与 `ReturnNone` 的真实生产路径、测试覆盖和潜在耦合控制流。
4. 先在 verifier 层收紧 contract，再删除 codegen fallback；若这暴露出 CFG 生成漏洞，则一并修正产生该漏洞的路径。
5. 增补或收紧测试，确保 negative case 与正向 fixture 都覆盖。
6. 运行任务要求的测试、相关定向测试及 `cargo clippy --all-targets -- -D warnings`。
7. 回写 `TODO.md` 完成记录，并提交本次改动。

## 当前进度更新

1. 已完成 `crates/scoopc/src/mir/mod.rs` 修改：`validate_refactor_direct_style()` 现在会通过 forbidden-todo 规则直接拒绝 `unterminated` sentinel，并已补 direct-style 负例测试。
2. 已完成 `crates/scoopc/src/mir/materialize.rs` 修改：materialized MIR 现在会拒绝 non-`Unit` 的 `Return { value: None }`，并已补对应负例测试。
3. 已完成 `crates/scoopc/src/llvm/codegen/mir_body.rs` 修改：raw MIR codegen 不再为 non-`Unit` 空返回合成默认值，而是仅允许 `Unit` 空返回；并补了 helper 级单测。
4. 下一步：运行任务要求的定向测试与 `cargo clippy --all-targets -- -D warnings`，确认无编译/风格回归；随后回写 `PIPELINE_GAPS.md`、`TODO.md` 并提交。

## 验证与文档回写进度

1. 已完成验证：
   - `cargo test -p scoopc refactor_mir_no_todo`
   - `cargo test -p scoopc refactor_mir_no_return_none`
   - `cargo test -p scoopc refactor_mir_placeholder_inventory`
   - `cargo test -p scoopc refactor_materialized_mir`
   - `cargo test -p scoopc codegen_gap_inventory`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/while_break_continue.scoop`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor/handle_perform.scoop`
   - `cargo clippy --all-targets -- -D warnings`
2. 已完成文档回写：
   - `PIPELINE_GAPS.md`：`§2.1`、`§2.4` 已改为 `Closed/Re-scoped`。
   - `TODO.md`：`P2-T02` 已标记为 `[DONE]`，完成记录已补齐。
3. 剩余步骤：检查最终 diff 与工作树状态，按任务号创建 git commit，然后停止。
