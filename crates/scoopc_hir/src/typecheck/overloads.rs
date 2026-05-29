//! 重载声明冲突诊断（T0457）。
//!
//! 目标：
//! - 在 typecheck 阶段（不进入函数体）提前诊断“永远无法在调用点消歧”的 overload 集：
//!   - 完全相同签名（重复定义）
//!   - 仅返回类型不同（返回类型不参与重载决议）
//!   - 默认参数导致的不可区分（例如 `f(x:Int)` 与 `f(x:Int, y:Int=0)`）
//!   - vararg 与非 vararg 在相同调用 arity 下不可区分
//! - 当前实现先覆盖：
//!   - 顶层/成员 `fun`
//!   - class 的 primary/secondary constructors
//! - 先按“位置实参（positional call）”做最小分析：仅考虑尾部默认参数可被省略的调用形态。
//!   更完整的 named args / 中间省略规则由后续任务补齐（与 T1305/T1306 联动）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{ImportTable, Index};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, FunctionType, NominalType, RefTypeKind, StarProjectionType, TypeId,
    TypeKind, TypeParamType, TypeStore, ValueTypeKind,
};

use super::TypeEnv;
use super::assignable::is_type_assignable;
use super::lower::{LoweredGenericBound, TypeLowerError, TypeLowering, build_where_bound_entries};

#[derive(Debug, Error, Diagnostic)]
pub enum OverloadDeclError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLower(#[from] TypeLowerError),

    #[error("重载签名冲突：{fqn}（{reason}）：{previous_candidate} <-> {conflict_candidate}")]
    #[diagnostic(code(scoop::typecheck::conflicting_overloads))]
    Conflict {
        fqn: Box<str>,
        reason: Box<str>,
        previous_candidate: Box<str>,
        conflict_candidate: Box<str>,
        #[label("冲突声明在这里")]
        conflict: miette::SourceSpan,
        #[label("第一次声明在这里")]
        previous: miette::SourceSpan,
    },

    #[error(
        "不支持的泛型重载 shape：{fqn}：{previous_candidate} <-> {conflict_candidate}；only differ-by-bound generic overloads are supported; rename the function or restructure"
    )]
    #[diagnostic(
        code(scoop::typecheck::generic_overload_shape_mismatch),
        help(
            "only differ-by-bound generic overloads are supported; rename the function or restructure"
        )
    )]
    GenericShapeMismatch {
        fqn: Box<str>,
        previous_candidate: Box<str>,
        conflict_candidate: Box<str>,
        #[label("泛型重载 shape 不兼容")]
        conflict: miette::SourceSpan,
        #[label("候选声明在这里")]
        previous: miette::SourceSpan,
    },

    #[error(
        "vararg 与非 vararg 重载重叠：{fqn} 在 {arity} 个实参时不可区分：{previous_candidate} <-> {conflict_candidate}"
    )]
    #[diagnostic(
        code(scoop::typecheck::vararg_overlaps_non_vararg),
        help(
            "rename one overload or change the fixed parameters so their accepted arities/types do not overlap"
        )
    )]
    VarargOverlapsNonVararg {
        fqn: Box<str>,
        arity: usize,
        previous_candidate: Box<str>,
        conflict_candidate: Box<str>,
        #[label("vararg/non-vararg 重叠声明在这里")]
        conflict: miette::SourceSpan,
        #[label("候选声明在这里")]
        previous: miette::SourceSpan,
    },
}

#[derive(Debug, Clone)]
struct ParamInfo {
    ty: TypeId,
    effective_ty: EffectiveType,
    shape: GenericShape,
    has_default: bool,
    is_vararg: bool,
}

#[derive(Debug, Clone)]
struct FunSigInfo {
    receiver_ty: Option<TypeId>,
    receiver: Option<EffectiveType>,
    receiver_shape: Option<GenericShape>,
    params: Vec<ParamInfo>,
    return_ty: Option<TypeId>,
    effects: EffectRow,
}

#[derive(Debug, Clone)]
struct FunDeclInfo {
    decl_file: PathBuf,
    name_span: Span,
    sig: FunSigInfo,
}

#[derive(Debug, Clone)]
struct CtorSigInfo {
    params: Vec<ParamInfo>,
}

#[derive(Debug, Clone, Copy)]
struct EffectiveSignature<'a> {
    receiver_ty: Option<TypeId>,
    receiver: Option<&'a EffectiveType>,
    receiver_shape: Option<&'a GenericShape>,
    params: &'a [ParamInfo],
}

impl FunSigInfo {
    fn effective_signature(&self) -> EffectiveSignature<'_> {
        EffectiveSignature {
            receiver_ty: self.receiver_ty,
            receiver: self.receiver.as_ref(),
            receiver_shape: self.receiver_shape.as_ref(),
            params: &self.params,
        }
    }
}

impl CtorSigInfo {
    fn effective_signature(&self) -> EffectiveSignature<'_> {
        EffectiveSignature {
            receiver_ty: None,
            receiver: None,
            receiver_shape: None,
            params: &self.params,
        }
    }
}

impl<'a> EffectiveSignature<'a> {
    /// Compare definition-time overload signature identity:
    /// receiver and effective parameter types only.
    fn is_equivalent_to(self, other: Self) -> bool {
        if self.receiver != other.receiver {
            return false;
        }
        if self.params.len() != other.params.len() {
            return false;
        }
        self.params
            .iter()
            .zip(other.params)
            .all(|(pa, pb)| pa.effective_ty == pb.effective_ty)
    }

    /// Return the first positional arity where trailing defaults
    /// make two signatures indistinguishable.
    fn first_ambiguous_positional_arity(self, other: Self) -> Option<usize> {
        if self.receiver != other.receiver {
            return None;
        }

        let min_self = min_positional_arity(self.params);
        let min_other = min_positional_arity(other.params);
        let max_k = self.params.len().min(other.params.len());

        for k in 0..=max_k {
            if k < min_self || k < min_other {
                continue;
            }
            if self.prefix_effective_types_equal(other, k) {
                // k == len == len is covered by full signature equivalence.
                if k == self.params.len() && k == other.params.len() {
                    continue;
                }
                return Some(k);
            }
        }
        None
    }

