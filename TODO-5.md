# TODO-5：P5-P6 Call-site resolution、callable identity 与最终收尾

> 索引：[`TODO.md`](./TODO.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 覆盖阶段：P5-P6  
> 包目标：实现 overload call-site 五阶段 resolution，贯通 selected callable identity，并完成 spec / fixtures / docs / regression matrix 收尾。

## P5：Overload call-site resolution 与 callable identity 贯通

### [DONE] P5-T01：实现 Phase A-C：候选收集、visibility、applicability

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §5.1-§5.3、§7.3、§7.4、§8.1、§8.2
- 目标：
  - 建立 call-site overload pipeline 的前三阶段，形成后续 specificity 的输入集合。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/resolve/scopes.rs::{BlockScopeChecker::resolve_call_site,resolve_call_ident_callee,resolve_call_member_callee,resolve_member_access_on_type_receiver,try_resolve_where_bound_member_access}`
  - `crates/scoopc_hir/src/typecheck/expr/call/dispatch.rs::infer_call_expr_type`
  - `crates/scoopc_hir/src/typecheck/expr/call/args.rs::{collect_call_arg_infos,check_call_arg_named_rules,map_call_args_to_params_with_defaults,map_call_args_to_params_with_defaults_and_varargs}`
  - `crates/scoopc_hir/src/typecheck/expr/call/generic.rs::{instantiate_fun_sig_for_call,check_fun_where_constraints_after_instantiation}`
  - `crates/scoopc_hir/src/typecheck/expr/call/member_call.rs::infer_member_call_expr_type`
  - `crates/scoopc_hir/src/typecheck/expr/call/ctor.rs::collect_matched_ctor_overloads_for_owner`
- 必须实现的内容：
  1. Candidate collection order: local -> member -> extension -> top-level -> imported.
  2. Implement “outer scope fully shadows inner/later scopes”: once a layer yields same-name candidates, do not continue to lower-priority layers even if candidates later fail applicability.
  3. Visibility filter before applicability; invisible candidates must not affect specificity.
  4. Applicability checks: arity / named / default / vararg mapping; argument type subtype relation; no implicit widening; function type subtype checks by existing variance/effect rules; tuple/struct variance according to existing type system; `Nothing` as subtype of every type.
  5. For no applicable overload, diagnostics list all same-name candidates and per-candidate rejection reason.
- 必须遵从的约束：
  - Do not perform specificity tie-breaking in Phase A-C except filtering impossible candidates.
  - Do not let invisible candidates suppress visible candidates.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. fixtures for local shadowing top-level/import, visibility before applicability, no applicable overload diagnostics.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Call-site resolution has a deterministic applicable candidate set with actionable no-applicable diagnostics.
- 依赖：P4-T05R
- 完成记录：
  - 改动范围：更新 `resolve/scopes.rs` 的调用点候选收集层级与 invisible member fallback；更新 `typecheck/expr/call/{mod.rs,value_call.rs,dispatch.rs,member_call.rs,ctor.rs}` 的 Phase B/C 过滤与 no-applicable diagnostics；同步相关 overload fixtures。
  - 核心决策：top-level call candidate collection 以同包/root、显式 import、star import 分层，同时合并函数与构造候选，命中可见候选后不继续下沉；跨文件函数签名在 typecheck 入口按调用源过滤 visibility；不可见 member 只保留为兜底诊断，不再压制可见 extension/inherited candidate；no-applicable overload 使用候选签名、位置和基础 applicability rejection reason 输出。
  - 验证结果：`cargo fmt`；`cargo clippy --all-targets -- -D warnings`；`cargo test --all --all-targets`；`python3 tools/run_fixtures.py`。
  - 与 `PLAN.md` / 设计文档对应闭合：闭合 `PLAN.md` P5 Phase A-C 与 `OVERLOAD_RESOLUTION.md` §5.1-§5.3、§7.4、§8.1、§8.2；P5-T02 可基于已过滤的 applicable candidate set 继续实现 specificity。

### [TODO] P5-T01a：修复 Phase A-C review blockers

- 参考：
  - P5-T01 完成记录
  - P5-T01R 初次 review blocker 记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §5.1-§5.3、§7.4、§8.1、§8.2
- 目标：
  - 修复 P5-T01R 审查中发现的 Phase A-C 前置缺口，使 review 能验证候选收集、visibility、applicability 与 no-applicable diagnostics 的边界。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/resolve/scopes.rs::{resolve_call_ident_callee,resolve_call_member_callee,resolve_member_access_on_value_receiver,extension_fun_candidates,try_resolve_where_bound_member_access}`
  - `crates/scoopc_hir/src/typecheck/expr/call/{dispatch.rs,member_call.rs,ctor.rs,value_call.rs,generic.rs,args.rs}`
  - no-applicable / visibility / shadowing / extension fixtures
- 必须实现的内容：
  1. Typecheck 普通调用、constructor 调用和 extension member 调用必须消费 resolver 写入的 `ResolvedCall.candidates`，不得继续只依赖 stale single `resolved` FQN。
  2. Member 层必须整体优先于 extension 层；inherited visible member 不得被同名 visible extension 抢先，invisible direct member 不得压制后续 visible member/extension 诊断路径。
  3. Cross-file/index 函数签名必须保留 `param_is_vararg`，constructor applicability 必须用与函数一致的 arity / named / default / vararg mapping 规则；如果 lowering 侧仍缺 vararg ctor 绑定能力，必须修复该能力而不是绕过。
  4. Where-bound member call 和 late extension lookup 不得按首个候选或 import 排序提前选择；所有候选必须先经过 visibility 与 applicability，再进入后续 selection/ambiguity 路径。
  5. no-applicable diagnostics fixtures 必须断言候选签名和 per-candidate rejection reason；shadowing fixtures 必须覆盖 local shadow top-level/import；visibility fixtures 必须确认不可见候选不会影响可见候选。
- 必须遵从的约束：
  - 不得把 Phase A-C 缺口留给 P5-T02 specificity 或 P5-T04 callable identity 作为 workaround；后续阶段只能建立在清晰 applicable candidate set 上。
  - 不得让不可见候选影响后续选择。
  - 不得通过 fixture 缩窄、改写测试形状或只报 `AmbiguousCall` 来避开 applicability 过滤。
- 验证：
  1. `cargo fmt`
  2. `cargo clippy --all-targets -- -D warnings`
  3. targeted Phase A-C fixtures
  4. `python3 tools/run_fixtures.py`
  5. `cargo test --all --all-targets`
- 完成条件：
  - P5-T01R 能复核并确认 Phase A-C 的候选集合、visibility-before-applicability 和 no-applicable diagnostics，无需再新增前置修复任务。
- 依赖：P5-T01
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T01R：Review Phase A-C resolution

- 参考：
  - P5-T01 完成记录
  - P5-T01a 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §5.1-§5.3
- 目标：
  - 复核候选收集、visibility 和 applicability 的阶段边界。
- 必须检查的文件/位置：
  - P5-T01 修改的 resolve/typecheck call files
  - no-applicable / visibility / shadowing fixtures
- 必须实现的内容：
  1. 确认 scope 层叠顺序和 shadow 语义。
  2. 确认 visibility 在 applicability 前过滤。
  3. 确认 no applicable diagnostics 列出候选和原因。
  4. 确认没有提前做 specificity 或 effect fallback。
- 必须遵从的约束：
  - 不得让不可见候选影响后续选择。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted Phase A-C fixtures。
- 完成条件：
  - P5-T02 可基于清晰 applicable candidate set 实现 specificity。
- 依赖：P5-T01a
- 阻塞记录：
  - 2026-05-29 初次 review 发现 P5-T01 仍有 Phase A-C 前置缺口：`ResolvedCall.candidates` 在部分普通/extension/member typecheck 路径未被消费，member/inherited/extension 层级仍可能违反 member-before-extension，跨文件签名 vararg 元数据和 constructor vararg applicability 不完整，where-bound / late extension 路径仍可能提前按首个候选或 import 顺序选择，no-applicable fixtures 未充分断言候选签名与 rejection reason。已新增 P5-T01a 作为本 review 的前置修复任务。
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T02：实现 Phase D-E specificity 与 ambiguity diagnostics

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §5.4-§6.4、§10
- 目标：
  - 从 applicable candidates 中选出唯一最具体候选；若不可唯一确定，输出高质量歧义错误。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/expr/call/member_call.rs::{is_strictly_more_specific_member_overload,pick_most_specific_member_overload}`
  - `crates/scoopc_hir/src/typecheck/expr/call/ctor.rs::{pick_most_specific_ctor_overload,select_ctor_overload_for_owner}`
  - shared overload selection helper introduced in P5-T01 if created
  - subtype / type comparison helpers used by typecheck
  - diagnostics for `ambiguous_overload`
- 必须实现的内容：
  1. Implement specificity rule: A more specific than B iff for every parameter position `A.eff_i <: B.eff_i` and at least one position is strict.
  2. Include member receiver as parameter position 0 for member method specificity.
  3. Effective type source: concrete param -> concrete type; method-level TP -> declared bound, default `Any`; multiple bounds -> intersection if supported; composite type -> recursively substitute TP bounds.
  4. Do not use inferred substitution for specialization.
  5. Function type specificity must follow function subtype relation.
  6. Ambiguity diagnostic must list all applicable candidates, file/line/col, signature, effective type source, and incomparable positions.
- 必须遵从的约束：
  - Do not choose “first candidate” on incomparable ties.
  - Do not use return type or effect row for specificity.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. fixtures for concrete vs generic, bound chain, incomparable bounds, lambda function-type specificity, cross-incomparable multi-param overload.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Overload selection follows `OVERLOAD_RESOLUTION.md` §6 and ambiguity diagnostics are user-actionable.
- 依赖：P5-T01R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T02R：Review specificity 与 ambiguity diagnostics

- 参考：
  - P5-T02 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §6、§10
- 目标：
  - 复核 specificity 偏序、effective type 来源和 ambiguity diagnostics。
- 必须检查的文件/位置：
  - P5-T02 修改的 overload selection helpers
  - ambiguity fixtures
- 必须实现的内容：
  1. 确认 concrete/generic/bound-chain specificity 正确。
  2. 确认 incomparable bounds 不会被任意选中。
  3. 确认 member receiver 被当作第 0 参数位。
  4. 确认 diagnostics 包含候选位置、effective type 来源和不可比原因。
- 必须遵从的约束：
  - 不得使用 inferred substitution 触发 specialization。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted specificity / ambiguity fixtures。
- 完成条件：
  - P5-T02 行为符合 §6 / §10。
- 依赖：P5-T02
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T03：整合 member / constructor / operator / effect-after-selection 路径

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §7、§8、§9、§11.3
- 目标：
  - 让 member methods、constructors、operators、function/lambda overloads 使用同一 resolution 语义，并确保 effect row 在 selection 后校验。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/expr/call/dispatch.rs::infer_call_expr_type`
  - `crates/scoopc_hir/src/typecheck/expr/call/member_call.rs::infer_member_call_expr_type`
  - `crates/scoopc_hir/src/typecheck/expr/call/ctor.rs::{try_infer_nominal_constructor_call_expr_type_with_expected,select_ctor_overload_for_owner}`
  - `crates/scoopc_hir/src/typecheck/expr/call/value_call.rs::{infer_function_type_call_expr_type,infer_function_value_call_expr_type,infer_top_level_fun_value_expr_type}`
  - `crates/scoopc_hir/src/typecheck/expr/ops.rs::infer_operator_overload_binary_expr_type`
  - effect checking logic used after call selection
- 必须实现的内容：
  1. Member method resolution uses static receiver type for overload selection; virtual dispatch happens only after selected signature is known.
  2. Child overload set includes inherited visible methods, overridden replacements, and child-added overloads.
  3. Constructor calls use same Phase A-E model inside class constructor set.
  4. Operator expressions first desugar/find operator method name, then use same overload resolution after P3-T01 modifier gate.
  5. Lambda/function type overloads use normal subtype specificity; do not add special reject.
  6. Effect row check happens after unique candidate selection and must not cause fallback to a less-specific overload.
- 必须遵从的约束：
  - Do not make overload resolution dynamic based on runtime class.
  - Do not let effect compatibility influence candidate choice.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. fixtures for static resolution + dynamic dispatch, constructor overload, operator overload, lambda overload, effect mismatch after selection.
  3. `cargo test --all --all-targets`
- 完成条件：
  - All callable surfaces use a coherent overload selection model.
- 依赖：P5-T02R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T03R：Review call surface 整合结果

- 参考：
  - P5-T03 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §7、§8、§9
- 目标：
  - 复核 member/constructor/operator/lambda/effect 路径都使用同一 resolution 模型。
- 必须检查的文件/位置：
  - P5-T03 修改的 call dispatch files
  - member / constructor / operator / lambda overload fixtures
  - effect mismatch after selection fixtures
- 必须实现的内容：
  1. 确认 static receiver type 决定 overload signature。
  2. 确认 virtual dispatch 只发生在 selected signature 之后。
  3. 确认 constructor/operator/lambda 路径没有自定义旁路。
  4. 确认 effect row mismatch 不回退到其它 overload。
- 必须遵从的约束：
  - 不得让 runtime class 或 effect compatibility 影响 overload choice。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted call surface fixtures。
- 完成条件：
  - P5-T03 行为符合 §7-§9。
- 依赖：P5-T03
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T04：贯通 selected callable identity，修复 concrete / arity / generic-concrete codegen bug

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§5
- 目标：
  - Typecheck 选出的 overload identity 必须贯穿 HIR/MIR/materialization/codegen；lowering 不再按 bare FQN 混淆同名函数。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/hir/mod.rs::{TopLevelFunCallSiteIndex,EffectOpCallSiteIndex,CallableAbiIdentity}` and call binding structs
  - `crates/scoopc_hir/src/hir/lower/expr/typechecked.rs::{materialized_direct_call_target_fqn_for_binding,typechecked_direct_call_expr,try_lower_typechecked_operator_overload_binary_expr,try_lower_typechecked_compare_to_binary_expr}`
  - `crates/scoopc_hir/src/hir/lower/expr/canonical_call.rs::lower_canonical_call_expr`
  - `crates/scoopc_mir/src/mir/materialize/hir_calls.rs::{collect_hir_direct_call_instance_requests,choose_hir_direct_call_template_for_binding}`
  - `crates/scoopc_mir/src/mir/callables.rs::{MaterializedCallableFamilies,MaterializedCallableView}`
  - `crates/scoopc_mir/src/mir/lower/fn_lowering_call.rs::{lower_typed_call_expr,lower_direct_call_expr,lower_dispatch_call_expr_from_contract}`
  - `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs::{codegen_mir_call,codegen_mir_direct_call_with_policy}`
- 必须实现的内容：
  1. Extend call binding metadata to include selected overload identity, not just callee FQN.
  2. Preserve selected type args, owner, parameter signature, and callable ABI identity through HIR lowering.
  3. MIR materialization must choose template/callable version from binding identity.
  4. LLVM declarations/calls must use the materialized callable version corresponding to the selected overload.
  5. Enable the three P0-T02 run-pass baseline fixtures by removing their `IGNORE-UNTIL-FIX` directives once implementation is ready: `tests/fixtures/run-pass/overload_concrete_bug.scoop`, `tests/fixtures/run-pass/overload_arity_bug.scoop`, and `tests/fixtures/run-pass/overload_gvc_ok.scoop`.
- 必须遵从的约束：
  - Do not fix by changing symbol names only; the selected semantic identity must drive materialization.
  - Do not leave fallback codegen lookup by same-name FQN in overload-sensitive paths.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted run-pass fixtures for concrete, arity, generic+concrete overload bugs.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Known overload bugs no longer fail in codegen/materialization.
- 依赖：P5-T03R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T04R：Review selected callable identity 贯通

- 参考：
  - P5-T04 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§5
- 目标：
  - 复核 selected overload identity 是否真正贯穿 HIR/MIR/materialization/codegen。
- 必须检查的文件/位置：
  - P5-T04 修改的 HIR/MIR/materialization/codegen files
  - concrete / arity / generic-concrete overload bug fixtures
- 必须实现的内容：
  1. 确认 call binding 不只保存 bare FQN。
  2. 确认 MIR materialization 使用 selected overload identity 选择 callable version。
  3. 确认 LLVM declaration/call 不再混淆同名 overload。
  4. 确认三个已知 bug fixture 全部通过。
- 必须遵从的约束：
  - 不得接受仅靠 symbol spelling 避免冲突但 semantic identity 仍缺失的实现。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted overload run-pass fixtures。
  3. `cargo test --all --all-targets`
- 完成条件：
  - P5-T04 修复完整且有 regression 保护。
- 依赖：P5-T04
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T05：审计 overload diagnostics 与 user-visible failure policy

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P5
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §10
  - `tools/audit_user_visible_failure_policy.py::FRONTEND_REJECT_FORBIDDEN_TERMS`
- 目标：
  - overload 相关错误全部可定位、可解释、无 backend/internal term 泄漏。
- 必须检查的文件/位置：
  - diagnostics emitted from `crates/scoopc_hir/src/typecheck/overloads.rs`
  - diagnostics emitted from `crates/scoopc_hir/src/typecheck/expr/call/**`
  - diagnostics emitted from `crates/scoopc_hir/src/typecheck/expr/ops.rs`
  - `tests/fixtures/typecheck/**/**/*overload*.scoop`
  - `tools/audit_user_visible_failure_policy.py`
- 必须实现的内容：
  1. Ensure `ambiguous_overload`, `no_applicable_overload`, `conflicting_overloads`, `generic_overload_shape_mismatch`, `vararg_overlaps_non_vararg` all list candidate file/line/col.
  2. For ambiguity, include reason section describing incomparable effective parameter types and source of generic bounds.
  3. For no-applicable, include per-candidate arity/type/visibility rejection reason.
  4. Add fixture assertions that diagnostics do not contain forbidden internal terms.
  5. If audit script needs overload-specific coverage, extend it rather than relying on manual review.
- 必须遵从的约束：
  - Do not hide candidate details to make tests easier.
  - Do not introduce backend/codegen errors for frontend-resolvable overload failures.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. `python3 tools/audit_user_visible_failure_policy.py` if script is executable for this repo state.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Overload diagnostics satisfy design requirements and failure policy.
- 依赖：P5-T04R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P5-T05R：Review overload diagnostics 审计

- 参考：
  - P5-T05 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §10
- 目标：
  - 复核 overload diagnostics 已满足 user-visible failure policy。
- 必须检查的文件/位置：
  - overload diagnostics code paths
  - overload typecheck fixtures
  - `tools/audit_user_visible_failure_policy.py`
- 必须实现的内容：
  1. 确认所有 overload errors 列候选位置。
  2. 确认 ambiguity/no-applicable reasons 足够具体。
  3. 确认 forbidden internal terms 没有出现在用户可见错误。
  4. 确认 frontend-resolvable overload failures 不会落到 backend/codegen。
- 必须遵从的约束：
  - 不得用弱化错误测试覆盖真实诊断缺口。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. `python3 tools/audit_user_visible_failure_policy.py` if available。
- 完成条件：
  - P5 包完整闭合，可以进入 spec/fixture 收尾。
- 依赖：P5-T05
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

## P6：Spec / fixtures / docs 全量收尾与回归矩阵

### [TODO] P6-T01：回写 `SCOOP_FULL_SPEC.md` 与 split spec 的全部语言变更

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - [`SPEC_FIX.md`](./SPEC_FIX.md) summary table
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §12
- 目标：
  - 活跃 spec 与 P1-P5 实现后的语言行为一致。
- 必须修改的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
- 必须实现的内容：
  1. Update all relevant spec sections: `Nothing` bottom type; tuple `.0` / `.1`; refutable `val` panic fallback; `!!` / `as` panic and `as?` unchanged; effect op plain qualified call; handler `on`; closure `var` capture forbidden; f-string `${...}` and literal braces; `ref` / `value` bound keywords; default `internal` visibility; `operator` modifier required; overload definition-time and call-site rules; delete `@Inline` old prose.
  2. Ensure examples compile under new syntax or are explicitly marked as negative examples.
  3. If split spec has a generator, use it and record command; otherwise hand-sync edited sections.
- 必须遵从的约束：
  - Do not leave contradictory old examples in spec.
  - Do not document unimplemented compatibility aliases.
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. Manual review of changed spec sections against `SPEC_FIX.md` and `OVERLOAD_RESOLUTION.md`.
- 完成条件：
  - Active spec no longer describes old language surface as valid.
- 依赖：P5-T05R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P6-T01R：Review spec 回写完整性

- 参考：
  - P6-T01 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) summary table
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §12
- 目标：
  - 复核 active spec 是否完整反映 P1-P5 目标行为。
- 必须检查的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
- 必须实现的内容：
  1. 对照 `SPEC_FIX.md` 13 项逐条确认 spec 已更新。
  2. 对照 `OVERLOAD_RESOLUTION.md` §12 确认 overload rules 已写入。
  3. 确认旧 surface 不再作为正例出现。
  4. 确认 spec examples 与实现语法一致。
- 必须遵从的约束：
  - 不得留下 split spec 与 full spec 矛盾。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
- 完成条件：
  - P6-T01 spec 回写完整且一致。
- 依赖：P6-T01
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P6-T02：同步 spec doctests 与 handwritten fixtures 到新 surface

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - `tools/spec_fixtures.py`
  - `tools/run_fixtures.py`
- 目标：
  - 所有 active fixtures 使用目标语法与目标 semantics；旧语法只保留为明确 negative fixture。
- 必须修改的文件/位置：
  - `tests/fixtures/spec_doctest/**`
  - `tests/fixtures/parse/**`
  - `tests/fixtures/typecheck/**`
  - `tests/fixtures/infer/**`
  - `tests/fixtures/hir/**`
  - `tests/fixtures/mir/**`
  - `tests/fixtures/effect_facts/**`
  - `tests/fixtures/effect_lowered/**`
  - `tests/fixtures/run-pass/**`
  - `tests/fixtures/build/**`
- 必须实现的内容：
  1. Run `python3 tools/spec_fixtures.py sync` after P6-T01 spec edits.
  2. Mechanically update handwritten fixtures: `perform` -> plain effect op call; handler `with` -> `on`; `._0` -> `.0`; f-string `{expr}` -> `${expr}`; `AnyRef` / `AnyValue` -> `ref` / `value`; add explicit `public` where fixture models exported API; add `operator` modifier where operator syntax is intended.
  3. Keep or add negative fixtures for old syntax with file names and expected diagnostics clearly indicating rejection.
  4. Refresh expected dumps for HIR/MIR/effect facts only where textual changes are semantically expected.
- 必须遵从的约束：
  - Do not delete coverage just because syntax changed.
  - Do not update expected dumps blindly; confirm semantic reason for each churn.
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. `python3 tools/run_fixtures.py`
  3. targeted run for every fixture path changed in this task.
- 完成条件：
  - Fixture suite is synchronized with new language surface.
- 依赖：P6-T01R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P6-T02R：Review fixture 同步结果

- 参考：
  - P6-T02 完成记录
  - P0-T01 inventory
- 目标：
  - 复核 generated doctests 和 handwritten fixtures 已与新 surface 同步。
- 必须检查的文件/位置：
  - `tests/fixtures/spec_doctest/**`
  - all fixture paths changed by P6-T02
  - `tools/spec_fixtures.py`
  - `tools/run_fixtures.py`
- 必须实现的内容：
  1. 确认 `spec_fixtures.py sync` 后无 stale generated fixture。
  2. 抽样复核每类语法迁移。
  3. 确认 negative fixtures 明确表达 old surface rejection。
  4. 确认 dump expect churn 有语义理由。
- 必须遵从的约束：
  - 不得通过删除 fixture coverage 通过 review。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. `python3 tools/run_fixtures.py`
- 完成条件：
  - Fixture suite 与 spec/compiler surface 一致。
- 依赖：P6-T02
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P6-T03：执行旧 surface 与 overload/codegen 回归审计

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6
  - P0-T01 完成记录中的旧 surface inventory
- 目标：
  - 确认旧 surface 已从 active positive path 中清除，overload/codegen bug 已有回归保护。
- 必须检查的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - `sysroot/**`
  - `tests/fixtures/**`
  - `crates/scoopc_ast/src/**`
  - `crates/scoopc_hir/src/**`
  - `crates/scoopc_mir/src/**`
  - `crates/scoopc_codegen_llvm/src/**`
- 必须实现的内容：
  1. Audit old surface occurrences and classify any remaining hit as archive/history/design doc、negative fixture、diagnostic text explaining removal、or active bug requiring fix before P6 completion.
  2. Confirm no active positive fixture uses old `perform`, handler `with`, tuple `._0`, old f-string interpolation, `@Inline`, `AnyRef` / `AnyValue`.
  3. Confirm overload bug fixtures exist and pass for concrete overload, arity overload, generic+concrete specificity.
  4. Confirm `.cone` API export tests or manual sample show only explicit public declarations in `api.scoopir`.
  5. Confirm diagnostics for overload errors still include candidate locations and no forbidden internal terms.
- 必须遵从的约束：
  - Do not treat design docs `SPEC_FIX.md` / `OVERLOAD_RESOLUTION.md` as active old-surface violations.
  - Do not accept active compiler code paths that support old positive syntax unless design docs were updated first.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. `python3 tools/spec_fixtures.py check`
  3. targeted overload run-pass/typecheck fixtures.
  4. targeted cone export test or documented manual command in completion record.
- 完成条件：
  - Audit has no unexplained active old-surface hits.
- 依赖：P6-T02R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P6-T03R：Review 旧 surface 与回归审计

- 参考：
  - P6-T03 完成记录
  - P0-T01 inventory
- 目标：
  - 复核旧 surface audit 和 overload/codegen regression audit 的可信度。
- 必须检查的文件/位置：
  - P6-T03 audit 命中清单
  - old-surface negative fixtures
  - overload bug regression fixtures
  - cone export sample/test
- 必须实现的内容：
  1. 抽样复查 P6-T03 的 old surface classification。
  2. 确认 remaining hits 都是允许类别。
  3. 确认 overload regression 不只存在文件，还实际运行通过。
  4. 确认 cone export audit 可复现。
- 必须遵从的约束：
  - 不得把 active support for old syntax 标记为“历史命中”。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. `python3 tools/spec_fixtures.py check`
  3. targeted overload/cone tests。
- 完成条件：
  - P6-T03 audit 可以作为最终收口依据。
- 依赖：P6-T03
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P6-T04：全量格式化、测试矩阵与最终收口记录

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P6、§6
  - [`SPEC_FIX.md`](./SPEC_FIX.md) summary table
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §12
- 目标：
  - 关闭本轮执行计划，确保 spec、compiler、sysroot、fixtures、diagnostics、codegen 全部一致。
- 必须实现的内容：
  1. Run formatting and full test matrix.
  2. Record full verification commands and results in this task completion record.
  3. If any remaining issue is intentionally deferred, it must be outside `SPEC_FIX.md` / `OVERLOAD_RESOLUTION.md` scope and must be written as explicit v2+ backlog with rationale.
  4. Confirm `TODO.md` statuses for completed tasks are updated consistently if the team uses `[DONE]` markers.
- 必须遵从的约束：
  - P6-T04 cannot pass with unimplemented items from `SPEC_FIX.md` or `OVERLOAD_RESOLUTION.md` silently deferred.
  - Do not use partial targeted test pass as replacement for full regression unless environment lacks required dependency; if so, record blocker precisely.
- 验证：
  1. `cargo fmt`
  2. `cargo test --all --all-targets`
  3. `python3 tools/spec_fixtures.py check`
  4. `python3 tools/run_fixtures.py`
  5. LLVM/backend targeted tests required by changed code paths if not already covered by full suite.
- 完成条件：
  - `SPEC_FIX.md` and `OVERLOAD_RESOLUTION.md` target behavior is the active contract in spec and compiler.
  - Old surface only remains in archive/history/design baseline or explicit negative fixtures.
- 依赖：P6-T03R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P6-T04R：Review 最终收口质量

- 参考：
  - P6-T04 完成记录
  - [`PLAN.md`](./PLAN.md) §6
  - [`SPEC_FIX.md`](./SPEC_FIX.md) summary table
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §12
- 目标：
  - 最终复核本轮计划是否完整闭合，防止未完成项被静默延期。
- 必须检查的文件/位置：
  - `PLAN.md`
  - `TODO.md`
  - `TODO-1.md` 到 `TODO-5.md`
  - `SCOOP_FULL_SPEC.md`
  - `sysroot/**`
  - compiler paths changed by P1-P6
  - fixture changes from P1-P6
- 必须实现的内容：
  1. 对照 `SPEC_FIX.md` summary table 确认每项都有实现、spec 更新或明确回写决议。
  2. 对照 `OVERLOAD_RESOLUTION.md` §12 确认 overload rules 和 diagnostics 已落地。
  3. 复核 full test matrix 结果是否真实、完整、可复现。
  4. 确认所有 TODO package 状态和完成记录一致。
  5. 若发现阻塞问题，直接修复或把任务退回 `[TODO]`，不得标记全包完成。
- 必须遵从的约束：
  - Review 不是签字；必须能指出具体 evidence。
- 验证：
  1. `cargo fmt`
  2. `cargo test --all --all-targets`
  3. `python3 tools/spec_fixtures.py check`
  4. `python3 tools/run_fixtures.py`
- 完成条件：
  - 本轮 Spec Fix + Overload Resolution 执行计划完成，剩余 backlog 只包含明确超出本轮范围的 v2+ 项。
- 依赖：P6-T04
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：
