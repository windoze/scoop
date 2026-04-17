# 执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在开始该任务前发现最新提交中提到的既有问题，先修复这些问题，再继续。

## 约束

- 先检查最新提交，看是否提到需要先处理的既有问题。
- 再读取 `TODO.md`，定位第一个未完成任务。
- 如果该任务过大，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
- 实现后必须运行相关测试，并尽量补齐必要测试。
- 需要同步更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`。
- 完成后提交 git commit，然后停止，不继续下一个任务。
- 若遇到规范不匹配、缺失特性或阻塞项，不做规避实现；应先把前置修复加入 `TODO.md`、调整顺序、更新 `PLAN.md`，提交后停止。

## 初始步骤

1. 检查最新提交的提交信息与变更说明，确认是否点名了待修复问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，确定当前优先级和第一个未完成任务。
3. 评估任务规模与依赖：
   - 若可直接完成，则进入实现。
   - 若过大或被前置问题阻塞，则拆分/重排任务，并先处理新的首个子任务或前置修复项。
4. 实施代码修改，保持实现符合规范，不引入临时绕过方案。
5. 运行格式化、相关测试与必要的静态检查；至少覆盖本次修改影响范围。
6. 更新文档与计划文件，标记本轮完成情况。
7. 提交变更并停止。

## 进度记录

- 已创建本计划文件，后续在关键检查点后追加更新。
- 已完成初始排查：
  - 最新提交为 `fdd7728 [T3014c] Restore delegated observable effect-instance key`。
  - 最新提交信息未额外点名一个独立于 `TODO.md` 的新遗留修复项。
  - `TODO.md` 的首个未完成任务是 `T3014cR`（review 任务）。
- 当前执行中的 review 范围：
  1. 审查 `crates/scoopc/src/typecheck/expr/entry.rs` 中 delegated-property inline 表达式检查，确认只覆盖 lowering 实际会 inline 的标准 delegates（`lazy` / `observable` / `vetoable`），没有把整个 delegate 调用误接回普通 call typecheck 路径。
  2. 审查 `crates/scoopc/src/hir/lower/sugar.rs` 的 observable/vetoable lowering，确认 callback body 中的 `Raise.raise(...)` 仍走普通 HIR 表达式 lowering，`Perform.effect_ty` 来源仍是统一的 typecheck side table。
  3. 审查 `crates/scoopc/src/llvm/codegen/effect/mod.rs`、`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 与 `crates/scoopc/src/llvm/codegen/mod.rs`，确认 ordinary `perform`、state-machine `emit_perform_op` 和 dispatch matching 都共用 `effect_instance_key()` / `matching_effect_instance_keys_for_handled_effect()` 合同，无 delegated-property callback 专用 fallback。
- 进行中的验证：
  - `cargo test -p scoopc lower_typed_single_source_file_preserves_effect_ty_in_observable_delegate_callback -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`
- review 新发现的真实问题：
  - 标准 delegated-property side table 当前只保存声明点 AST（callback body / initializer body / type ref），但未绑定声明点 `SourceFile` / `ast::File` 身份。
  - 跨文件使用 delegated property 时，`lower_observable_delegated_property_assign()` / `lower_lazy_delegated_property_get_from_receiver()` 等路径会在“使用点文件”的 lowering 上下文里直接 lower“声明点文件”的 AST，导致：
    1. `file.inferred_performed_effect_tys` / `inferred_expr_tys` / `inferred_binding_tys` 可能查错文件；
    2. `self.source.slice(span)` 可能从错误源文件切片标识符文本；
    3. `SymbolInterner` 当前只按本地 `Span` 给 local 分配 `SymbolId`，跨文件 inline AST 时可能与使用点文件同 offset 的 local 发生冲突。
- 已调整计划：
  1. 先修复 delegated-property side table 的“声明点上下文”传播与 foreign-AST lowering。
  2. 同时把 local `SymbolId` 的 interning 从“仅看 span”升级为“源文件 + span”，避免跨文件 inline AST 冲突。
  3. 增加一条多文件 typed-lowering 回归，锁定“在另一文件里触发 observable callback 的 `Raise.raise(...)` 仍保留真实 `effect_ty`”。
- 已完成实现：
  - 为标准 delegated-property info（`lazy` / `observable` / `vetoable`）补齐声明点 `SourceFile` / `ast::File` 上下文。
  - `lower_lazy_delegated_property_get_from_receiver()`、`lower_observable_vetoable_delegated_property_get_from_receiver()`、`lower_observable_delegated_property_assign()`、`lower_vetoable_delegated_property_assign()` 现在会在 lower 声明点 AST（type ref / callback body / initializer body）时显式切回声明点上下文。
  - HIR lowering 的 local symbol interning 现已从“仅按 span”升级为“源文件 + span”，避免跨文件 inline AST 的 local `SymbolId` 冲突。
  - 已新增多文件回归 `lower_for_compilation_unit_multi_files_preserves_effect_ty_in_cross_file_observable_delegate_callback`。
- 已完成验证：
  - `cargo test -p scoopc lower_for_compilation_unit_multi_files_preserves_effect_ty_in_cross_file_observable_delegate_callback -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/delegated_property_observable_raise_does_not_poison_mutex.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop --features llvm -- test`
- 验证结论：
  - 新增多文件低层回归通过，证明跨文件 observable callback 内 `Raise.raise(...)` 仍会保留 `Perform.effect_ty = Raise<Int>`。
  - 既有 runtime fixture `delegated_property_observable_raise_does_not_poison_mutex.scoop` 通过，说明“raise 被捕获后属性继续可用”语义未回退。
  - 全量 `scoop --features llvm -- test` 未回退到 delegated-property observable fixture；当前首个停止点仍是已跟踪的 stale `EXPECT: fail` `effect_escape_continuation_indirect_perform_closure_tail_return_string.scoop`（`T3017`）。
- 收尾状态：
  - 已更新 `TODO.md` / `PLAN.md`，将 `T3014cR` 标记完成，并记录本轮发现与修复。
  - 当前只剩 git commit。提交后按要求停止。本轮下一个未完成任务将是 `T3014R`。
