# TODO-4：P4 Overload definition-time 规则落地

> 索引：[`TODO.md`](./TODO.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 覆盖阶段：P4  
> 包目标：在声明处理阶段拒绝“无论怎么调用都不可能合法”的 overload set，给 P5 call-site resolution 一个干净输入。

## P4：Overload definition-time 规则落地

### [TODO] P4-T01：实现 overload effective signature 与 signature equivalence helper

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §3、§4.1、§6.4、§9.1
- 目标：
  - 建立 definition-time overload checks 共用的 signature/effective-type helper。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/overloads.rs::{check_file_overload_conflicts,collect_fun_decl,collect_ctor_decl,check_fun_overload_set,check_ctor_overload_set}`
  - `crates/scoopc_hir/src/resolve/mod.rs::{FunSig,FunOverload,ConstructorOverload,ParamSig}`
  - type alias expansion / type equality helpers in type store or type lowering modules
  - generic bound metadata from `crates/scoopc_hir/src/typecheck/type_env.rs`
- 必须实现的内容：
  1. Define internal representation for overload parameter effective type: concrete parameter -> itself; method-level TP -> declared bound, default `Any`; composite type containing TP -> recursively substitute declared bound; `ref` / `value` bound constraints from P3-T06 must be representable or rejected if they cannot appear as callable param effective type.
  2. Implement signature equivalence: arity + effective parameter types, with transparent type alias expansion.
  3. Ensure return type and effect row are excluded from signature equivalence.
  4. Detect TP alpha-equivalent signatures as conflicts.
  5. Add diagnostics listing both conflicting declarations with file/line/col and rendered signature.
- 必须遵从的约束：
  - Do not use pretty `TypeStore::display()` text as the only equality mechanism.
  - Do not include return type/effect row in conflict checks.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. typecheck fixtures for return-only conflict, effect-only conflict, TP alpha-equivalent conflict, `<T>` vs `<T: Any>` conflict.
  3. `cargo test --all --all-targets`
- 完成条件：
  - All later overload definition-time checks can reuse a single effective signature model.
- 依赖：P3-T07R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T01R：Review effective signature helper

- 参考：
  - P4-T01 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §4.1、§6.4、§9.1
- 目标：
  - 复核 effective signature helper 是否可作为 P4 后续规则的单一基础。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/typecheck/overloads.rs`
  - `crates/scoopc_hir/src/resolve/mod.rs`
  - type alias / type equality helper callsites touched by P4-T01
  - conflict fixtures
- 必须实现的内容：
  1. 确认 signature 不含 return type/effect row。
  2. 确认 TP alpha-equivalence 和 `<T>` vs `<T: Any>` conflict 被覆盖。
  3. 确认 helper 不依赖 pretty text 做唯一相等判断。
  4. 确认 diagnostics 有双方位置和 signature。
- 必须遵从的约束：
  - 不得接受多个并行 signature equivalence 实现。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted overload conflict fixtures。
- 完成条件：
  - P4-T02/P4-T03/P4-T05 可复用该 helper。
- 依赖：P4-T01
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T02：实现 generic overload shape 规则

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §4.2、§6.4
- 目标：
  - 允许“同 shape、仅 bound 不同”的 generic overload；拒绝当前不支持的 generic overload 形态。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/overloads.rs::{check_fun_overload_set,check_ctor_overload_set}`
  - helper from P4-T01 for effective type / shape comparison
  - `crates/scoopc_hir/src/typecheck/expr/call/generic.rs` only if shared bound-shape utilities belong there
  - diagnostics definitions for `generic_overload_shape_mismatch`
- 必须实现的内容：
  1. Allow legal cases: `fun debugPrint<T>(x: T)` with `fun debugPrint<T: Debug>(x: T)`; `fun h<T>(x: T)` with `fun h(x: Int)`; bound chain such as `<T: Animal>` vs `<T: Dog>` when shape matches.
  2. Reject unsupported shape mismatch: `fun f<T>(x: T, y: T)` vs `fun f<T, U>(x: T, y: U)`; TP appearing in different nested positions when not purely differ-by-bound; TP count differences that create consistency constraints.
  3. Do not reject incomparable same-shape bounds at definition time; leave `Comparable` vs `Numeric` ambiguity to call site.
  4. Diagnostic `scoop::typecheck::generic_overload_shape_mismatch` must include both candidates and hint to rename or restructure.
- 必须遵从的约束：
  - Do not implement inferred-substitution specialization.
  - Do not reject bound-incomparable same-shape overloads at definition time.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. typecheck fixtures for legal differ-by-bound, legal concrete+generic, rejected TP consistency constraint, legal incomparable bounds.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Generic overload sets entering P5 match `OVERLOAD_RESOLUTION.md` §4.2.
- 依赖：P4-T01R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T02R：Review generic overload shape 规则

- 参考：
  - P4-T02 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §4.2、§6.4
- 目标：
  - 复核 generic overload shape 判断准确，未过度 reject 或放行 unsupported 形态。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/typecheck/overloads.rs`
  - generic overload fixtures
  - diagnostics definitions touched by P4-T02
- 必须实现的内容：
  1. 确认 differ-by-bound 合法。
  2. 确认 concrete + generic same-shape 合法。
  3. 确认 TP consistency constraint 被 reject。
  4. 确认 incomparable same-shape bounds 定义点通过。
  5. 确认错误码和 hint 符合设计。
- 必须遵从的约束：
  - 不得把 call-site ambiguity 提前错误地作为 definition-time reject。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted generic overload fixtures。
- 完成条件：
  - P4-T02 行为符合 §4.2。
- 依赖：P4-T02
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T03：实现 vararg 与非 vararg overlap 的定义点 reject

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §4.3、§8.2
- 目标：
  - 同名 vararg 与非 vararg 候选只要存在可共同适用的 arity/type overlap，就在定义点拒绝。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/headers.rs::check_vararg_params`
  - `crates/scoopc_hir/src/typecheck/overloads.rs::{check_fun_overload_set,check_ctor_overload_set}`
  - `crates/scoopc_hir/src/typecheck/expr/call/args.rs::{map_call_args_to_params_with_defaults,map_call_args_to_params_with_defaults_and_varargs}` for compatibility with call-site mapper assumptions
  - diagnostics for `vararg_overlaps_non_vararg`
- 必须实现的内容：
  1. For each same-name overload set, compare vararg candidates with non-vararg candidates.
  2. Reject if non-vararg arity is in vararg acceptable range and corresponding fixed/vararg element types are compatible by subtype/effective type check.
  3. Include candidate locations and signatures in diagnostic.
  4. Add fixtures for rejected `fun a(x: Int)` + `fun a(xs: Int*)`, and legal non-overlap `fun b()` + `fun b(x: Int, ys: Int*)`.
- 必须遵从的约束：
  - Do not defer vararg overlap ambiguity to call site.
  - Do not use spread operator as disambiguation escape hatch.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted vararg overload fixtures.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Call-site resolution never sees vararg/non-vararg arity overlap ambiguity.
- 依赖：P4-T02R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T03R：Review vararg overlap reject

- 参考：
  - P4-T03 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §4.3、§8.2
- 目标：
  - 复核 vararg overlap reject 的覆盖和诊断质量。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/typecheck/overloads.rs`
  - `crates/scoopc_hir/src/typecheck/headers.rs`
  - vararg overload fixtures
- 必须实现的内容：
  1. 确认 overlap cases 在定义点 reject。
  2. 确认 non-overlap legal cases 通过。
  3. 确认 diagnostics 包含候选位置。
  4. 确认 spread operator 没有成为规避定义点 reject 的旁路。
- 必须遵从的约束：
  - 不得把 overlap reject 推迟到 P5 call-site。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted vararg fixtures。
- 完成条件：
  - P4-T03 行为符合 §4.3 / §8.2。
- 依赖：P4-T03
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T04：实现 override / overload 边界与虚方法 generic 禁止

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §4.4、§4.5、§7.1、§9.3
- 目标：
  - 明确 method override 与 overload 的定义点边界，并禁止 virtual method 引入方法级 TP。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/inheritance.rs::{check_file_inheritance,check_fun_override,check_property_override}`
  - `crates/scoopc_hir/src/typecheck/interfaces.rs::{check_file_interfaces,check_one_interface_impl,member_fun_match}`
  - `crates/scoopc_hir/src/typecheck/override_effects.rs::check_file_override_effects`
  - `crates/scoopc_hir/src/typecheck/overloads.rs` if method overload set checks live there
  - `crates/scoopc_hir/src/resolve/mod.rs::ModifierSet`
  - diagnostics for `override_non_open_method`, `missing_override`, `override_target_not_found`, `virtual_method_cannot_be_generic`
- 必须实现的内容：
  1. Parent non-open + child same signature -> reject, regardless of child `override` spelling.
  2. Parent open + child same signature without `override` -> reject.
  3. Child `override` with no matching parent signature -> reject.
  4. Child same name but different signature -> legal new overload.
  5. Reject method-level type params on `open fun`, `abstract fun`, `override fun`, and interface body methods.
  6. Allow class-level/interface-level TP in virtual method signatures.
  7. Ensure effect row is not used to find override target; effect variance/compat stays in `override_effects` after target match.
- 必须遵从的约束：
  - Do not treat same-name/different-signature child method as overriding non-open parent.
  - Do not allow virtual generic method through vtable/interface path.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. typecheck fixtures for four override boundary cases and virtual generic rejection.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Method overload/override sets satisfy `OVERLOAD_RESOLUTION.md` §4.4-§4.5.
- 依赖：P4-T03R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T04R：Review override / overload 边界

- 参考：
  - P4-T04 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §4.4、§4.5
- 目标：
  - 复核 override boundary 和 virtual generic reject 是否完整。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/typecheck/inheritance.rs`
  - `crates/scoopc_hir/src/typecheck/interfaces.rs`
  - `crates/scoopc_hir/src/typecheck/override_effects.rs`
  - override/generic method fixtures
- 必须实现的内容：
  1. 确认四类 override boundary 行为覆盖。
  2. 确认 method-level TP 在 virtual method positions 被 reject。
  3. 确认 class-level/interface-level TP 仍可用于 virtual method signature。
  4. 确认 effect row 不参与 override target lookup。
- 必须遵从的约束：
  - 不得让 virtual generic method 混入 vtable/interface path。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted override/generic method fixtures。
- 完成条件：
  - P4-T04 行为符合 §4.4-§4.5。
- 依赖：P4-T04
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T05：把 constructor overload 纳入 definition-time 规则与 diagnostics

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P4
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §8.3
- 目标：
  - Constructor overload 与 fun overload 共用 signature equivalence、generic shape、vararg overlap 和 diagnostics。
- 必须修改的文件/位置：
  - `crates/scoopc_hir/src/typecheck/overloads.rs::{collect_ctor_decl,check_ctor_overload_set}`
  - `crates/scoopc_hir/src/resolve/mod.rs::ConstructorOverload`
  - `crates/scoopc_hir/src/typecheck/expr/call/ctor.rs::{collect_matched_ctor_overloads_for_owner,pick_most_specific_ctor_overload,select_ctor_overload_for_owner}` for later P5 compatibility
  - constructor parser/header metadata in `crates/scoopc_ast/src/parser/decls.rs`
- 必须实现的内容：
  1. Apply P4-T01 signature equivalence to constructors.
  2. Apply P4-T02 generic shape rules to ctor-level type parameters.
  3. Apply P4-T03 vararg overlap checks to constructors.
  4. Distinguish ctor-level TP from class-level TP in diagnostics and effective signature.
  5. Add fixtures for constructor duplicate signature, generic shape mismatch, and legal overload.
- 必须遵从的约束：
  - Do not treat class-level generic parameters as method/ctor-level specialization knobs.
  - Do not implement call-site constructor specificity in this task; P5 owns selection.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted constructor overload typecheck fixtures.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Definition-time overload contract is uniform for fun, method where applicable, and constructor.
- 依赖：P4-T04R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P4-T05R：Review constructor overload definition-time 规则

- 参考：
  - P4-T05 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §8.3
- 目标：
  - 复核 constructor overload 已纳入 P4 definition-time 规则。
- 必须检查的文件/位置：
  - `crates/scoopc_hir/src/typecheck/overloads.rs`
  - `crates/scoopc_hir/src/resolve/mod.rs`
  - constructor overload fixtures
- 必须实现的内容：
  1. 确认 constructor duplicate signature、generic shape、vararg overlap 均复用 fun overload 规则。
  2. 确认 ctor-level TP 与 class-level TP 区分正确。
  3. 确认 diagnostics 有候选位置和 signature。
  4. 确认未提前实现/改变 P5 call-site constructor specificity。
- 必须遵从的约束：
  - 不得让 constructor 成为 overload definition-time 规则的旁路。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted constructor overload fixtures。
  3. `cargo test --all --all-targets`
- 完成条件：
  - P4 包全部完成；P5 可以基于合法 overload set 做 call-site resolution。
- 依赖：P4-T05
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：
