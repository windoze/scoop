## 本轮执行计划（P4-T01h）

按照 `PROMPT.md` 规范完成 `TODO.md` 中第一个未完成任务 **P4-T01h**：完整支持构造器 type argument（LHS expected type 反推 + 显式 type argument 调用）。

### 任务确认

- `TODO.md` 中 `P4-T01g` 已 `[DONE]`；下一条是 `P4-T01h`，与 `P4-T01g` 之间无强依赖；`P4-T01i` 是独立的 fixture / 单测清理任务，不阻塞本任务。
- 最近一次提交是 `[P4-T01g] Resolve inherited members via supertype walk`，无遗留未完成事项需要并入。

### 实现方案要点

两类构造器 type argument 路径需要补齐：

1. **LHS expected type 反推**：`val c: Container<Int> = Container()` / `val b: Box<Int> = Box(7)`（`T` 不在 ctor arg 暴露）必须从 LHS 注解的 nominal type argument 反推 `T`，而非 silent-fallback 到 `Any`。
2. **构造器位置显式 type argument**：`Container<Int>()` 必须接入与顶层 generic fun `empty<Int>()` 相同的 typed call-site contract，不再触发 `missing typed call-site contract`。

### 顺序

1. 复现两类失败：用最小 scoop 代码确认 `Container<Int>()` 与 `val c: Container<Int> = Container()` 的当前行为。
2. 阅读 typecheck call-site 主路径：
   - `crates/scoopc/src/typecheck/expr/call.rs`：ctor call 形态、type-arg solver 入口、`combined_member_instance_type_args`。
   - `crates/scoopc/src/typecheck/expr/entry.rs`：initializer expected-type 下传。
   - `crates/scoopc/src/hir/lower/expr.rs`：call lowering / typed call-site contract 注册。
   - `crates/scoopc/src/typecheck/lower.rs`：`lower_type_fqn_with_args` 之类的 nominal lowering。
3. 实现：
   - typecheck 在 ctor call solver 启动时把 LHS expected nominal type 的 type-args 作为 candidate；
   - 显式 ctor type args 的 parsing 若 parser 已支持就仅补 typecheck / lowering，否则需补 parser；
   - 解出的 `T` 同步到 monomorphization 主线，不引入新 erased path。
4. 加 fixture：
   - run-pass：zero-arg ctor + LHS 反推；ctor 有非-T arg + LHS 反推；显式 ctor type args；多 generic + 显式 + LHS 一致。
   - typecheck-error：显式 ctor type args 与 LHS 不一致 → 现有 mismatch 错误。
5. 跑 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`tests/fixtures/typecheck`、`cargo clippy --all-targets -- -D warnings`、定向 `cargo test -p scoopc`。
6. 写 `[DONE] P4-T01h` 完成记录并提交。

### 风险点

- 显式 ctor type args 现状是 `frontend_prepare_failed`，可能 parser 已经能 parse 但 typecheck/lowering 未补完整；具体路径需阅读后判断。
- LHS 反推可能涉及 expected-type 通道在 ctor call 路径的注入，已有 `combined_member_instance_type_args` / `find_member_owner_nominal_instantiation` 可复用。
- 解算结果若与 ctor arg 反推冲突需明确优先级（仿照顶层 generic fun：显式 > 反推 > arg 推导）。
- `P4-T01i` 中已知的 9 个 `cargo test -p scoopc` 失败 + 3 个 fixture 失败仍是 baseline 噪声，本任务不修；只需确认本任务不引入新增失败。

### 进展更新

- **typecheck 改动**：
  - `CtorParamInstantiationRequest` 新增 `explicit_type_args` / `expected_owner_args` 两个可选字段，在 `instantiate_ctor_param_tys` 中按 **显式 > arg-driven > LHS expected > `Any`** 合并，冲突即返回 `Ok(None)` 让调用点退化到 `NoMatchingOverload`；
  - `collect_matched_ctor_overloads_for_owner` / `infer_nominal_constructor_call_expr_type` 接受同样的两个可选参数；`select_ctor_overload_for_owner`、super-ctor `check_ctor_call_args_by_arity` 路径仍传 `None`；
  - 新增 `try_infer_nominal_constructor_call_expr_type_with_expected`：在 expected-context 命中 `Call` 形态、callee 透明展开 `TypeApply` 后是 unresolved `Ident` 且 expected 为 nominal generic instantiation 时，把 expected owner-args 喂给 ctor solver；
  - `infer_call_expr_type` dispatch 现把已经提取的 `explicit_type_args` 也传给 `infer_nominal_constructor_call_expr_type`；
  - `infer_expr_type_in_expected_context` 中加 `Call` 形态分支，先调上面的 `try_*_with_expected`，命中失败回到既有 dispatch。
- **HIR 改动**：在 ctor binding 识别处与 `try_lower_struct_ctor_call_expr` 入口透明展开 `TypeApply`；ctor binding 仍键于 `Call` 整体 span，因此不需要新 IR path 即可让 `Container<Int>(...)` 的 ctor binding 重新被回写。
- **新增 4 个 run-pass fixture + 2 个 typecheck-fail fixture**：
  - `tests/fixtures/run-pass/ctor_type_arg_lhs_zero_arg_ctor_basic.scoop`
  - `tests/fixtures/run-pass/ctor_type_arg_lhs_non_t_ctor_arg_basic.scoop`
  - `tests/fixtures/run-pass/ctor_type_arg_explicit_basic.scoop`
  - `tests/fixtures/run-pass/ctor_type_arg_explicit_with_lhs_consistent_basic.scoop`
  - `tests/fixtures/typecheck/ctor_type_arg_explicit_conflicts_with_lhs_is_error.scoop`
  - `tests/fixtures/typecheck/ctor_type_arg_explicit_conflicts_with_arg_is_error.scoop`
- **回归确认**：
  - 6 个新增 fixture 全部通过；
  - `intrinsic_generic_class_body_method_basic.scoop` / `smart_cast_any_member_access_generic_class_basic.scoop` / `generic_class_method.scoop` / `member_call_generic_class_body_method_basic.scoop` 既有 generic class fixture 全部通过，靠 ctor arg 反推 T 的既有路径未受冲击；
  - P4-T01g 锁定的 5 个 inherited fixture 全部通过；
  - 全量 `tests/fixtures/run-pass`：394 passed / 2 failed（仅 P4-T01i 范畴）；
  - 全量 `tests/fixtures/typecheck`：434 passed / 1 failed（仅 P4-T01i 范畴）；
  - `cargo clippy --all-targets -- -D warnings`：通过（新增字段后 `collect_matched_ctor_overloads_for_owner` 触发 `too_many_arguments`，按既有惯例加 `#[allow]`）；
  - `cargo test -p scoopc`：仍是 9 个 pre-existing P4-T01i 失败，未引入新增失败。

### 完成状态

- 已完成：实现、回归、`[DONE]` 完成记录、`memory/claude_plan.md` 刷新；
- 待提交：`crates/scoopc/src/typecheck/expr/{call,infer}.rs`、`crates/scoopc/src/hir/lower/expr.rs`、`tests/fixtures/run-pass/ctor_type_arg_*.scoop`、`tests/fixtures/typecheck/ctor_type_arg_*.scoop`、`TODO.md`、`memory/claude_plan.md`。
