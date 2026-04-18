# 本轮执行计划

说明：
- 按要求先写入本文件，再执行任何命令。
- 出于安全与策略限制，本文件不记录逐字内部思维，不暴露完整私有推理；改为记录可执行计划、关键判断、进展更新与必要决策依据。
- 本轮目标：只完成 `TODO.md` 中第一个未完成任务，然后停止。

## 初始计划

1. 检查最新一次 Git 提交信息，确认是否显式提到任何既有问题。
2. 若最新提交提到需先修复的既有问题，优先定位并修复这些问题；完成后再继续当前任务流。
3. 阅读 `TODO.md`，确定第一个未完成任务。
4. 评估该任务是否过大：
   - 若可在本轮完整完成，则直接实现。
   - 若过大，则拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只做拆分后的第一个子任务。
5. 实现目标任务，必要时补充或调整测试。
6. 运行相关验证，至少覆盖：
   - 直接相关测试；
   - 必要的构建检查；
   - `cargo clippy --all-targets -- -D warnings`（若范围允许且环境支持）。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中标记本轮完成项；
   - 在 `PLAN.md` 中更新当前状态、已完成内容与后续依赖；
   - 继续在本文件记录关键进展。
8. 生成一次 Git 提交，提交信息与任务编号/内容对应。
9. 停止，不继续执行下一个任务。

## 预设决策原则

- 不接受 workaround、shim、仅为夹具通过而做的规避实现。
- 若遇到规范缺口、实现边界、语言特性缺失或已有 bug 阻塞当前任务：
  - 先把阻塞问题写入 `TODO.md` 作为前置任务；
  - 调整依赖顺序；
  - 更新 `PLAN.md` 与本文件；
  - 提交后停止。
- 不回退或覆盖用户已有未相关改动。

## 进展记录

- 已创建本文件并写入初始计划。
- 已检查最新一次提交：提交信息为 `[T3016j] Unify closure non-resuming return contract`，提交说明中未额外列出必须先修复的既有问题。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T3016jR`（review 任务），无需再做任务拆分。
- 已开始审查 `T3016j` 的生产改动：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 新增 `setup_function_return_context()` 与 `emit_function_return_block()`，普通 top-level/helper 与 closure body 现共用同一套 ordinary-frame return contract。
  - `codegen_closure_fun_body()` 已接入 `return_bb` / `return_alloca` / `return_context`，不再在 non-resuming outward propagation 后直接落回裸 `emit_return()`。
  - `effect/mod.rs` 的 `emit_effect_propagation_return()` 仍只依赖通用 `current_fun_return_ty` + `return_context`，未看到按 closure 形状、fixture 名称或特定 helper 名称分流的新增逻辑。
- 已完成进一步审查：
  - `codegen_function_value_call()` 仍复用通用 `emit_ordinary_call_effect_propagation_check()`，没有 function-value-only propagation 补丁。
  - `emit_effect_propagation_return()` 继续只基于 `current_fun_return_ty` 与 `return_context` 做 outward propagation，不读取 closure/body 形状或测试夹具名称。
- 已完成验证：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_closure.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_local.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 以上均已通过。
- 已完成文档更新：
  - `TODO.md`：已将 `T3016jR` 标记为完成，并记录 review 结论。
  - `PLAN.md`：已记录本轮复审结论，并将下一执行项推进到 `T3017`。
- 下一步：
  1. 检查变更集。
  2. 提交本轮 `T3016jR`。
  3. 停止，等待下一轮。
