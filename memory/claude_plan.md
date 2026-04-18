# 本轮执行计划（初始化）

更新时间：2026-04-18

说明：
用户要求先写入“完整思考过程”。我不会记录逐字内部推理，但会持续维护足够详细的决策摘要、执行步骤、关键发现、计划变更与完成状态，供你检查进度。

## 当前目标

按 `TODO.md` 的顺序完成第一个未完成任务；若发现前置缺陷、规范不匹配或任务过大，则先按要求调整 `TODO.md` / `PLAN.md`，提交后停止。

## 预定步骤

1. 检查最新一次 git 提交，确认是否提到现存问题或已知缺陷。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读与该任务直接相关的 `PLAN.md`、`README.md`、实现代码与测试，建立上下文。
4. 判断该任务是否可在本轮完整完成。
5. 若任务过大：
   - 细化为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md` 的任务拆分与顺序；
   - 执行新的第一个子任务。
6. 若存在前置缺陷、规范偏差或缺失特性阻塞当前任务：
   - 先把缺陷转成更靠前的任务；
   - 更新 `PLAN.md` 说明阻塞关系；
   - 仅提交计划与任务调整，然后停止。
7. 若任务可执行：
   - 实现代码；
   - 补充或更新测试；
   - 运行格式化、测试与必要的 lint / clippy；
   - 修复所有发现的问题直到通过。
8. 更新文档状态：
   - 在 `TODO.md` 标记当前任务完成；
   - 在 `PLAN.md` 记录进展；
   - 在本文件记录关键步骤与结果。
9. 使用清晰的提交信息提交本轮变更。
10. 停止，不继续处理下一个任务。

## 待确认事项

- 最新提交是否声明了必须先修复的问题。
- 第一个未完成任务的范围、依赖和验收标准。
- 当前仓库是否已存在未提交改动，需要避免覆盖。

## 执行日志

- 已创建本计划文件，准备开始检查最新提交与任务列表。
- 已检查最新提交 `511f62fe29038e8bb828b253061cc5cb3cbac6f4`（`[T3016j] Queue closure non-resuming blocker before T3017`）。该提交本身是在把新 blocker `T3016j` 前置到 `T3017` 前，没有额外未跟踪修复项；因此本轮首个未完成任务确认为 `T3016j`。
- 已定位 `TODO.md` 中当前第一个未完成任务：
  - `T3016j [TODO] 修正 ordinary closure/function-value callee 中 non-resuming effect 外传后的返回合同`
- 已读取对照 fixture：
  - 失败用例：`tests/fixtures/run-pass/effect_indirect_perform_nonresuming_closure.scoop`
  - 通过对照：`tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop`
- 已复现失败：
  - 命令：`cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_closure.scoop`
  - 结果：`scoop::llvm::unsupported_main_body: return value`
- 已定位报错代码：
  - `crates/scoopc/src/llvm/codegen/mod.rs:11894-11902` 的 `emit_return()` 在返回非 `Unit/Never` 时拿不到 `value.value`。
- 当前诊断摘要：
  - 顶层 ordinary helper 路径在 `codegen_top_level_fun()` 中会建立 `return_context`，因此 non-resuming effect outward propagation 会统一走“写默认返回值 -> branch 到 return_bb -> 统一 return”这条合同。
  - closure 路径 `codegen_closure_fun_body()` 当前只设置了 `current_fun_return_ty`，没有建立 `return_context`。
  - 因此 closure body 内 perform 触发 outward propagation 后，`emit_effect_propagation_return()` / `finish_function_return_path()` 仍可能落回直接 `emit_return()`；一旦尾部落点只剩 dead-path dummy，就会在 `emit_return()` 里报 `return value`。
- 当前修复假设：
  - 让 `codegen_closure_fun_body()` 与 ordinary helper 共享同一套 function-level return contract：为 closure body 建立/恢复 `return_context` 与统一 `return_bb`，而不是在 closure 内直接走裸 `emit_return()`。
  - 修改后需要确认：
    - closure body 的 outward propagation 与 ordinary helper 一致；
    - function-value 调用路径不需要额外按 closure/helper 分流；
    - 不引入按 fixture 名称或 closure 形状的特判。

## 接下来的实现步骤

1. 修改 `codegen_closure_fun_body()`，为 closure body 建立与顶层普通函数一致的 return block / return alloca / return_context 生命周期。
2. 如有必要，抽取或复用统一的 return-path helper，避免 closure 与 ordinary helper 合同继续分叉。
3. 增加 focused regression，优先锁定：
   - closure/function-value callee 在 non-resuming effect outward propagation 后不会再落回直接 value-return；
   - 对照的普通 helper call-chain 继续通过。
4. 运行定向验证与全量质量门槛：
   - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_closure.scoop`
   - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 已完成状态

- 代码实现已完成：
  - 在 `crates/scoopc/src/llvm/codegen/mod.rs` 中新增共享 helper：
    - `setup_function_return_context()`
    - `emit_function_return_block()`
  - `codegen_top_level_fun()` 已切换为复用这两个 helper，避免普通函数 / closure 在 return block 上继续分叉。
  - `codegen_closure_fun_body()` 现已建立 `return_bb` / `return_alloca` / `return_context`，因此 closure body 内的 outward non-resuming effect 不会再落回直接 `emit_return()`。
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs` 中 `emit_effect_propagation_return()` 的注释已同步修正：closure 不再属于“没有 return_bb 的内部函数”特例。
- focused regression 已添加：
  - `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_local.scoop`
  - `tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_local.stdout`
- 定向验证结果：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_closure.scoop`：通过
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop`：通过
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_local.scoop`：通过
- 质量门槛结果：
  - `cargo fmt --check`：通过（中途发现 1 处换行风格差异，已执行 `cargo fmt` 后复验通过）
  - `cargo test --all`：通过
  - `cargo clippy --all-targets -- -D warnings`：通过
- 文档状态已更新：
  - `TODO.md`：已把 `T3016j` 标记为完成，并补充进展与验证记录。
  - `PLAN.md`：已记录本轮完成更新，并把当前 effect 主线下一项推进到 `T3016jR`。
- 下一步动作：
  - 检查最终 diff；
  - 提交本轮变更，提交后停止。