    fn first_vararg_non_vararg_overlap_arity(
        self,
        other: Self,
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> Option<usize> {
        match (self.vararg_index(), other.vararg_index()) {
            (Some(_), None) => self.first_overlap_arity_as_vararg(other, lower, builtins),
            (None, Some(_)) => other.first_overlap_arity_as_vararg(self, lower, builtins),
            _ => None,
        }
    }

    fn first_overlap_arity_as_vararg(
        self,
        non_vararg: Self,
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> Option<usize> {
        if !self.receivers_overlap(non_vararg, lower, builtins) {
            return None;
        }

        for arity in min_positional_arity(non_vararg.params)..=non_vararg.params.len() {
            if !self.vararg_accepts_positional_arity(arity) {
                continue;
            }
            if self.positional_types_overlap(non_vararg, arity, lower, builtins) {
                return Some(arity);
            }
        }

        None
    }

    fn vararg_index(self) -> Option<usize> {
        self.params.iter().position(|param| param.is_vararg)
    }

    fn vararg_accepts_positional_arity(self, arity: usize) -> bool {
        let Some(vararg_idx) = self.vararg_index() else {
            return false;
        };
        if arity >= vararg_idx {
            return true;
        }
        self.params[arity..vararg_idx]
            .iter()
            .all(|param| param.has_default)
    }

    fn positional_param_at(self, arg_idx: usize) -> Option<&'a ParamInfo> {
        match self.vararg_index() {
            Some(vararg_idx) if arg_idx >= vararg_idx => self.params.get(vararg_idx),
            _ => self.params.get(arg_idx),
        }
    }

    fn positional_types_overlap(
        self,
        other: Self,
        arity: usize,
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> bool {
        (0..arity).all(|arg_idx| {
            let Some(left) = self.positional_param_at(arg_idx) else {
                return false;
            };
            let Some(right) = other.positional_param_at(arg_idx) else {
                return false;
            };
            params_overlap(left, right, lower, builtins)
        })
    }

    fn receivers_overlap(
        self,
        other: Self,
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> bool {
        match (
            self.receiver_ty,
            self.receiver,
            other.receiver_ty,
            other.receiver,
        ) {
            (None, None, None, None) => true,
            (Some(left_ty), Some(left_eff), Some(right_ty), Some(right_eff)) => {
                effective_types_overlap(left_eff, right_eff)
                    || is_type_assignable(left_ty, right_ty, lower, builtins)
                    || is_type_assignable(right_ty, left_ty, lower, builtins)
            }
            _ => false,
        }
    }

    fn prefix_effective_types_equal(self, other: Self, k: usize) -> bool {
        self.params
            .iter()
            .take(k)
            .zip(other.params.iter().take(k))
            .all(|(pa, pb)| pa.effective_ty == pb.effective_ty)
    }

    fn has_method_type_param_shape(self) -> bool {
        self.receiver_shape.is_some_and(GenericShape::contains_hole)
            || self.params.iter().any(|param| param.shape.contains_hole())
    }

    fn has_generic_shape_mismatch(self, other: Self) -> bool {
        if !self.has_method_type_param_shape() && !other.has_method_type_param_shape() {
            return false;
        }
        // Shape mismatch is a refinement of signature-equivalence conflicts:
        // candidates with distinct effective signatures remain call-site candidates.
        if !self.is_equivalent_to(other) {
            return false;
        }

        let mut matcher = GenericShapeMatcher::default();
        match (self.receiver_shape, other.receiver_shape) {
            (Some(left), Some(right)) if !matcher.matches(left, right) => return true,
            (Some(_), None) | (None, Some(_)) => return false,
            _ => {}
        }

        self.params
            .iter()
            .zip(other.params)
            .any(|(left, right)| !matcher.matches(&left.shape, &right.shape))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TypeParamKey {
    name: String,
    decl_file: PathBuf,
    decl_span: Span,
}

impl TypeParamKey {
    fn from_ast(source: &SourceFile, param: &ast::TypeParam) -> Self {
        Self {
            name: source.slice(param.name.span).to_string(),
            decl_file: source.path().to_path_buf(),
            decl_span: param.name.span,
        }
    }

    fn from_type_param(param: &TypeParamType) -> Self {
        Self {
            name: param.name.clone(),
            decl_file: param.decl_file.clone(),
            decl_span: param.decl_span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EffectiveEffectRow {
    terms: Vec<EffectiveType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum EffectiveType {
    Any,
    String,
    Unit,
    Nothing,
    Bool,
    Char,
    Float64,
    Float32,
    Int,
    UInt,
    IntN(u16),
    UIntN(u16),
    Option(Box<EffectiveType>),
    Tuple(Vec<EffectiveType>),
    RefNominal {
        fqn: String,
        args: Vec<EffectiveType>,
        eff: Option<EffectiveEffectRow>,
    },
    ValueNominal {
        fqn: String,
        args: Vec<EffectiveType>,
        eff: Option<EffectiveEffectRow>,
    },
    Function {
        receiver: Option<Box<EffectiveType>>,
        params: Vec<EffectiveType>,
        return_ty: Box<EffectiveType>,
        effects: EffectiveEffectRow,
        effects_closed: bool,
    },
    Union(Vec<EffectiveType>),
    StarProjection(Box<EffectiveType>),
    Param(TypeParamKey),
    RefBound,
    ValueBound,
    Intersection(Vec<EffectiveType>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GenericEffectShape {
    terms: Vec<GenericShape>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum GenericShape {
    Closed(EffectiveType),
    Option(Box<GenericShape>),
    Tuple(Vec<GenericShape>),
    RefNominal {
        fqn: String,
        args: Vec<GenericShape>,
        eff: Option<GenericEffectShape>,
    },
    ValueNominal {
        fqn: String,
        args: Vec<GenericShape>,
        eff: Option<GenericEffectShape>,
    },
    Function {
        receiver: Option<Box<GenericShape>>,
        params: Vec<GenericShape>,
        return_ty: Box<GenericShape>,
        effects: GenericEffectShape,
        effects_closed: bool,
    },
    Union(Vec<GenericShape>),
    StarProjection(Box<GenericShape>),
    Hole(TypeParamKey),
}

#[derive(Debug, Clone)]
struct CtorDeclInfo {
    decl_file: PathBuf,
    span: Span,
    sig: CtorSigInfo,
}

struct CtorDeclAst<'a> {
    span: Span,
    type_params: &'a [ast::TypeParam],
    where_clause: Option<&'a ast::WhereClause>,
    params: &'a [ast::Param],
}

/// 检查当前文件中的重载声明是否存在“必然冲突”的情况。
///
/// 说明：
/// - 该 pass 需要在 `check_file_type_refs` 之后运行，确保签名里的类型引用都可 lowering；
/// - 当前实现只分析“位置实参 + 尾部默认参数省略”的可调用性。
pub fn check_file_overload_conflicts(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), OverloadDeclError> {
    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);

    let pkg_prefix = package_prefix(source, file.package.as_ref());

    let mut funs_by_fqn: HashMap<String, Vec<FunDeclInfo>> = HashMap::new();
    let mut ctors_by_type: HashMap<String, Vec<CtorDeclInfo>> = HashMap::new();

    collect_items(
        source,
        &file.items,
        &pkg_prefix,
        &mut lower,
        builtins,
        &mut funs_by_fqn,
        &mut ctors_by_type,
    )?;

    // 先检查函数 overload set。
    for (fqn, decls) in funs_by_fqn {
        check_fun_overload_set(&fqn, &decls, &lower, builtins)?;
    }

    // 再检查构造器 overload set（按宿主 type FQN 分组）。
    for (type_fqn, decls) in ctors_by_type {
        check_ctor_overload_set(&type_fqn, &decls, &lower, builtins)?;
    }

    Ok(())
}

fn collect_items(
    source: &SourceFile,
    items: &[ast::Item],
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    funs_by_fqn: &mut HashMap<String, Vec<FunDeclInfo>>,
    ctors_by_type: &mut HashMap<String, Vec<CtorDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    for item in items {
        match item {
            ast::Item::TypeAlias(_ta) => {}
            ast::Item::Fun(fun) => {
                collect_fun_decl(source, fun, prefix, lower, builtins, funs_by_fqn)?
            }
            ast::Item::ExtensionProperty(_p) => {}
            ast::Item::Val(_v) => {}
            ast::Item::Type(ty) => collect_type_decl(
                source,
                ty,
                prefix,
                lower,
                builtins,
                funs_by_fqn,
                ctors_by_type,
            )?,
            ast::Item::Object(obj) => collect_object_decl(
                source,
                obj,
                prefix,
                lower,
                builtins,
                funs_by_fqn,
                ctors_by_type,
            )?,
        }
    }
    Ok(())
}

fn collect_type_decl(
    source: &SourceFile,
    ty: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    funs_by_fqn: &mut HashMap<String, Vec<FunDeclInfo>>,
    ctors_by_type: &mut HashMap<String, Vec<CtorDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    let name = source.slice(ty.name.span).to_string();
    let type_fqn = join_prefix(prefix, &name);
    let is_annotation_class =
        ty.kind == ast::TypeKind::Class && ty.modifiers.contains(&ast::Modifier::Annotation);

    // class/type 的 type params 进入作用域，供 ctor/member signatures lowering。
    lower.push_type_params(&ty.type_params);
    let bounds = build_where_bound_entries(source, &ty.type_params, ty.where_clause.as_ref());
    let ty_where_bounds_pushed = if bounds.is_empty() {
        false
    } else {
        lower.push_where_bounds(bounds);
        true
    };
    let ty_eff_binding = if let Some(eff_param) = &ty.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
            None => crate::ty::EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    if !is_annotation_class && let Some(primary) = &ty.primary_ctor {
        collect_ctor_decl(
            source,
            &type_fqn,
            CtorDeclAst {
                span: primary.params_span,
                type_params: &[],
                where_clause: None,
                params: &primary.params,
            },
            lower,
            builtins,
            ctors_by_type,
        )?;
    }

    if let Some(body) = &ty.body {
        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(_v) => {}
                ast::TypeMember::Property(_p) => {}
                ast::TypeMember::InitBlock(_b) => {}
                ast::TypeMember::SecondaryCtor(ctor) => {
                    if is_annotation_class {
                        continue;
                    }
                    collect_ctor_decl(
                        source,
                        &type_fqn,
                        CtorDeclAst {
                            span: ctor.span,
                            type_params: &ctor.type_params,
                            where_clause: ctor.where_clause.as_ref(),
                            params: &ctor.params,
                        },
                        lower,
                        builtins,
                        ctors_by_type,
                    )?;
                }
                ast::TypeMember::Fun(fun) => {
                    if is_annotation_class {
                        continue;
                    }
                    collect_fun_decl(source, fun, &type_fqn, lower, builtins, funs_by_fqn)?
                }
                ast::TypeMember::Type(nested) => collect_type_decl(
                    source,
                    nested,
                    &type_fqn,
                    lower,
                    builtins,
                    funs_by_fqn,
                    ctors_by_type,
                )?,
                ast::TypeMember::Object(obj) => collect_object_decl(
                    source,
                    obj,
                    &type_fqn,
                    lower,
                    builtins,
                    funs_by_fqn,
                    ctors_by_type,
                )?,
            }
        }
    }

    if ty_eff_binding {
        lower.pop_effect_row_param_binding();
    }
    if ty_where_bounds_pushed {
        lower.pop_where_bounds();
    }
    lower.pop_type_params(&ty.type_params);

    Ok(())
}

fn collect_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    funs_by_fqn: &mut HashMap<String, Vec<FunDeclInfo>>,
    ctors_by_type: &mut HashMap<String, Vec<CtorDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    let obj_name = match &obj.name {
        Some(id) => source.slice(id.span).to_string(),
        None => {
            if !matches!(obj.kind, ast::ObjectKind::Companion) {
                // parser 会拒绝 `object { ... }`，这里作为防御性兜底。
                return Ok(());
            }
            "Companion".to_string()
        }
    };

    let obj_fqn = join_prefix(prefix, &obj_name);

    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::EnumVariant(_v) => {}
            ast::TypeMember::Property(_p) => {}
            ast::TypeMember::InitBlock(_b) => {}
            ast::TypeMember::SecondaryCtor(ctor) => {
                // object 内也允许 constructor 语法节点（与 class 共享 TypeBody 结构），
                // 但它在语义上无效；当前阶段忽略即可。
                let _ = ctor;
            }
            ast::TypeMember::Fun(fun) => {
                collect_fun_decl(source, fun, &obj_fqn, lower, builtins, funs_by_fqn)?
            }
            ast::TypeMember::Type(nested) => collect_type_decl(
                source,
                nested,
                &obj_fqn,
                lower,
                builtins,
                funs_by_fqn,
                ctors_by_type,
            )?,
            ast::TypeMember::Object(nested) => collect_object_decl(
                source,
                nested,
                &obj_fqn,
                lower,
                builtins,
                funs_by_fqn,
                ctors_by_type,
            )?,
        }
    }

    Ok(())
}

fn collect_ctor_decl(
    source: &SourceFile,
    type_fqn: &str,
    ctor: CtorDeclAst<'_>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    ctors_by_type: &mut HashMap<String, Vec<CtorDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    lower.push_type_params(ctor.type_params);
    let bounds = build_where_bound_entries(source, ctor.type_params, ctor.where_clause);
    let where_bounds_pushed = if bounds.is_empty() {
        false
    } else {
        lower.push_where_bounds(bounds);
        true
    };
    let result = (|| {
        let effective_bounds = collect_callable_type_param_effective_bounds(
            source,
            ctor.type_params,
            ctor.where_clause,
            lower,
            builtins,
        )?;
        lower_params(source, ctor.params, lower, &effective_bounds)
    })();
    if where_bounds_pushed {
        lower.pop_where_bounds();
    }
    lower.pop_type_params(ctor.type_params);
    let params = result?;
    ctors_by_type
        .entry(type_fqn.to_string())
        .or_default()
        .push(CtorDeclInfo {
            decl_file: source.path().to_path_buf(),
            span: ctor.span,
            sig: CtorSigInfo { params },
        });
    Ok(())
}

fn collect_fun_decl(
    source: &SourceFile,
    fun: &ast::FunDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    funs_by_fqn: &mut HashMap<String, Vec<FunDeclInfo>>,
) -> Result<(), OverloadDeclError> {
    let name = source.slice(fun.name.span).to_string();
    let fqn = join_prefix(prefix, &name);

    lower.push_type_params(&fun.type_params);
    let bounds = build_where_bound_entries(source, &fun.type_params, fun.where_clause.as_ref());
    let where_bounds_pushed = if bounds.is_empty() {
        false
    } else {
        lower.push_where_bounds(bounds);
        true
    };
    let fun_eff_binding = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
            None => crate::ty::EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    let effective_bounds = collect_callable_type_param_effective_bounds(
        source,
        &fun.type_params,
        fun.where_clause.as_ref(),
        lower,
        builtins,
    )?;

    let (receiver_ty, receiver, receiver_shape) = match &fun.receiver {
        Some(r) => {
            let ty = lower.lower_type_ref(r)?;
            (
                Some(ty),
                Some(effective_type_from_type_id(
                    ty,
                    lower.types(),
                    &effective_bounds,
                )),
                Some(generic_shape_from_type_id(
                    ty,
                    lower.types(),
                    &effective_bounds,
                )),
            )
        }
        None => (None, None, None),
    };
    let params = lower_params(source, &fun.params, lower, &effective_bounds)?;
    let return_ty = match &fun.return_ty {
        Some(ret) => Some(lower.lower_type_ref(ret)?),
        None => None,
    };
    let effects = lower.lower_effect_row_expr(fun.effects.as_ref())?;

    if fun_eff_binding {
        lower.pop_effect_row_param_binding();
    }
    if where_bounds_pushed {
        lower.pop_where_bounds();
    }
    lower.pop_type_params(&fun.type_params);

    funs_by_fqn
        .entry(fqn.clone())
        .or_default()
        .push(FunDeclInfo {
            decl_file: source.path().to_path_buf(),
            name_span: fun.name.span,
            sig: FunSigInfo {
                receiver_ty,
                receiver,
                receiver_shape,
                params,
                return_ty,
                effects,
            },
        });

    Ok(())
}

fn lower_params(
    source: &SourceFile,
    params: &[ast::Param],
    lower: &mut TypeLowering<'_>,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> Result<Vec<ParamInfo>, OverloadDeclError> {
    let mut out = Vec::with_capacity(params.len());
    for p in params {
        // `check_file_headers` 已要求参数必须带类型注解；这里作为健壮性兜底。
        let Some(ty) = &p.ty else {
            continue;
        };

        let id = lower.lower_type_ref(ty)?;
        let effective_ty = effective_type_from_type_id(id, lower.types(), effective_bounds);
        let shape = generic_shape_from_type_id(id, lower.types(), effective_bounds);
        let has_default = p.default_value.is_some();
        let is_vararg = p.is_vararg;

        // 这里保留 name 主要是为后续 named args 冲突分析做铺垫；
        // 当前最小实现不将 name 纳入冲突判定。
        let _name = source.slice(p.name.span);

        out.push(ParamInfo {
            ty: id,
            effective_ty,
            shape,
            has_default,
            is_vararg,
        });
    }
    Ok(out)
}

fn collect_callable_type_param_effective_bounds(
    source: &SourceFile,
    type_params: &[ast::TypeParam],
    where_clause: Option<&ast::WhereClause>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<HashMap<TypeParamKey, EffectiveType>, OverloadDeclError> {
    if type_params.is_empty() {
        return Ok(HashMap::new());
    }

    let mut key_by_name: HashMap<String, TypeParamKey> = HashMap::new();
    let mut raw_bounds: HashMap<TypeParamKey, Vec<LoweredGenericBound>> = HashMap::new();
    for param in type_params {
        let key = TypeParamKey::from_ast(source, param);
        key_by_name.insert(key.name.clone(), key.clone());
        raw_bounds.insert(key, Vec::new());
    }

    for constraint in ast::generic_constraints(type_params, where_clause) {
        let name = source.slice(constraint.ty_param.span);
        let Some(key) = key_by_name.get(name) else {
            continue;
        };
        let lowered = lower.lower_generic_bound(constraint.bound)?;
        raw_bounds.entry(key.clone()).or_default().push(lowered);
    }

    let mut effective_bounds: HashMap<TypeParamKey, EffectiveType> = raw_bounds
        .keys()
        .cloned()
        .map(|key| (key, EffectiveType::Any))
        .collect();

    // Bounds may refer to sibling method type parameters; iterate to a fixed
    // point so `T: U, U: Debug` makes both parameters effective as `Debug`.
    for _ in 0..=raw_bounds.len() {
        let mut changed = false;
        let mut next = effective_bounds.clone();
        for (key, bounds) in &raw_bounds {
            let effective = if bounds.is_empty() {
                effective_type_from_type_id(builtins.any, lower.types(), &effective_bounds)
            } else {
                canonical_effective_intersection(
                    bounds
                        .iter()
                        .copied()
                        .map(|bound| {
                            effective_type_from_lowered_bound(
                                bound,
                                lower.types(),
                                &effective_bounds,
                            )
                        })
                        .collect(),
                )
            };
            if next.get(key) != Some(&effective) {
                next.insert(key.clone(), effective);
                changed = true;
            }
        }
        effective_bounds = next;
        if !changed {
            break;
        }
    }

    Ok(effective_bounds)
}

fn effective_type_from_lowered_bound(
    bound: LoweredGenericBound,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> EffectiveType {
    match bound {
        LoweredGenericBound::Type(ty) => effective_type_from_type_id(ty, types, effective_bounds),
        LoweredGenericBound::Ref => EffectiveType::RefBound,
        LoweredGenericBound::Value => EffectiveType::ValueBound,
    }
}

fn effective_type_from_type_id(
    ty: TypeId,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> EffectiveType {
    match types.kind(ty) {
        TypeKind::Ref(kind) => effective_ref_type(kind, types, effective_bounds),
        TypeKind::Value(kind) => effective_value_type(kind, types, effective_bounds),
        TypeKind::StarProjection(star) => effective_star_projection(star, types, effective_bounds),
        TypeKind::Param(param) => {
            let key = TypeParamKey::from_type_param(param);
            effective_bounds
                .get(&key)
                .cloned()
                .unwrap_or(EffectiveType::Param(key))
        }
    }
}

fn effective_ref_type(
    kind: &RefTypeKind,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> EffectiveType {
    match kind {
        RefTypeKind::Any => EffectiveType::Any,
        RefTypeKind::String => EffectiveType::String,
        RefTypeKind::Nominal(nominal) => {
            let (args, eff) = effective_nominal_parts(nominal, types, effective_bounds);
            EffectiveType::RefNominal {
                fqn: nominal.fqn.clone(),
                args,
                eff,
            }
        }
        RefTypeKind::Function(fun) => effective_function_type(fun, types, effective_bounds),
        RefTypeKind::Union(union) => EffectiveType::Union(
            union
                .variants
                .iter()
                .copied()
                .map(|ty| effective_type_from_type_id(ty, types, effective_bounds))
                .collect(),
        ),
    }
}

fn effective_value_type(
    kind: &ValueTypeKind,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> EffectiveType {
    match kind {
        ValueTypeKind::Unit => EffectiveType::Unit,
        ValueTypeKind::Nothing => EffectiveType::Nothing,
        ValueTypeKind::Bool => EffectiveType::Bool,
        ValueTypeKind::Char => EffectiveType::Char,
        ValueTypeKind::Float64 => EffectiveType::Float64,
        ValueTypeKind::Float32 => EffectiveType::Float32,
        ValueTypeKind::Int => EffectiveType::Int,
        ValueTypeKind::UInt => EffectiveType::UInt,
        ValueTypeKind::IntN(bits) => EffectiveType::IntN(*bits),
        ValueTypeKind::UIntN(bits) => EffectiveType::UIntN(*bits),
        ValueTypeKind::Option(inner) => EffectiveType::Option(Box::new(
            effective_type_from_type_id(*inner, types, effective_bounds),
        )),
        ValueTypeKind::Tuple(elements) => EffectiveType::Tuple(
            elements
                .iter()
                .copied()
                .map(|ty| effective_type_from_type_id(ty, types, effective_bounds))
                .collect(),
        ),
        ValueTypeKind::Nominal(nominal) => {
            let (args, eff) = effective_nominal_parts(nominal, types, effective_bounds);
            EffectiveType::ValueNominal {
                fqn: nominal.fqn.clone(),
                args,
                eff,
            }
        }
    }
}

fn effective_nominal_parts(
    nominal: &NominalType,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> (Vec<EffectiveType>, Option<EffectiveEffectRow>) {
    let args = nominal
        .args
        .iter()
        .copied()
        .map(|ty| effective_type_from_type_id(ty, types, effective_bounds))
        .collect();
    let eff = nominal
        .eff
        .as_ref()
        .map(|row| effective_effect_row(row, types, effective_bounds));
    (args, eff)
}

fn effective_function_type(
    fun: &FunctionType,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> EffectiveType {
    EffectiveType::Function {
        receiver: fun
            .receiver
            .map(|ty| Box::new(effective_type_from_type_id(ty, types, effective_bounds))),
        params: fun
            .params
            .iter()
            .copied()
            .map(|ty| effective_type_from_type_id(ty, types, effective_bounds))
            .collect(),
        return_ty: Box::new(effective_type_from_type_id(
            fun.return_ty,
            types,
            effective_bounds,
        )),
        effects: effective_effect_row(&fun.effects, types, effective_bounds),
        effects_closed: fun.effects_closed,
    }
}

fn effective_star_projection(
    star: &StarProjectionType,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> EffectiveType {
    EffectiveType::StarProjection(Box::new(effective_type_from_type_id(
        star.read_ty,
        types,
        effective_bounds,
    )))
}

fn effective_effect_row(
    row: &EffectRow,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> EffectiveEffectRow {
    EffectiveEffectRow {
        terms: row
            .terms
            .iter()
            .copied()
            .map(|ty| effective_type_from_type_id(ty, types, effective_bounds))
            .collect(),
    }
}

fn generic_shape_from_type_id(
    ty: TypeId,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> GenericShape {
    match types.kind(ty) {
        TypeKind::Ref(kind) => generic_ref_shape(kind, types, effective_bounds),
        TypeKind::Value(kind) => generic_value_shape(kind, types, effective_bounds),
        TypeKind::StarProjection(star) => GenericShape::StarProjection(Box::new(
            generic_shape_from_type_id(star.read_ty, types, effective_bounds),
        )),
        TypeKind::Param(param) => {
            let key = TypeParamKey::from_type_param(param);
            if let Some(method_key) = method_type_param_key(&key, effective_bounds) {
                GenericShape::Hole(method_key)
            } else {
                GenericShape::Closed(effective_type_from_type_id(ty, types, effective_bounds))
            }
        }
    }
}

fn method_type_param_key(
    key: &TypeParamKey,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> Option<TypeParamKey> {
    effective_bounds
        .keys()
        .find(|method_key| *method_key == key)
        .or_else(|| {
            effective_bounds
                .keys()
                .find(|method_key| method_key.name == key.name)
        })
        .cloned()
}

fn generic_ref_shape(
    kind: &RefTypeKind,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> GenericShape {
    match kind {
        RefTypeKind::Any => GenericShape::Closed(EffectiveType::Any),
        RefTypeKind::String => GenericShape::Closed(EffectiveType::String),
        RefTypeKind::Nominal(nominal) => {
            let (args, eff) = generic_nominal_shape_parts(nominal, types, effective_bounds);
            GenericShape::RefNominal {
                fqn: nominal.fqn.clone(),
                args,
                eff,
            }
        }
        RefTypeKind::Function(fun) => generic_function_shape(fun, types, effective_bounds),
        RefTypeKind::Union(union) => GenericShape::Union(
            union
                .variants
                .iter()
                .copied()
                .map(|ty| generic_shape_from_type_id(ty, types, effective_bounds))
                .collect(),
        ),
    }
}

fn generic_value_shape(
    kind: &ValueTypeKind,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> GenericShape {
    match kind {
        ValueTypeKind::Option(inner) => GenericShape::Option(Box::new(generic_shape_from_type_id(
            *inner,
            types,
            effective_bounds,
        ))),
        ValueTypeKind::Tuple(elements) => GenericShape::Tuple(
            elements
                .iter()
                .copied()
                .map(|ty| generic_shape_from_type_id(ty, types, effective_bounds))
                .collect(),
        ),
        ValueTypeKind::Nominal(nominal) => {
            let (args, eff) = generic_nominal_shape_parts(nominal, types, effective_bounds);
            GenericShape::ValueNominal {
                fqn: nominal.fqn.clone(),
                args,
                eff,
            }
        }
        ValueTypeKind::Unit => GenericShape::Closed(EffectiveType::Unit),
        ValueTypeKind::Nothing => GenericShape::Closed(EffectiveType::Nothing),
        ValueTypeKind::Bool => GenericShape::Closed(EffectiveType::Bool),
        ValueTypeKind::Char => GenericShape::Closed(EffectiveType::Char),
        ValueTypeKind::Float64 => GenericShape::Closed(EffectiveType::Float64),
        ValueTypeKind::Float32 => GenericShape::Closed(EffectiveType::Float32),
        ValueTypeKind::Int => GenericShape::Closed(EffectiveType::Int),
        ValueTypeKind::UInt => GenericShape::Closed(EffectiveType::UInt),
        ValueTypeKind::IntN(bits) => GenericShape::Closed(EffectiveType::IntN(*bits)),
        ValueTypeKind::UIntN(bits) => GenericShape::Closed(EffectiveType::UIntN(*bits)),
    }
}

fn generic_nominal_shape_parts(
    nominal: &NominalType,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> (Vec<GenericShape>, Option<GenericEffectShape>) {
    let args = nominal
        .args
        .iter()
        .copied()
        .map(|ty| generic_shape_from_type_id(ty, types, effective_bounds))
        .collect();
    let eff = nominal
        .eff
        .as_ref()
        .map(|row| generic_effect_shape(row, types, effective_bounds));
    (args, eff)
}

fn generic_function_shape(
    fun: &FunctionType,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> GenericShape {
    GenericShape::Function {
        receiver: fun
            .receiver
            .map(|ty| Box::new(generic_shape_from_type_id(ty, types, effective_bounds))),
        params: fun
            .params
            .iter()
            .copied()
            .map(|ty| generic_shape_from_type_id(ty, types, effective_bounds))
            .collect(),
        return_ty: Box::new(generic_shape_from_type_id(
            fun.return_ty,
            types,
            effective_bounds,
        )),
        effects: generic_effect_shape(&fun.effects, types, effective_bounds),
        effects_closed: fun.effects_closed,
    }
}

fn generic_effect_shape(
    row: &EffectRow,
    types: &TypeStore,
    effective_bounds: &HashMap<TypeParamKey, EffectiveType>,
) -> GenericEffectShape {
    GenericEffectShape {
        terms: row
            .terms
            .iter()
            .copied()
            .map(|ty| generic_shape_from_type_id(ty, types, effective_bounds))
            .collect(),
    }
}

fn canonical_effective_intersection(mut terms: Vec<EffectiveType>) -> EffectiveType {
    let mut flat = Vec::new();
    for term in terms.drain(..) {
        match term {
            EffectiveType::Intersection(inner) => flat.extend(inner),
            other => flat.push(other),
        }
    }

    if flat.len() > 1 {
        flat.retain(|term| !matches!(term, EffectiveType::Any));
    }

    let mut unique = Vec::new();
    for term in flat {
        if !unique.contains(&term) {
            unique.push(term);
        }
    }

    match unique.len() {
        0 => EffectiveType::Any,
        1 => unique.pop().expect("one term"),
        _ => {
            unique.sort_by_key(EffectiveType::sort_key);
            EffectiveType::Intersection(unique)
        }
    }
}

impl GenericShape {
    fn contains_hole(&self) -> bool {
        match self {
            GenericShape::Closed(_) => false,
            GenericShape::Option(inner) | GenericShape::StarProjection(inner) => {
                inner.contains_hole()
            }
            GenericShape::Tuple(elements) | GenericShape::Union(elements) => {
                elements.iter().any(GenericShape::contains_hole)
            }
            GenericShape::RefNominal { args, eff, .. }
            | GenericShape::ValueNominal { args, eff, .. } => {
                args.iter().any(GenericShape::contains_hole)
                    || eff.as_ref().is_some_and(GenericEffectShape::contains_hole)
            }
            GenericShape::Function {
                receiver,
                params,
                return_ty,
                effects,
                ..
            } => {
                receiver
                    .as_ref()
                    .is_some_and(|receiver| receiver.contains_hole())
                    || params.iter().any(GenericShape::contains_hole)
                    || return_ty.contains_hole()
                    || effects.contains_hole()
            }
            GenericShape::Hole(_) => true,
        }
    }
}

impl GenericEffectShape {
    fn contains_hole(&self) -> bool {
        self.terms.iter().any(GenericShape::contains_hole)
    }
}

#[derive(Default)]
struct GenericShapeMatcher {
    left_to_right_holes: HashMap<TypeParamKey, TypeParamKey>,
    right_to_left_holes: HashMap<TypeParamKey, TypeParamKey>,
    left_to_closed: HashMap<TypeParamKey, GenericShape>,
    right_to_closed: HashMap<TypeParamKey, GenericShape>,
}

impl GenericShapeMatcher {
    fn matches(&mut self, left: &GenericShape, right: &GenericShape) -> bool {
        match (left, right) {
            (GenericShape::Hole(left_key), GenericShape::Hole(right_key)) => {
                self.match_holes(left_key, right_key)
            }
            (GenericShape::Hole(left_key), right) if !right.contains_hole() => {
                self.match_left_hole_to_closed(left_key, right)
            }
            (left, GenericShape::Hole(right_key)) if !left.contains_hole() => {
                self.match_right_hole_to_closed(right_key, left)
            }
            (GenericShape::Closed(left), GenericShape::Closed(right)) => left == right,
            (GenericShape::Option(left), GenericShape::Option(right))
            | (GenericShape::StarProjection(left), GenericShape::StarProjection(right)) => {
                self.matches(left, right)
            }
            (GenericShape::Tuple(left), GenericShape::Tuple(right))
            | (GenericShape::Union(left), GenericShape::Union(right)) => {
                self.match_shape_slices(left, right)
            }
            (
                GenericShape::RefNominal {
                    fqn: left_fqn,
                    args: left_args,
                    eff: left_eff,
                },
                GenericShape::RefNominal {
                    fqn: right_fqn,
                    args: right_args,
                    eff: right_eff,
                },
            )
            | (
                GenericShape::ValueNominal {
                    fqn: left_fqn,
                    args: left_args,
                    eff: left_eff,
                },
                GenericShape::ValueNominal {
                    fqn: right_fqn,
                    args: right_args,
                    eff: right_eff,
                },
            ) => {
                left_fqn == right_fqn
                    && self.match_shape_slices(left_args, right_args)
                    && self.match_effect_shapes(left_eff.as_ref(), right_eff.as_ref())
            }
            (
                GenericShape::Function {
                    receiver: left_receiver,
                    params: left_params,
                    return_ty: left_return,
                    effects: left_effects,
                    effects_closed: left_closed,
                },
                GenericShape::Function {
                    receiver: right_receiver,
                    params: right_params,
                    return_ty: right_return,
                    effects: right_effects,
                    effects_closed: right_closed,
                },
            ) => {
                left_closed == right_closed
                    && self
                        .match_optional_shape(left_receiver.as_deref(), right_receiver.as_deref())
                    && self.match_shape_slices(left_params, right_params)
                    && self.matches(left_return, right_return)
                    && self.match_effect_rows(left_effects, right_effects)
            }
            _ => false,
        }
    }

    fn match_holes(&mut self, left: &TypeParamKey, right: &TypeParamKey) -> bool {
        if self.left_to_closed.contains_key(left) || self.right_to_closed.contains_key(right) {
            return false;
        }

        match (
            self.left_to_right_holes.get(left),
            self.right_to_left_holes.get(right),
        ) {
            (Some(existing_right), Some(existing_left)) => {
                existing_right == right && existing_left == left
            }
            (Some(_), None) | (None, Some(_)) => false,
            (None, None) => {
                self.left_to_right_holes.insert(left.clone(), right.clone());
                self.right_to_left_holes.insert(right.clone(), left.clone());
                true
            }
        }
    }

    fn match_left_hole_to_closed(&mut self, left: &TypeParamKey, closed: &GenericShape) -> bool {
        if self.left_to_right_holes.contains_key(left) {
            return false;
        }
        match self.left_to_closed.get(left) {
            Some(existing) => existing == closed,
            None => {
                self.left_to_closed.insert(left.clone(), closed.clone());
                true
            }
        }
    }

    fn match_right_hole_to_closed(&mut self, right: &TypeParamKey, closed: &GenericShape) -> bool {
        if self.right_to_left_holes.contains_key(right) {
            return false;
        }
        match self.right_to_closed.get(right) {
            Some(existing) => existing == closed,
            None => {
                self.right_to_closed.insert(right.clone(), closed.clone());
                true
            }
        }
    }

    fn match_shape_slices(&mut self, left: &[GenericShape], right: &[GenericShape]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| self.matches(left, right))
    }

    fn match_optional_shape(
        &mut self,
        left: Option<&GenericShape>,
        right: Option<&GenericShape>,
    ) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => self.matches(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    fn match_effect_shapes(
        &mut self,
        left: Option<&GenericEffectShape>,
        right: Option<&GenericEffectShape>,
    ) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => self.match_effect_rows(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    fn match_effect_rows(&mut self, left: &GenericEffectShape, right: &GenericEffectShape) -> bool {
        self.match_shape_slices(&left.terms, &right.terms)
    }
}

impl EffectiveType {
    fn render(&self) -> String {
        match self {
            EffectiveType::Any => "Any".to_string(),
            EffectiveType::String => "String".to_string(),
            EffectiveType::Unit => "Unit".to_string(),
            EffectiveType::Nothing => "Nothing".to_string(),
            EffectiveType::Bool => "Bool".to_string(),
            EffectiveType::Char => "Char".to_string(),
            EffectiveType::Float64 => "Float64".to_string(),
            EffectiveType::Float32 => "Float32".to_string(),
            EffectiveType::Int => "Int".to_string(),
            EffectiveType::UInt => "UInt".to_string(),
            EffectiveType::IntN(bits) => format!("Int{bits}"),
            EffectiveType::UIntN(bits) => format!("UInt{bits}"),
            EffectiveType::Option(inner) => format!("{}?", inner.render()),
            EffectiveType::Tuple(elements) => {
                format!("({})", render_effective_types(elements))
            }
            EffectiveType::RefNominal { fqn, args, eff }
            | EffectiveType::ValueNominal { fqn, args, eff } => {
                let mut rendered = if args.is_empty() {
                    fqn.clone()
                } else {
                    format!("{fqn}<{}>", render_effective_types(args))
                };
                if let Some(row) = eff
                    && !row.terms.is_empty()
                {
                    rendered.push_str(" / eff ");
                    rendered.push_str(&row.render());
                }
                rendered
            }
            EffectiveType::Function {
                receiver,
                params,
                return_ty,
                effects,
                effects_closed,
            } => {
                let receiver = receiver
                    .as_ref()
                    .map(|ty| format!("{}.", ty.render()))
                    .unwrap_or_default();
                let bang = if *effects_closed { "!" } else { "" };
                format!(
                    "{}({}) -> {} / {}{}",
                    receiver,
                    render_effective_types(params),
                    return_ty.render(),
                    effects.render(),
                    bang
                )
            }
            EffectiveType::Union(variants) => variants
                .iter()
                .map(EffectiveType::render)
                .collect::<Vec<_>>()
                .join(" | "),
            EffectiveType::StarProjection(read_ty) => format!("*({})", read_ty.render()),
            EffectiveType::Param(param) => param.name.clone(),
            EffectiveType::RefBound => "ref".to_string(),
            EffectiveType::ValueBound => "value".to_string(),
            EffectiveType::Intersection(terms) => terms
                .iter()
                .map(EffectiveType::render)
                .collect::<Vec<_>>()
                .join(" & "),
        }
    }

    fn sort_key(&self) -> String {
        self.render()
    }
}

impl EffectiveEffectRow {
    fn render(&self) -> String {
        if self.terms.is_empty() {
            return "Pure".to_string();
        }
        self.terms
            .iter()
            .map(EffectiveType::render)
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

fn render_effective_types(types: &[EffectiveType]) -> String {
    types
        .iter()
        .map(EffectiveType::render)
        .collect::<Vec<_>>()
        .join(", ")
}

fn effective_types_overlap(left: &EffectiveType, right: &EffectiveType) -> bool {
    left == right
        || matches!(left, EffectiveType::Any)
        || matches!(right, EffectiveType::Any)
        || effective_intersections_overlap(left, right)
}

fn effective_intersections_overlap(left: &EffectiveType, right: &EffectiveType) -> bool {
    match (left, right) {
        (EffectiveType::Intersection(left_terms), _) => left_terms
            .iter()
            .all(|term| effective_types_overlap(term, right)),
        (_, EffectiveType::Intersection(right_terms)) => right_terms
            .iter()
            .all(|term| effective_types_overlap(left, term)),
        _ => false,
    }
}

fn params_overlap(
    left: &ParamInfo,
    right: &ParamInfo,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    effective_types_overlap(&left.effective_ty, &right.effective_ty)
        || is_type_assignable(left.ty, right.ty, lower, builtins)
        || is_type_assignable(right.ty, left.ty, lower, builtins)
}

fn render_fun_signature(fqn: &str, sig: &FunSigInfo) -> String {
    let params = render_param_list(&sig.params);
    match &sig.receiver {
        Some(receiver) => format!("{}.{fqn}({params})", receiver.render()),
        None => format!("{fqn}({params})"),
    }
}

fn render_ctor_signature(type_fqn: &str, sig: &CtorSigInfo) -> String {
    format!("{type_fqn}.<init>({})", render_param_list(&sig.params))
}

fn render_candidate(signature: String, location: &str) -> Box<str> {
    format!("{signature} @ {location}").into_boxed_str()
}

fn format_decl_location(lower: &TypeLowering<'_>, decl_file: &Path, span: Span) -> String {
    let Some(source) = lower.env().source(decl_file) else {
        return decl_file.display().to_string();
    };
    let Ok((line, col)) = source.offset_to_line_col(span.start) else {
        return decl_file.display().to_string();
    };
    format!("{}:{line}:{col}", decl_file.display())
}

fn render_param_list(params: &[ParamInfo]) -> String {
    params
        .iter()
        .map(|param| {
            let mut rendered = param.effective_ty.render();
            if param.is_vararg {
                rendered.push('*');
            }
            if param.has_default {
                rendered.push_str(" = ...");
            }
            rendered
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn check_fun_overload_set(
    fqn: &str,
    decls: &[FunDeclInfo],
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), OverloadDeclError> {
    if decls.len() <= 1 {
        return Ok(());
    }

    // 以源码顺序稳定排序，便于错误稳定复现。
    let mut decls = decls.to_vec();
    decls.sort_by_key(|d| d.name_span.start);

    for i in 0..decls.len() {
        for j in (i + 1)..decls.len() {
            let a = &decls[i];
            let b = &decls[j];
            let a_sig = a.sig.effective_signature();
            let b_sig = b.sig.effective_signature();
            let a_location = format_decl_location(lower, &a.decl_file, a.name_span);
            let b_location = format_decl_location(lower, &b.decl_file, b.name_span);

            if let Some(arity) = a_sig.first_vararg_non_vararg_overlap_arity(b_sig, lower, builtins)
            {
                return Err(OverloadDeclError::VarargOverlapsNonVararg {
                    fqn: fqn.to_string().into_boxed_str(),
                    arity,
                    previous_candidate: render_candidate(
                        render_fun_signature(fqn, &a.sig),
                        &a_location,
                    ),
                    conflict_candidate: render_candidate(
                        render_fun_signature(fqn, &b.sig),
                        &b_location,
                    ),
                    conflict: b.name_span.into(),
                    previous: a.name_span.into(),
                });
            }

            if a_sig.has_generic_shape_mismatch(b_sig) {
                return Err(OverloadDeclError::GenericShapeMismatch {
                    fqn: fqn.to_string().into_boxed_str(),
                    previous_candidate: render_candidate(
                        render_fun_signature(fqn, &a.sig),
                        &a_location,
                    ),
                    conflict_candidate: render_candidate(
                        render_fun_signature(fqn, &b.sig),
                        &b_location,
                    ),
                    conflict: b.name_span.into(),
                    previous: a.name_span.into(),
                });
            }

            if a_sig.is_equivalent_to(b_sig) {
                let reason = match (a.sig.return_ty, b.sig.return_ty) {
                    (Some(ra), Some(rb)) if ra != rb => {
                        "仅返回类型不同（返回类型不参与重载决议）".to_string()
                    }
                    _ if a.sig.effects != b.sig.effects => {
                        "仅 effect row 不同（effect row 不参与重载决议）".to_string()
                    }
                    _ => "重复或不可区分的签名".to_string(),
                };
                return Err(OverloadDeclError::Conflict {
                    fqn: fqn.to_string().into_boxed_str(),
                    reason: reason.into_boxed_str(),
                    previous_candidate: render_candidate(
                        render_fun_signature(fqn, &a.sig),
                        &a_location,
                    ),
                    conflict_candidate: render_candidate(
                        render_fun_signature(fqn, &b.sig),
                        &b_location,
                    ),
                    conflict: b.name_span.into(),
                    previous: a.name_span.into(),
                });
            }

            if let Some(arity) = a_sig.first_ambiguous_positional_arity(b_sig) {
                let reason = format!("默认参数导致在提供 {arity} 个实参时不可区分（位置调用）");
                return Err(OverloadDeclError::Conflict {
                    fqn: fqn.to_string().into_boxed_str(),
                    reason: reason.into_boxed_str(),
                    previous_candidate: render_candidate(
                        render_fun_signature(fqn, &a.sig),
                        &a_location,
                    ),
                    conflict_candidate: render_candidate(
                        render_fun_signature(fqn, &b.sig),
                        &b_location,
                    ),
                    conflict: b.name_span.into(),
                    previous: a.name_span.into(),
                });
            }
        }
    }

    Ok(())
}

fn check_ctor_overload_set(
    type_fqn: &str,
    decls: &[CtorDeclInfo],
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), OverloadDeclError> {
    if decls.len() <= 1 {
        return Ok(());
    }

    let mut decls = decls.to_vec();
    decls.sort_by_key(|d| d.span.start);

    for i in 0..decls.len() {
        for j in (i + 1)..decls.len() {
            let a = &decls[i];
            let b = &decls[j];
            let a_sig = a.sig.effective_signature();
            let b_sig = b.sig.effective_signature();
            let a_location = format_decl_location(lower, &a.decl_file, a.span);
            let b_location = format_decl_location(lower, &b.decl_file, b.span);

            if let Some(arity) = a_sig.first_vararg_non_vararg_overlap_arity(b_sig, lower, builtins)
            {
                return Err(OverloadDeclError::VarargOverlapsNonVararg {
                    fqn: format!("{type_fqn}.<init>").into_boxed_str(),
                    arity,
                    previous_candidate: render_candidate(
                        render_ctor_signature(type_fqn, &a.sig),
                        &a_location,
                    ),
                    conflict_candidate: render_candidate(
                        render_ctor_signature(type_fqn, &b.sig),
                        &b_location,
                    ),
                    conflict: b.span.into(),
                    previous: a.span.into(),
                });
            }

            if a_sig.has_generic_shape_mismatch(b_sig) {
                return Err(OverloadDeclError::GenericShapeMismatch {
                    fqn: format!("{type_fqn}.<init>").into_boxed_str(),
                    previous_candidate: render_candidate(
                        render_ctor_signature(type_fqn, &a.sig),
                        &a_location,
                    ),
                    conflict_candidate: render_candidate(
                        render_ctor_signature(type_fqn, &b.sig),
                        &b_location,
                    ),
                    conflict: b.span.into(),
                    previous: a.span.into(),
                });
            }

            if a_sig.is_equivalent_to(b_sig) {
                return Err(OverloadDeclError::Conflict {
                    fqn: format!("{type_fqn}.<init>").into_boxed_str(),
                    reason: "重复或不可区分的构造器签名".into(),
                    previous_candidate: render_candidate(
                        render_ctor_signature(type_fqn, &a.sig),
                        &a_location,
                    ),
                    conflict_candidate: render_candidate(
                        render_ctor_signature(type_fqn, &b.sig),
                        &b_location,
                    ),
                    conflict: b.span.into(),
                    previous: a.span.into(),
                });
            }

            if let Some(arity) = a_sig.first_ambiguous_positional_arity(b_sig) {
                let reason = format!("默认参数导致在提供 {arity} 个实参时不可区分（位置调用）");
                return Err(OverloadDeclError::Conflict {
                    fqn: format!("{type_fqn}.<init>").into_boxed_str(),
                    reason: reason.into_boxed_str(),
                    previous_candidate: render_candidate(
                        render_ctor_signature(type_fqn, &a.sig),
                        &a_location,
                    ),
                    conflict_candidate: render_candidate(
                        render_ctor_signature(type_fqn, &b.sig),
                        &b_location,
                    ),
                    conflict: b.span.into(),
                    previous: a.span.into(),
                });
            }
        }
    }

    Ok(())
}

/// 位置调用下可省略的参数只来自“尾部默认参数”：
/// - `f(x:Int, y:Int=0)` 可用 `f(1)`
/// - `f(x:Int=0, y:Int)` 不能用 `f()`（需要 named args 支持才能省略中间参数）
fn min_positional_arity(params: &[ParamInfo]) -> usize {
    let mut trailing_defaults = 0usize;
    for p in params.iter().rev() {
        if p.has_default {
            trailing_defaults += 1;
        } else {
            break;
        }
    }
    params.len().saturating_sub(trailing_defaults)
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}
