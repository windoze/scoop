use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::ast;
use crate::cone::ConeId;
use crate::resolve::{ConstructorOverload, FunOverload, Visibility};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::infer::ExpectedTypeFrom;
use super::member::find_member_owner_nominal_instantiation;
use super::ops::{
    collect_member_method_signatures_from_index, is_symbol_visible_from_source,
    literal_absorbs_to_expected, try_extract_member_call_receiver_fqn_and_args,
    try_extract_nominal_fqn_and_args,
};
use super::util::{
    expr_kind_name, fmt_overload_signature, join_overload_signatures, short_name_from_fqn,
};

use super::collect::build_fun_where_constraints_from_resolve_sig;
use super::{EffParamSig, ExprInferInputs, ExprTypeError, FUNPTR_FQN, FunSigOwned, PTR_FQN};

use super::super::assignable::is_type_assignable;
use super::super::eff_row_subst::{
    EffRowVarSubstPlan, apply_eff_row_var_subst_plan, build_eff_row_var_subst_plan,
};
use super::super::lower::{LoweredGenericBound, TypeLowering};
use super::super::type_env::{EnumVariantInfo, TypeSymbol};
use super::super::{TypeLowerError, TypeSymbolKind};

#[derive(Debug, Clone)]
pub(super) enum CallArgKind {
    Positional,
    Named { name: String, name_span: Span },
}

#[derive(Debug, Clone)]
pub(super) struct CallArgInfo<'a> {
    pub(super) kind: CallArgKind,
    pub(super) expr: &'a ast::Expr,
    pub(super) ty: TypeId,
    pub(super) is_spread: bool,
    pub(super) needs_expected_type: bool,
}

#[derive(Clone, Copy)]
struct MemberCallRequest<'a> {
    call_expr: &'a ast::Expr,
    receiver: &'a ast::Expr,
    member: &'a ast::MemberIdent,
    args: &'a [ast::Expr],
    explicit_type_args: Option<&'a [TypeId]>,
    explicit_eff_arg: Option<&'a EffectRow>,
    safe: bool,
}

#[derive(Clone, Copy)]
pub(super) struct EnumVariantCtorTarget<'a> {
    pub(super) enum_fqn: &'a str,
    pub(super) variant_name: &'a str,
    pub(super) callee_span: Span,
}

#[derive(Clone, Copy)]
pub(in super::super) struct EnumTypeSubstContext<'a> {
    pub(in super::super) decl_file: &'a Path,
    pub(in super::super) enum_source: &'a SourceFile,
    pub(in super::super) use_span: Span,
    pub(in super::super) enum_fqn: &'a str,
    pub(in super::super) builtins: BuiltinTypes,
    pub(in super::super) type_param_set: &'a HashSet<String>,
    pub(in super::super) subst: &'a HashMap<String, TypeId>,
}

#[derive(Debug, Clone, Default)]
struct ExplicitTypeApplyArgs {
    type_args: Vec<TypeId>,
    eff_arg: Option<EffectRow>,
}

mod args;
mod ctor;
mod dispatch;
mod effect_op;
mod enum_variant;
mod gates;
mod generic;
mod member_call;
mod value_call;

// Re-export each submodule's API up to `expr::call::*` so the rest of
// `expr` keeps its `super::call::Foo` access pattern. Sibling submodules
// also pick these items up via `use super::*;`. Some globs only re-export
// `pub(super)` helpers used by sibling submodules (visible to call only)
// and contribute nothing at expr level — `#[allow(unused_imports)]`
// silences the corresponding warning.
#[allow(unused_imports)]
pub(super) use {
    args::*, ctor::*, dispatch::*, effect_op::*, enum_variant::*, gates::*, generic::*,
    member_call::*, value_call::*,
};

// `lower_type_ref_with_enum_subst` is consumed by sibling typecheck modules
// (`when_pat`, `when_exhaustiveness`) so it stays visible at the wider
// `crate::typecheck` scope.
pub(in crate::typecheck) use enum_variant::lower_type_ref_with_enum_subst;
