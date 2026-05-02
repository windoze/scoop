# 本次执行计划

说明：按安全与协作约束，这里记录可审计的执行计划、关键判断依据与进度更新，不记录原始内部推理。

## 初始计划

1. 读取 `TODO.md`，确认详细任务文件索引与顺序。
2. 依次检查对应 `TODO-Px.md`，定位第一个未完成的详细任务。
3. 查看最近提交，判断是否存在与该任务直接相关且尚未收尾的问题；若有，将其视为当前任务的一部分或在详细任务文件中补充为前置依赖。
4. 阅读当前任务涉及的代码、测试、文档与约束，确认实现边界。
5. 直接完成该任务；若遇到阻塞当前任务的真实缺口，则先修复该缺口，或在相应 `TODO-Px.md`/`TODO.md` 中新增最小前置任务并停止。
6. 运行与任务相关的验证；若任务范围涉及通用质量门禁，再补充运行格式化、测试与 `clippy` 检查，修复出现的问题。
7. 更新 `TODO-Px.md` 的完成记录；如任务索引、标题、顺序或依赖发生变化，同步更新 `TODO.md`；仅在阶段计划真正变化时更新 `PLAN.md`。
8. 提交本次更改并停止，不继续下一个任务。

## 进度日志

- 已创建本文件，准备开始读取任务索引。
- 已读取 `TODO.md` 与 `TODO-P0.md`~`TODO-P3.md` 的完成记录。
- 当前首个未完成详细任务：`P3-T02R`（位于 `TODO-P3.md`）。
- 最近提交为 `[P3-T02] Lower refactor MIR from typed contracts`；提交主题与当前 review 直接相关，但尚未看到提交信息中显式记录的额外未完成事项，后续会在代码与验证中继续核对。

## 当前执行计划（P3-T02R）

1. 复读 `TODO-P3.md` 中 `P3-T02` / `P3-T02R` 条目，明确 review 检查点与禁止项。
2. 检查 `crates/scoopc/src/mir/lower.rs`、`crates/scoopc/src/mir/mod.rs`、`crates/scoopc/src/mir/materialize.rs`、refactor MIR stage 模块、P2 typed contract side table 定义，确认 refactor MIR 是否以 typed contract 为 authoritative 输入。
3. 搜索旧式猜测入口（`continuation_resume_call_spans`、`non_pure_continuation_resume_call_spans`、`is_continuation_resume_call`、`resume callee lowering pending`），判断这些命中是否仍影响 refactor MIR 主路径。
4. 重新运行 `P3-T02` 指定测试与命令；若 review 发现阻塞当前任务的真实缺陷，则优先修复，或按要求补充前置任务并同步 TODO。
5. 若 review 通过，则回写 `TODO-P3.md` 的 `P3-T02R` 完成记录；如任务索引/顺序未变，不改 `TODO.md`；若阶段计划未变，不改 `PLAN.md`。
6. 运行必要质量门禁并创建提交，然后停止。

## 新发现 / 计划调整

- 审查发现：`crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs` 虽已向 `MirLoweringFacts` 注入 typed contracts，但 `crates/scoopc/src/mir/lower.rs` 仍保留 legacy resume span fallback、legacy perform site fallback，以及缺失 typed handle metadata 时回退到 HIR 重建 contract 的路径。
- 这与 `P3-T02R` “refactor MIR 不再依赖 span / 名字 / HIR fallback 猜语义”的完成条件直接冲突，因此本次先修正这些 fallback 的 refactor 可达路径，并补充/复验定向测试，再回写 review 完成记录。

## 已完成步骤

1. 已把 refactor MIR stage 的 facts 构造改为 `MirLoweringFacts::from_refactor_typed_handoff(...)`，不再从 legacy resume/effect fallback 表中起步。
2. 已在 `crates/scoopc/src/mir/lower.rs` 中显式区分 legacy fallback 与 refactor typed-contract 模式：
   - refactor 模式下不再使用 legacy resume span fallback；
   - refactor 模式下若缺失 perform/handle typed contract，会显式产生 contract-missing `Todo(...)`，而不是静默回退到 HIR/side-table 猜测；
   - 新增单元测试覆盖“切到 refactor typed contracts 时 legacy fallback 被清空”。
3. 搜索复核完成：`rg -n "continuation_resume_call_spans|non_pure_continuation_resume_call_spans|is_continuation_resume_call|resume callee lowering pending" crates/scoopc/src/mir crates/scoopc/src/effect_refactor_pipeline` 现在只命中 `crates/scoopc/src/effect_refactor_pipeline/mir_stage.rs` 中的测试断言，不再命中 refactor MIR stage 的直接 lowering 逻辑。
4. 定向验证已通过：
   - `cargo test -p scoopc --no-default-features refactor_typed_contracts_clear_legacy_resume_and_perform_fallbacks`
   - `cargo test -p scoopc --no-default-features refactor_mir_lowering_contract`
   - `cargo test -p scoopc --no-default-features refactor_direct_mir_stage`
   - `cargo test -p scoopc --no-default-features effect_refactor_pipeline`
   - `cargo test -p scoop --no-default-features dump_mir`
   - `cargo test -p scoop --no-default-features parity`
   - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/direct_and_fun_value_call.scoop`
   - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/dispatch_and_resume_call.scoop`
   - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir/handle_perform.scoop`
   - `cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/continuation_resume_unit_sugar.scoop`
   - `cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-mir tests/fixtures/mir/dispatch_and_resume_call.scoop`
   - `cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`
