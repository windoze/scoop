#!/usr/bin/env python3
"""Audit the user-visible failure policy boundaries."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Sequence


FAILURE_POLICY_AUDIT_FILES: tuple[str, ...] = ('crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/string.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/class_ctor.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/composed_call.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/continuation.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/effect_outcome.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/emit_entry.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/emitter.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/frame.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/gc_alloc.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/handle_boundary.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/handle_completion.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/lower_source.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_resume.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/mod.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/payload.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_types.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/step_case.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/symbol_naming.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/verification.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/wrapper.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/boxing.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/declare.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/function.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/gc_locals.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/numeric.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/runtime_error.rs',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs',
 'crates/scoopc_mir/src/mir/materialize/dispatch.rs',
 'crates/scoopc_mir/src/mir/materialize/entry.rs',
 'crates/scoopc_mir/src/mir/materialize/generic_mir.rs',
 'crates/scoopc_mir/src/mir/materialize/hir_calls.rs',
 'crates/scoopc_mir/src/mir/materialize/inputs.rs',
 'crates/scoopc_mir/src/mir/materialize/instance.rs',
 'crates/scoopc_mir/src/mir/materialize/mod.rs',
 'crates/scoopc_mir/src/mir/materialize/output.rs',
 'crates/scoopc_mir/src/mir/materialize/reachable.rs',
 'crates/scoopc_mir/src/mir/materialize/rewrite.rs',
 'crates/scoopc_mir/src/mir/materialize/run.rs',
 'crates/scoopc_mir/src/mir/materialize/seed.rs',
 'crates/scoopc_mir/src/mir/materialize/templates.rs',
 'crates/scoopc_mir/src/mir/materialize/utils.rs',
 'crates/scoopc_mir/src/mir/materialize/validation.rs',
 'crates/scoopc/src/pipeline/mir_stage.rs',
 'crates/scoopc_hir/src/stage.rs',
 'crates/scoopc_hir/src/typecheck/lower.rs',
 'crates/scoopc_hir/src/typecheck/when_pat.rs',
 'crates/scoopc_hir/src/typecheck/structs.rs')

FAILURE_KEYWORDS: tuple[str, ...] = ('Unsupported', 'todo!', 'panic!', 'unreachable!')
FRONTEND_REJECT_FORBIDDEN_TERMS: tuple[str, ...] = ('后端', 'backend', 'LLVM', 'codegen', 'Unsupported')
OVERLOAD_DIAGNOSTIC_CODES: tuple[str, ...] = (
    'scoop::typecheck::ambiguous_overload',
    'scoop::typecheck::no_applicable_overload',
    'scoop::typecheck::conflicting_overloads',
    'scoop::typecheck::generic_overload_shape_mismatch',
    'scoop::typecheck::vararg_overlaps_non_vararg',
)
OVERLOAD_DIAGNOSTIC_MARKERS: tuple[tuple[str, str], ...] = (
    (
        'crates/scoopc_hir/src/typecheck/expr/error.rs',
        'scoop::typecheck::ambiguous_overload',
    ),
    (
        'crates/scoopc_hir/src/typecheck/expr/error.rs',
        'scoop::typecheck::no_applicable_overload',
    ),
    (
        'crates/scoopc_hir/src/typecheck/overloads.rs',
        'scoop::typecheck::conflicting_overloads',
    ),
    (
        'crates/scoopc_hir/src/typecheck/overloads.rs',
        'scoop::typecheck::generic_overload_shape_mismatch',
    ),
    (
        'crates/scoopc_hir/src/typecheck/overloads.rs',
        'scoop::typecheck::vararg_overlaps_non_vararg',
    ),
)
OVERLOAD_DIAGNOSTIC_FIXTURE_DATA: tuple[tuple[str, str, tuple[str, ...]], ...] = (
    (
        'tests/fixtures/typecheck/call_overload_ambiguous_is_error.scoop',
        'scoop::typecheck::ambiguous_overload',
        (':18:5', ':22:5', 'reason:'),
    ),
    (
        'tests/fixtures/typecheck/call_overload_no_match_is_error.scoop',
        'scoop::typecheck::no_applicable_overload',
        (':20:5', ':24:5', 'argument 1 type Bool'),
    ),
    (
        'tests/fixtures/typecheck/overload_conflict_return_type_only_is_error.scoop',
        'scoop::typecheck::conflicting_overloads',
        (':15:5', ':19:5'),
    ),
    (
        'tests/fixtures/typecheck/generic_overload_consistency_shape_mismatch_is_error.scoop',
        'scoop::typecheck::generic_overload_shape_mismatch',
        (':15:9', ':17:12'),
    ),
    (
        'tests/fixtures/typecheck/vararg_overload_overlap_is_error.scoop',
        'scoop::typecheck::vararg_overlaps_non_vararg',
        (':15:5', ':19:5'),
    ),
)
POST_HIR_ALLOWED_FAILURE_CLASSIFICATIONS: tuple[str, ...] = (
    "internal bug sentinel",
    "output drift",
    "environment/toolchain/link/runtime path",
)
FAILURE_CLASSIFICATION_RULE_DATA: tuple[tuple[str, str], ...] = (('frontend reject',
  'Inputs outside the current language contract must fail early with a stable, '
  'source-located diagnostic.'),
 ('internal bug sentinel',
  '`panic!`/`unreachable!` may remain only as impossible-state guards after upstream '
  'contracts hold.'),
 ('output drift',
  "Post-HIR verifiers may fail when a later stage's output drifts away from the "
  'already-accepted HIR facts contract.'),
 ('environment/toolchain/link/runtime path',
  'Toolchain, linker, runtime, and execution-environment failures may remain outside '
  'the HIR semantic barrier.'),
 ('stale user-visible failure',
  'Previously user-visible `Unsupported*` buckets must stay replaced by real '
  'implementations, verifier failures, or earlier frontend rejects.'))
FRONTEND_REJECT_SURFACE_DATA: tuple[
    tuple[str, str, str, str, tuple[tuple[str, str], ...]], ...
] = (('B-36 async/await intentionally undefined surface',
  'crates/scoopc_hir/src/resolve/mod.rs',
  'scoop::resolve::async_await_surface_not_defined',
  '当前语言 contract 不定义 `async` / `await` 语法或内建 task runtime surface',
  (('crates/scoopc_hir/src/resolve/mod.rs', 'AsyncAwaitSurfaceNotDefined {'),
   ('tests/fixtures/umb_fix/B-36-spec-uncovered/neg_async_await_surface_rejected.scoop',
    '// EXPECT-ERROR-CODE: scoop::resolve::async_await_surface_not_defined'))),
 ('B-36 generator/yield intentionally undefined surface',
  'crates/scoopc_hir/src/resolve/mod.rs',
  'scoop::resolve::generator_yield_surface_not_defined',
  '当前语言 contract 不定义专用 generator / `yield` 语法；请使用 effect + resuming handler',
  (('crates/scoopc_hir/src/resolve/mod.rs', 'GeneratorYieldSurfaceNotDefined {'),
   ('tests/fixtures/umb_fix/B-36-spec-uncovered/neg_generator_yield_surface_rejected.scoop',
    '// EXPECT-ERROR-CODE: scoop::resolve::generator_yield_surface_not_defined'))),
 ('PIPELINE_GAPS §7.1',
  'crates/scoopc_hir/src/typecheck/expr/error.rs',
  'scoop::typecheck::when_or_pattern_binder_not_allowed',
  '当前语言 contract 下，when or-pattern 不允许引入 binder',
  (('crates/scoopc_hir/src/typecheck/when_pat.rs',
    'ExprTypeError::WhenOrPatternBinderNotAllowed {'),
   ('tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_is_error.scoop',
    '// EXPECT-ERROR: 当前语言 contract 下，when or-pattern 不允许引入 binder'),
   ('tests/fixtures/typecheck/when_or_pattern_variant_payload_binder_sharing_is_error.scoop',
    '// EXPECT-ERROR: 当前语言 contract 下，when or-pattern 不允许引入 binder'))),
 ('PIPELINE_GAPS §7.2',
  'crates/scoopc_hir/src/typecheck/expr/error.rs',
  'scoop::typecheck::function_type_cast_not_supported',
  '当前语言 contract 下，除 `Any as? (...)->R / Pure!` 外不接受函数类型的 runtime cast：{from} -> '
  '{to}；请改用函数子类型/coercion，或先包进 nominal wrapper',
  (('crates/scoopc_hir/src/typecheck/expr/infer.rs',
    'Err(ExprTypeError::FunctionTypeCastNotSupported {'),
   ('crates/scoopc/src/pipeline/mir_stage.rs',
    'fn mir_value_primitives_reject_open_function_type_cast_before_mir()'))),
 ('PIPELINE_GAPS §7.2',
  'crates/scoopc_hir/src/typecheck/expr/error.rs',
  'scoop::typecheck::effectful_function_type_cast_not_supported',
  '当前语言 contract 下，带非 `Pure` effect row 的函数类型不能参与显式 `as`/`as?`：{from} -> {to}；effect '
  'row 只存在于编译期，不能作为 runtime cast 合同',
  (('crates/scoopc_hir/src/typecheck/expr/infer.rs',
    'return Err(ExprTypeError::EffectfulFunctionTypeCastNotSupported {'),
   ('crates/scoopc/src/pipeline/mir_stage.rs',
    'function type runtime cast must be rejected before MIR'))),
 ('type lowering use-site effect row target validation',
  'crates/scoopc_hir/src/typecheck/lower.rs',
  'scoop::typecheck::use_site_eff_arg_not_allowed',
  '当前语言 contract 下，只有显式声明 effect row 形参的名义类型才允许 use-site effect row 实参（`eff '
  '...`）；{name} 不满足该条件',
  (('crates/scoopc_hir/src/typecheck/lower.rs',
    'TypeLowerError::UseSiteEffectRowArgNotAllowed {'),
   ('tests/fixtures/typecheck/use_site_eff_arg_target_without_eff_param_is_error.scoop',
    '// EXPECT-ERROR-CODE: scoop::typecheck::use_site_eff_arg_not_allowed'))),
 ('PIPELINE_GAPS §7.5',
  'crates/scoopc_hir/src/typecheck/structs.rs',
  'scoop::typecheck::struct_field_must_be_val',
  '当前语言 contract 下，struct 字段必须是 `val`，不允许 `var`：{struct_fqn}.{field}',
  (('crates/scoopc_hir/src/typecheck/structs.rs',
    'StructDeclError::StructFieldMustBeVal {'),
   ('tests/fixtures/typecheck/struct_primary_ctor_var_is_error.scoop',
    '// EXPECT-ERROR: 当前语言 contract 下，struct 字段必须是 `val`，不允许 `var`'),
   ('tests/fixtures/typecheck/struct_field_must_be_val_is_error.scoop',
    '// EXPECT-ERROR: 当前语言 contract 下，struct 字段必须是 `val`，不允许 `var`'))),
 ('P2 HIR barrier @CallingConvention non-generic gate',
  'crates/scoopc_hir/src/typecheck/annotations.rs',
  'scoop::typecheck::calling_convention_fun_generics_not_supported',
  '`@CallingConvention` 当前不支持泛型函数',
  (('crates/scoopc_hir/src/typecheck/annotations.rs',
    'AnnotationError::CallingConventionFunGenericsNotSupported {'),
   ('tests/fixtures/typecheck/calling_convention_body_generic_is_error.scoop',
    '// EXPECT-ERROR-CODE: '
    'scoop::typecheck::calling_convention_fun_generics_not_supported'))),
 ('P2 HIR barrier top-level var storage policy gate',
  'crates/scoopc_hir/src/typecheck/annotations.rs',
  'scoop::typecheck::top_level_var_requires_threadlocal_or_global',
  '顶层 `var` 必须显式标注 `@ThreadLocal` 或 `@Global`：{var_name}',
  (('crates/scoopc_hir/src/typecheck/annotations.rs',
    'AnnotationError::TopLevelVarRequiresThreadLocalOrGlobal {'),
   ('tests/fixtures/typecheck/top_level_var_requires_threadlocal_or_global_is_error.scoop',
    '// EXPECT-ERROR-CODE: '
    'scoop::typecheck::top_level_var_requires_threadlocal_or_global'))),
  )
STALE_USER_VISIBLE_UNSUPPORTED_MARKERS: tuple[tuple[str, str], ...] = ()
POST_UPSTREAM_VALIDATION_GUARDS: tuple[tuple[str, str], ...] = (('crates/scoopc_hir/src/typecheck/lower.rs::lower_struct_direct_field_infos',
  'typecheck::check_file_headers rejects struct/class primary-ctor params without a '
  'type annotation via TypeHeaderError::MissingTypeAnnotation'),
 ('crates/scoopc_hir/src/typecheck/when_pat.rs::WhenPat::Variant empty path guard',
  'parser guarantees a `WhenPat::Variant` path always contains at least one segment'),
 ('crates/scoopc_hir/src/typecheck/when_pat.rs::WhenPat::Variant missing enum decl '
  'guard',
  'enum_instance_from_type returns the enum FQN by looking it up in the type env, so '
  'the env must contain a corresponding enum_decl'),
 ('crates/scoopc_hir/src/typecheck/when_pat.rs::WhenPat::Variant type-arg arity guard',
  'enum_instance_from_type produces enum_args from the same nominal instance whose '
  'declaration we just looked up; arity matches by construction'),
 ('crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs::codegen_early_return',
  'typecheck::expr::stmt rejects return outside the immediately enclosing named '
  'function via ReturnNotInFunctionBody'),
 ('crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs::codegen_mir_store_member',
  'typecheck assignment/member-store gate rejects non-writable member-store targets '
  'before LLVM codegen'),
 ('crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs::codegen_mir_member_place',
  'typecheck assignment/member-store gate rejects immutable class member stores before '
  'LLVM codegen'),
 ('crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs::codegen_struct_lit',
  'typecheck struct literal gate rejects missing required fields before LLVM codegen'),
 ('crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs::codegen_mir_make_struct',
  'typecheck/MIR struct gate preserves required field coverage before LLVM codegen'),
 ('crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs::codegen_equality/codegen_bool_logic/coerce_value',
  'typecheck/operator gate proves scalar operator operands, string equality operands, '
  'and coercion targets before LLVM codegen'),
 ('crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs::source_slice_at and '
  'main/literal.rs::codegen_literal',
  'parser/typecheck literal gate proves literal source spans, target types, and string '
  'materialization contracts before LLVM codegen'))
INTERNAL_BUG_SENTINEL_HITS: tuple[str, ...] = ('crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:18:            '
 'panic!("codegen_mir_make_tuple: MIR verifier accepted non-tuple aggregate target '
 'type");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:24:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:31:            '
 'panic!("codegen_mir_make_tuple: MIR verifier accepted tuple aggregate arity drift");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:40:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:64:                    '
 'panic!("codegen_mir_make_tuple: MIR verifier accepted valueless tuple element")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:126:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:240:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:246:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:252:            '
 'panic!("codegen_mir_make_struct: MIR verifier accepted struct without layout")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:255:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:270:                '
 'unreachable!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:275:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:320:                    '
 'panic!("codegen_mir_make_struct: MIR verifier accepted valueless struct field")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:347:            '
 'panic!("codegen_mir_tuple_get: MIR verifier accepted tuple get without operand '
 'type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:350:            '
 'panic!("codegen_mir_tuple_get: MIR verifier accepted tuple get on non-tuple type");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:353:            '
 'panic!("codegen_mir_tuple_get: MIR verifier accepted tuple index drift")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:358:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:363:            '
 'panic!("codegen_mir_tuple_get: TypeStore equivalence verifier accepted unsupported '
 'tuple operand codegen type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:369:                '
 'panic!("codegen_mir_tuple_get: MIR verifier accepted valueless tuple operand")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:440:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:482:            '
 'panic!("scoop_alloc_typed closure allocation must return a pointer");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:539:                '
 'panic!("scoop_alloc_typed closure env allocation must return a pointer");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:556:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:661:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:666:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/aggregates.rs:673:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:19:            '
 'panic!("codegen_mir_class_ctor_ordered_args: MIR verifier accepted {kind}");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:26:                '
 'panic!("codegen_mir_class_ctor_ordered_args: MIR verifier accepted {kind}");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:45:                    '
 'panic!("codegen_mir_class_ctor_ordered_args: MIR verifier accepted {kind}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:254:            '
 'panic!("codegen_bound_mir_call_args_from_signature: MIR call ABI verifier accepted '
 'arg binding drift")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:270:                    '
 'panic!("codegen_bound_mir_call_args_from_signature: TypeStore equivalence verifier '
 'accepted unsupported call arg type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/args.rs:288:                    '
 'panic!("codegen_bound_mir_call_args_from_signature: MIR call ABI verifier accepted '
 'missing evaluated arg slot")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:32:                        '
 'panic!("codegen_mir_call: MIR call ABI verifier accepted non-function closure '
 'callee")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:40:                        '
 'panic!("codegen_mir_call: MIR call ABI verifier accepted non-function function-value '
 'callee")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:48:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:171:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:176:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:203:                '
 'panic!("codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted '
 'unsupported direct call return type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:327:                            '
 'panic!("codegen_mir_direct_call_with_policy: MIR call ABI verifier accepted missing '
 'deferred return value")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:345:            '
 '.unwrap_or_else(|| panic!("codegen_mir_class_ctor_call_at_site: verifier accepted '
 'missing class ctor call site"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:379:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:405:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:432:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:438:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:494:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:514:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:528:                '
 'panic!("codegen_mir_function_value_call_from_closure_obj: MIR call ABI verifier '
 'accepted unsupported function-value return type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:563:            (None, '
 'CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:680:                            '
 'panic!("codegen_mir_function_value_call_from_closure_obj: MIR call ABI verifier '
 'accepted missing deferred return value")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:718:                        '
 'panic!("codegen_mir_plain_dynamic_call_with_policy: MIR verifier accepted '
 'non-function plain closure callee")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:732:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:737:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:749:                        '
 'panic!("codegen_mir_plain_dynamic_call_with_policy: MIR call ABI verifier accepted '
 'non-function plain function-value callee")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:758:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:763:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:775:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:780:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:817:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs:836:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:116:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:121:            '
 'panic!("ensure_lir_source_closure_callable_defined: callable is not a closure body")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:162:        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:209:            '
 'panic!("declare_lir_source_closure_fun_with_signature: LIR source verifier accepted '
 'unsupported closure return type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:231:                    '
 'panic!("declare_lir_source_closure_fun_with_signature: TypeStore equivalence '
 'verifier accepted unsupported closure param type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:285:            '
 'panic!("declare_lir_source_plain_fun_with_symbol: LIR source ABI verifier accepted '
 'unsupported plain return type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:303:                '
 'panic!("declare_lir_source_plain_fun_with_symbol: TypeStore equivalence verifier '
 'accepted unsupported plain param type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:377:                        '
 'panic!("declare_lir_plain_fun_with_symbol: TypeStore equivalence verifier accepted '
 'unsupported plain param type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:399:                '
 'panic!("declare_lir_plain_fun_with_symbol: LIR facts verifier accepted unsupported '
 'plain return type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:480:            '
 'panic!("codegen_lir_source_closure_fun: LIR source ABI verifier accepted invalid '
 'CFG")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:488:                '
 'panic!("codegen_lir_source_closure_fun: LIR source verifier accepted unsupported '
 'closure return type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:502:                        '
 'panic!("codegen_lir_source_closure_fun: LIR source ABI verifier accepted missing '
 'sret param")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:535:                '
 'panic!("codegen_lir_source_closure_fun: LIR source ABI verifier accepted missing '
 'start block")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:575:            '
 'panic!("bind_mir_closure_params: MIR verifier accepted closure without env param")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:578:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:586:                '
 'panic!("bind_mir_closure_params: MIR verifier accepted missing env local slot")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:602:                    '
 'panic!("bind_mir_closure_params: MIR call ABI verifier accepted missing param local '
 'slot")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:605:                '
 'panic!("bind_mir_closure_params: TypeStore equivalence verifier accepted unsupported '
 'param type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:612:                        '
 'panic!("bind_mir_closure_params: MIR call ABI verifier accepted missing LLVM param")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:647:                        '
 'panic!("codegen_mir_closure_env_param: MIR verifier accepted non-tuple closure env")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:652:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/callable_lookup.rs:692:            '
 '_ => panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:20:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:46:            '
 'panic!("codegen_mir_cast: MIR verifier accepted runtime cast metadata drift");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:73:            '
 'panic!("codegen_mir_cast_as: MIR verifier accepted invalid class-cast panic contract");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:70:            '
 'panic!("codegen_mir_cast_as: MIR verifier accepted invalid `as` failure contract");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:78:            '
 'panic!("codegen_mir_cast_as: MIR verifier accepted invalid `as` result contract");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:81:            '
 'panic!("codegen_mir_cast_as: MIR verifier accepted invalid `as` target contract");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:87:                '
 'panic!("codegen_mir_cast_as: TypeStore equivalence verifier accepted unsupported '
 '`as` target codegen type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:92:                '
 'panic!("codegen_mir_cast_as: MIR verifier accepted unsupported `as` target type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:139:            '
 'panic!("codegen_mir_cast_asq: MIR verifier accepted invalid `as?` failure '
 'contract");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:142:            '
 'panic!("codegen_mir_cast_asq: MIR verifier accepted invalid `as?` result contract");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:145:            '
 'panic!("codegen_mir_cast_asq: MIR verifier accepted invalid `as?` target contract");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:151:                '
 'panic!("codegen_mir_cast_asq: TypeStore equivalence verifier accepted unsupported '
 '`as?` target codegen type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:156:                '
 'panic!("codegen_mir_cast_asq: MIR verifier accepted unsupported `as?` target type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:159:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:166:                '
 'panic!("codegen_mir_cast_asq: TypeStore equivalence verifier accepted unsupported '
 '`as?` option codegen type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:169:            '
 'panic!("codegen_mir_cast_asq: MIR verifier accepted invalid `as?` result type");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:209:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:215:            '
 '.unwrap_or_else(|| panic!("codegen_mir_runtime_type_test_is_ok: TypeStore '
 'equivalence verifier accepted unsupported runtime type target"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:231:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:237:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:252:            _ => '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:336:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/cast.rs:345:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:22:                        '
 'panic!("codegen_mir_const: parser/typecheck accepted invalid char literal")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:37:                    '
 '_ => panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:62:                    '
 '_ => panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:144:                    '
 'panic!("codegen_mir_pattern_match_value: MIR verifier accepted Int pattern for '
 'non-int subject")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:156:                    '
 'panic!("codegen_mir_pattern_match_value: MIR verifier accepted Char pattern for '
 'non-int subject")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:167:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:172:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:180:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:200:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:214:                    '
 '.unwrap_or_else(|| panic!("codegen_mir_pattern_match_value: MIR verifier accepted '
 'Bool pattern for non-bool subject"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:234:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:240:            '
 '.unwrap_or_else(|| panic!("codegen_mir_is_pattern_match: MIR verifier accepted '
 'unsupported pattern subject metadata type"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:242:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:256:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:262:            '
 '.unwrap_or_else(|| panic!("codegen_mir_is_pattern_match: MIR verifier accepted '
 'unsupported runtime target type"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:265:            '
 '.unwrap_or_else(|| panic!("codegen_mir_is_pattern_match: MIR verifier accepted '
 'non-codegen runtime target type"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:267:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:276:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:282:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:297:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:303:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:308:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:323:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:331:                '
 'panic!("codegen_mir_tuple_pattern_match: MIR verifier accepted unsupported tuple '
 'pattern element type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:352:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:357:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:369:                '
 '.unwrap_or_else(|| panic!("codegen_mir_variant_pattern_match: MIR verifier accepted '
 'unknown enum variant pattern"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:381:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:462:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:467:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:472:                        '
 'panic!("codegen_mir_pattern_extract: MIR verifier accepted tuple extraction field '
 'drift")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:486:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:496:                        '
 '.unwrap_or_else(|| panic!("codegen_mir_pattern_extract: MIR verifier accepted '
 'unknown enum variant extraction"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/const_pat.rs:550:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:157:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:163:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:293:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:328:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:374:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:416:            '
 '.unwrap_or_else(|| panic!("codegen_mir_plain_dispatch_call: verifier accepted '
 'missing dispatch receiver pointer"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:551:                    '
 '.unwrap_or_else(|| panic!("selected_mir_class_ctor_from_contract: verifier accepted '
 'missing selected ctor")),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/dispatch.rs:555:                '
 'panic!("selected_mir_class_ctor_from_contract: verifier accepted unselected ctor '
 'contract");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:73:            '
 '.unwrap_or_else(|| panic!("codegen_mir_enum_variant_ctor_call: verifier accepted '
 'enum ctor TypeStore drift"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:79:            '
 '.unwrap_or_else(|| panic!("codegen_mir_enum_variant_ctor_call: verifier accepted '
 'unknown enum variant `{variant_name}`"))',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:82:            '
 'panic!("codegen_mir_enum_variant_ctor_call: verifier accepted enum ctor arity '
 'drift");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:85:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:92:                '
 'panic!("codegen_mir_enum_variant_ctor_call: verifier accepted named enum ctor arg");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:130:            '
 'unreachable!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:136:            '
 '.unwrap_or_else(|| panic!("codegen_mir_store_member: verifier accepted non-codegen '
 'member store value type"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:140:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:145:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:150:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:178:                '
 'panic!("codegen_mir_store_top_level_var: extern LIR root is missing contract")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:181:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:197:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:240:                '
 '.unwrap_or_else(|| panic!("codegen_mir_member_place: verifier accepted missing class '
 'member receiver operand type"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:243:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:249:                    '
 'unreachable!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:282:            '
 'panic!("codegen_mir_member_place: verifier accepted non-local member store '
 'receiver");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:286:            '
 'panic!("codegen_mir_member_place: verifier accepted member receiver slot type '
 'drift");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/member.rs:312:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:260:            '
 'panic!("mir_member_value_fqn_for_codegen: verifier accepted non-value member '
 'target")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:263:            '
 'panic!("mir_member_value_fqn_for_codegen: verifier accepted unresolved member '
 'target")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:275:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:282:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/mod.rs:287:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:349:            '
 'panic!("mir_closure_env_capture_element_cg_tys_from_contract: TypeStore equivalence '
 'verifier accepted unsupported closure env contract codegen type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:352:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:360:                '
 'panic!("mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted '
 'non-tuple closure env")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:363:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:385:            _ => '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:396:                        '
 'panic!("mir_closure_env_capture_element_cg_tys_from_contract: MIR verifier accepted '
 'missing closure env capture element")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/operand.rs:401:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/string.rs:15:            '
 'panic!("codegen_mir_unresolved_name_with_source_ty: MIR verifier accepted '
 'unsupported source type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:34:                '
 'panic!("bind_mir_params: MIR verifier accepted param arity drift")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:47:                    '
 'panic!("bind_mir_params: MIR verifier accepted unsupported param type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:121:                    '
 'panic!("bind_lir_source_params: LIR verifier accepted unsupported param type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:425:                        '
 'panic!("codegen_mir_rvalue: MIR verifier accepted closure env without codegen type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/terminator.rs:605:                    '
 'panic!("codegen_mir_effect_neutral_rvalue: MIR verifier accepted closure env without '
 'codegen type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:72:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:209:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:215:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:220:            '
 'panic!("codegen_platform_literal: sysroot contract accepted non-Platform result '
 'type");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:225:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:246:                    '
 'panic!("codegen_platform_literal: sysroot contract accepted unknown Platform field '
 '`{}`", layout_field.name)',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/transport.rs:278:                '
 'panic!("codegen_platform_literal: verified Platform field materialized without LLVM '
 'value")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs:44:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs:74:            '
 'panic!("mir_local_storage_cg_ty: MIR verifier accepted unsupported local type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/types.rs:747:                '
 'panic!("mir_member_receiver_codegen_type_id: verifier accepted member receiver '
 'TypeStore drift")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:19:            '
 'panic!("codegen_mir_callable_value_args: MIR call ABI verifier accepted invalid '
 'closure argument binding")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:43:                    '
 'panic!("codegen_mir_callable_value_args: MIR call ABI verifier accepted missing '
 'closure argument")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:91:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:104:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/value_args.rs:124:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs:89:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs:97:                        '
 'panic!("lower_dynamic_call_carrier: TypeStore equivalence verifier accepted '
 'unsupported FunPtr carrier codegen type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs:109:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/call_invoke.rs:123:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/composed_call.rs:481:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/composed_call.rs:491:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:431:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:453:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:462:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:469:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_carrier.rs:479:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:441:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:470:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:485:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:568:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:663:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:681:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:690:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:700:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/main_entry.rs:717:            '
 '| mir::TerminatorKind::Todo(_) => panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/payload.rs:80:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/payload.rs:388:                '
 'panic!("lower_completion_payload_as: effect verifier accepted unsupported completion '
 'payload target type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs:84:                '
 'panic!("runtime_error_unit_variant_payload: verifier accepted missing RuntimeError '
 'LIR enum layout")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs:91:                '
 'panic!("runtime_error_unit_variant_payload: verifier accepted missing RuntimeError '
 'variant `{variant_name}`")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs:94:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/runtime_error.rs:150:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:60:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:203:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:293:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:301:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/states.rs:370:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/body/wrapper.rs:488:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:233:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:316:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:547:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:564:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:606:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:950:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1010:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1343:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1384:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1432:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1552:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1578:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1616:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:1723:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2288:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2319:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2327:                    '
 '_ => unreachable!("filtered by match"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2352:            '
 '_ => panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2380:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2398:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2422:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2435:                '
 '_ => panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2654:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2939:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2949:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2973:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2985:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:2996:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3011:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3017:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3038:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3120:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3130:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3404:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3425:            '
 '_ => unreachable!("filtered by caller"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3738:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3761:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3809:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/effect_lowered/value.rs:3942:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/alloca.rs:48:            _ => '
 'unreachable!("cast_float only accepts Float64/Float32"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs:51:                        '
 'panic!("codegen_addressable_place: extern LIR root is missing contract")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs:105:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/call.rs:714:            '
 'unreachable!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:44:                    _ '
 '=> unreachable!("filtered by caller"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:49:            '
 'panic!("codegen_equality: typecheck gate accepted non-string rhs for string '
 'equality");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:60:                _ => '
 'unreachable!("filtered by caller"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:89:                _ => '
 'unreachable!("filtered by caller"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:98:                    '
 'panic!("codegen_equality: typecheck gate accepted incompatible float operands")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:105:                _ => '
 'unreachable!("filtered by caller"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:117:            '
 'panic!("codegen_equality: typecheck gate accepted incompatible integer operands")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:126:            _ => '
 'unreachable!("filtered by caller"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:148:            _ => '
 'unreachable!("filtered by caller"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/coerce.rs:293:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:14:            '
 'panic!("expect_insert_block: missing LLVM insert block while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:25:            '
 'panic!("expect_parent_function: block has no parent function while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:45:            '
 'panic!("expect_instruction_parent_block: instruction has no parent block while '
 '{context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:66:            '
 'panic!("expect_entry_block: function has no entry block while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:77:            '
 'panic!("expect_basic_value: call did not produce a basic value while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:89:            _ => '
 'panic!("expect_pointer_value: value was not a pointer while {context}"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:101:            _ => '
 'panic!("expect_int_value: value was not an integer while {context}"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:113:            _ => '
 'panic!("expect_float_value: value was not a float while {context}"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:125:            _ => '
 'panic!("expect_struct_value: value was not a struct while {context}"),',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:136:            '
 'panic!("expect_cg_bool: value was not a bool payload while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:147:            '
 'panic!("expect_cg_int: value was not an integer payload while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:158:            '
 'panic!("expect_cg_float: value was not a float payload while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:169:            '
 'panic!("expect_cg_pointer: value did not publish a payload while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:181:            '
 'panic!("expect_cg_value: value did not publish a payload while {context}")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:469:            '
 '.unwrap_or_else(|| panic!("{context}: LIR facts are missing global root `{fqn}`"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:471:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:486:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:499:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:522:                '
 'panic!("source_slice_at: parser/typecheck accepted a span outside its source")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/context.rs:525:            '
 'panic!("source_slice_at: parser/typecheck accepted an unsliceable source span")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/declare.rs:140:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/declare.rs:146:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:75:                '
 'panic!("codegen_type_check_expr: typecheck gate accepted non-runtime-ref operand");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:79:            '
 'panic!("codegen_type_check_expr: typecheck gate accepted valueless operand");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:82:            '
 'panic!("codegen_type_check_expr: typecheck gate accepted non-pointer runtime '
 'operand");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:118:                '
 'panic!("codegen_cast_as_expr: typecheck gate accepted non-runtime-ref target")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:127:                '
 'panic!("codegen_cast_as_expr: typecheck gate accepted non-runtime-ref operand")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:190:            '
 'panic!("codegen_cast_asq_expr: typecheck gate accepted non-Option `as?` result '
 'type");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:198:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:209:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:215:            '
 'panic!("codegen_cast_asq_expr: typecheck gate accepted valueless `as?` operand");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:218:            '
 'panic!("codegen_cast_asq_expr: typecheck gate accepted non-pointer `as?` operand");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:243:            '
 'panic!("codegen_cast_asq_expr: verified Option Some produced no value")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:251:            '
 'panic!("codegen_cast_asq_expr: verified Option None produced no value")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:369:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_op.rs:373:            _ => '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:17:                    '
 'panic!("codegen_var_ref: HIR verifier accepted an unbound local value")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:130:            '
 'panic!("codegen_struct_lit: typecheck accepted non-struct struct literal type");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:135:            '
 'panic!("codegen_struct_lit: typecheck accepted struct literal without nominal '
 'schema");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:140:            '
 'panic!("codegen_struct_lit: typecheck accepted struct literal without layout")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:150:                '
 'unreachable!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:200:                    '
 'panic!("codegen_struct_lit: typecheck accepted valueless struct field")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:221:            '
 'panic!("codegen_tuple_lit: typecheck accepted non-tuple tuple literal type");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:226:            '
 'panic!("codegen_tuple_lit: typecheck accepted tuple literal without tuple schema");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:230:            '
 'panic!("codegen_tuple_lit: typecheck accepted tuple literal arity drift");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:239:                '
 'panic!("codegen_tuple_lit: typecheck accepted unsupported tuple element type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:265:                    '
 'panic!("codegen_tuple_lit: typecheck accepted valueless tuple element")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:373:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:392:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:401:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/expr_value.rs:435:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:436:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:472:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:495:                        '
 'panic!("rebuild_value_from_storage_slot_with_explicit_frame: explicit-frame slot '
 'metadata ended before aggregate rebuild completed")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:643:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:701:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/frame.rs:804:            '
 'panic!("finalize_function_explicit_frame_lifecycle: verifier accepted missing '
 'explicit root frame descriptor global")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/function.rs:54:                    '
 'panic!("emit_function_return_block: function return context must publish return '
 'alloca")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/gc_locals.rs:85:            '
 'panic!("rematerialize_ptr_in_current_block: local slot GEP must publish source '
 'type")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/gc_locals.rs:305:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs:22:            '
 'panic!("defer_class_field_place: verifier accepted class field index drift")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs:144:            panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/globals.rs:169:            '
 'panic!("declare_extern_global_storage: verifier accepted extern global '
 'initializer");',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs:376:            '
 'panic!("stable_closure_key_for_lir_source_callable: LIR source contract accepted '
 'missing closure callable")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs:379:            '
 'panic!("stable_closure_key_for_lir_source_callable: callable is not a closure body")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/identity.rs:409:            '
 'panic!("direct_hir_closure_callable_fqn: closure codegen requires current callable '
 'owner")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:38:            '
 'panic!("codegen_decl_initializer_expr: typed HIR val must publish initializer")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:158:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:199:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:333:            '
 '.unwrap_or_else(|| panic!("codegen_top_level_immutable_value_init_fun_body: verifier '
 'accepted immutable value without initializer"));',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:427:                '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/immut_value.rs:447:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:36:                        '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:40:                    '
 'panic!("emit_return: MIR return contract accepted missing return value")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/literal.rs:79:                    '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/runtime_error.rs:211:                '
 'panic!("emit_raise_runtime_error_variant: verifier accepted missing RuntimeError '
 'unit variant `{variant_name}`")',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/main/runtime_error.rs:214:            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs:952:                    panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs:962:                            '
 'panic!(',
 'crates/scoopc_codegen_llvm/src/llvm/codegen/mod.rs:973:                                '
 'panic!(',
 'crates/scoopc_mir/src/mir/materialize/dispatch.rs:51:                    panic!(',
 'crates/scoopc_mir/src/mir/materialize/output.rs:40:            panic!(',
 'crates/scoopc_hir/src/typecheck/lower.rs:1242:                unreachable!(',
 'crates/scoopc_hir/src/typecheck/lower.rs:2487:                    _ => '
 'unreachable!("nominal lowering produced non-nominal type"),',
 'crates/scoopc_hir/src/typecheck/lower.rs:3380:                    _ => '
 'unreachable!("nominal lowering produced non-nominal type"),',
 'crates/scoopc_hir/src/typecheck/lower.rs:3631:                '
 'unreachable!("is_value_only_enum implies first_super exists");',
 'crates/scoopc_hir/src/typecheck/when_pat.rs:211:                    unreachable!(',
 'crates/scoopc_hir/src/typecheck/when_pat.rs:260:                unreachable!(')
CHECK_COUNT = 7


class AuditError(Exception):
    """Raised when the failure-policy audit cannot complete."""


@dataclass(frozen=True)
class FailureClassificationRule:
    name: str
    meaning: str


@dataclass(frozen=True)
class SourceMarker:
    path: str
    marker: str


@dataclass(frozen=True)
class FrontendRejectSurface:
    gap_id: str
    definition_path: str
    diagnostic_code: str
    message: str
    markers: tuple[SourceMarker, ...]


@dataclass(frozen=True)
class UpstreamGuardedSentinel:
    site: str
    upstream_gate: str


FAILURE_CLASSIFICATION_RULES: tuple[FailureClassificationRule, ...] = tuple(
    FailureClassificationRule(name, meaning)
    for name, meaning in FAILURE_CLASSIFICATION_RULE_DATA
)
FRONTEND_REJECT_SURFACES: tuple[FrontendRejectSurface, ...] = tuple(
    FrontendRejectSurface(
        gap_id=gap_id,
        definition_path=definition_path,
        diagnostic_code=diagnostic_code,
        message=message,
        markers=tuple(SourceMarker(path, marker) for path, marker in markers),
    )
    for gap_id, definition_path, diagnostic_code, message, markers in FRONTEND_REJECT_SURFACE_DATA
)
UPSTREAM_VALIDATION_GUARDS: tuple[UpstreamGuardedSentinel, ...] = tuple(
    UpstreamGuardedSentinel(site, upstream_gate)
    for site, upstream_gate in POST_UPSTREAM_VALIDATION_GUARDS
)


@dataclass(frozen=True)
class AuditReport:
    internal_bug_sentinels: int
    failures: tuple[str, ...]

    def render(self) -> str:
        if self.failures:
            lines = [
                f"user-visible failure policy audit: failed ({len(self.failures)} violations)"
            ]
            lines.extend(f"- {failure}" for failure in self.failures)
            return "\n".join(lines)
        return (
            "user-visible failure policy audit: ok "
            f"(files={len(FAILURE_POLICY_AUDIT_FILES)}, "
            f"rules={len(FAILURE_CLASSIFICATION_RULES)}, "
            f"failure_keywords={len(FAILURE_KEYWORDS)}, "
            f"allowed_post_hir_classes={len(POST_HIR_ALLOWED_FAILURE_CLASSIFICATIONS)}, "
            f"frontend_rejects={len(FRONTEND_REJECT_SURFACES)}, "
            f"upstream_guards={len(UPSTREAM_VALIDATION_GUARDS)}, "
            f"stale_unsupported={len(STALE_USER_VISIBLE_UNSUPPORTED_MARKERS)}, "
            f"internal_bug_sentinels={self.internal_bug_sentinels}, "
            f"checks={CHECK_COUNT})"
        )

    def has_failures(self) -> bool:
        return bool(self.failures)


def main(argv: Sequence[str] | None = None) -> int:
    parse_args(argv)
    try:
        report = run()
    except AuditError as exc:
        print(f"user-visible failure policy audit: error: {exc}", file=sys.stderr)
        return 1

    print(report.render(), file=sys.stderr)
    return 1 if report.has_failures() else 0


def parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Audit user-visible failure policy boundaries."
    )
    return parser.parse_args(argv)


def run() -> AuditReport:
    repo_root = Path(__file__).resolve().parent.parent
    failures: list[str] = []

    failures.extend(check_scope_keywords_and_rules(repo_root))
    failures.extend(check_frontend_reject_surfaces(repo_root))
    failures.extend(check_overload_diagnostic_policy(repo_root))
    failures.extend(check_stale_unsupported_markers(repo_root))
    failures.extend(check_upstream_guards())
    failures.extend(check_no_production_todo_macros(repo_root))

    internal_bug_hits = collect_production_line_hits(
        repo_root, lambda line: "panic!" in line or "unreachable!" in line
    )
    failures.extend(
        compare_lines(
            "internal bug sentinel markers",
            sorted(normalize_line_hit_marker(hit) for hit in internal_bug_hits),
            sorted(normalize_line_hit_marker(hit) for hit in INTERNAL_BUG_SENTINEL_HITS),
        )
    )

    return AuditReport(
        internal_bug_sentinels=len(internal_bug_hits), failures=tuple(failures)
    )


def check_scope_keywords_and_rules(repo_root: Path) -> list[str]:
    failures: list[str] = []
    for path in FAILURE_POLICY_AUDIT_FILES:
        if not (repo_root / path).is_file():
            failures.append(f"failure-policy audit file should exist: {path}")

    rule_names = tuple(rule.name for rule in FAILURE_CLASSIFICATION_RULES)
    expected_rule_names = (
        "frontend reject",
        "internal bug sentinel",
        "output drift",
        "environment/toolchain/link/runtime path",
        "stale user-visible failure",
    )
    if rule_names != expected_rule_names:
        failures.append(
            "classification rule names drifted: "
            f"expected {format_lines(expected_rule_names)}, got {format_lines(rule_names)}"
        )

    expected_allowed = (
        "internal bug sentinel",
        "output drift",
        "environment/toolchain/link/runtime path",
    )
    if POST_HIR_ALLOWED_FAILURE_CLASSIFICATIONS != expected_allowed:
        failures.append(
            "post-HIR allowed failure classes drifted: "
            f"expected {format_lines(expected_allowed)}, "
            f"got {format_lines(POST_HIR_ALLOWED_FAILURE_CLASSIFICATIONS)}"
        )

    expected_keywords = ("Unsupported", "todo!", "panic!", "unreachable!")
    if FAILURE_KEYWORDS != expected_keywords:
        failures.append(
            "failure keywords drifted: "
            f"expected {format_lines(expected_keywords)}, got {format_lines(FAILURE_KEYWORDS)}"
        )

    return failures


def check_frontend_reject_surfaces(repo_root: Path) -> list[str]:
    failures: list[str] = []
    for surface in FRONTEND_REJECT_SURFACES:
        definition = read_repo_file(repo_root, surface.definition_path)
        if surface.diagnostic_code not in definition:
            failures.append(
                f"missing frontend reject diagnostic code `{surface.diagnostic_code}` "
                f"in {surface.definition_path}"
            )
        if surface.message not in definition:
            failures.append(
                f"missing frontend reject message `{surface.message}` "
                f"in {surface.definition_path}"
            )
        for forbidden in FRONTEND_REJECT_FORBIDDEN_TERMS:
            if forbidden in surface.message:
                failures.append(
                    f"frontend reject `{surface.gap_id}` should not read like backend "
                    f"unsupported: {surface.message}"
                )
        for marker in surface.markers:
            failures.extend(check_source_marker(repo_root, marker))
    return failures


def check_overload_diagnostic_policy(repo_root: Path) -> list[str]:
    failures: list[str] = []
    for path, marker in OVERLOAD_DIAGNOSTIC_MARKERS:
        failures.extend(check_source_marker(repo_root, SourceMarker(path, marker)))

    for fixture_path, diagnostic_code, required_substrings in OVERLOAD_DIAGNOSTIC_FIXTURE_DATA:
        content = read_repo_file(repo_root, fixture_path)
        directives = tuple(fixture_directives(content))
        expected_code = f"EXPECT-ERROR-CODE: {diagnostic_code}"
        if expected_code not in directives:
            failures.append(
                f"overload diagnostic fixture {fixture_path} should assert `{expected_code}`"
            )
        for substring in required_substrings:
            if not any(
                directive.startswith("EXPECT-ERROR:") and substring in directive
                for directive in directives
            ):
                failures.append(
                    f"overload diagnostic fixture {fixture_path} should assert `EXPECT-ERROR` containing `{substring}`"
                )
        for forbidden in FRONTEND_REJECT_FORBIDDEN_TERMS:
            expected_forbidden = f"EXPECT-NOT-ERROR: {forbidden}"
            if expected_forbidden not in directives:
                failures.append(
                    f"overload diagnostic fixture {fixture_path} should assert `{expected_forbidden}`"
                )
    return failures


def fixture_directives(content: str) -> list[str]:
    directives: list[str] = []
    for raw_line in content.splitlines()[:32]:
        trimmed = raw_line.lstrip()
        if not trimmed.startswith("//"):
            break
        directives.append(trimmed[2:].strip())
    return directives


def check_stale_unsupported_markers(repo_root: Path) -> list[str]:
    failures: list[str] = []
    if STALE_USER_VISIBLE_UNSUPPORTED_MARKERS:
        failures.append(
            "P7-T02 final state requires stale user-visible Unsupported* markers to "
            "remain empty"
        )
    for path, marker in STALE_USER_VISIBLE_UNSUPPORTED_MARKERS:
        failures.extend(check_source_marker(repo_root, SourceMarker(path, marker)))
    return failures


def check_upstream_guards() -> list[str]:
    if UPSTREAM_VALIDATION_GUARDS:
        return []
    return ["post-upstream-validation guards should remain documented"]


def check_no_production_todo_macros(repo_root: Path) -> list[str]:
    observed = collect_production_line_hits(repo_root, lambda line: "todo!" in line)
    if not observed:
        return []
    return [
        "production failure-policy audit roots must not contain todo!: "
        + format_lines(observed)
    ]


def check_source_marker(repo_root: Path, marker: SourceMarker) -> list[str]:
    content = read_repo_file(repo_root, marker.path)
    if marker.marker in content:
        return []
    return [f"missing marker `{marker.marker}` in {marker.path}"]


def collect_production_line_hits(
    repo_root: Path, matches: Callable[[str], bool]
) -> list[str]:
    hits: list[str] = []
    for path in FAILURE_POLICY_AUDIT_FILES:
        content = production_source_text(repo_root, path)
        for index, line in enumerate(content.splitlines(), start=1):
            if matches(line):
                hits.append(f"{path}:{index}:{line}")
    return hits


def production_source_text(repo_root: Path, path: str) -> str:
    content = read_repo_file(repo_root, path)
    cutoffs = [
        index
        for index in (
            content.find("\n#[cfg(test)]"),
            content.find("\n#[cfg(all(test"),
        )
        if index != -1
    ]
    if not cutoffs:
        return content
    return content[: min(cutoffs)]


def compare_lines(
    label: str, observed: Sequence[str], expected: Sequence[str]
) -> list[str]:
    if list(observed) == list(expected):
        return []
    return [
        f"{label} drifted: expected {format_lines(expected)}, got {format_lines(observed)}"
    ]


def normalize_line_hit_marker(line: str) -> str:
    return re.sub(r":\d+:", ":<line>:", line, count=1)


def format_lines(lines: Sequence[str]) -> str:
    if not lines:
        return "[]"
    return "[" + ", ".join(repr(line) for line in lines) + "]"


def read_repo_file(repo_root: Path, path: str) -> str:
    try:
        with (repo_root / path).open("r", encoding="utf-8", newline="") as file:
            return file.read()
    except OSError as exc:
        raise AuditError(f"failed to read {path}: {exc}") from exc


if __name__ == "__main__":
    raise SystemExit(main())
