//! `TypeRef` → `Type` lowering（T0403）。
//!
//! 当前阶段的目标：
//! - 把 parser 产出的 AST `TypeRef` 转换为编译器内部类型表示（`ty::TypeId`）
//! - 在 lowering 过程中做最小语义校验：类型存在性（应由 resolve 保证）与泛型 arity 检查
//! - 覆盖 `Path` / `Tuple` / `Nullable` / `Function`，其它类型语法在后续任务逐步补齐

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::monomorph::{MonomorphKey, MonomorphRequest, MonomorphSymbol};
use crate::resolve::{ConstructorKind, ImportTable, Index, Visibility};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore,
    ValueTypeKind,
};
use crate::warnings::{self, CompileWarning};

use super::assignable::is_type_assignable;
use super::builtin_annotations::{BuiltinAnnotationFlags, collect_file_warning_suppressions};
use super::{TypeEnv, TypeSymbol, TypeSymbolKind};

const CLAYOUT_FQN: &str = "scoop.core.CLayout";
const PTR_FQN: &str = "scoop.unsafe.Ptr";
const FUNPTR_FQN: &str = "scoop.unsafe.FunPtr";
const CONTINUATION_ANSWER_HOLE_DECL_FILE: &str = "<continuation-answer-hole>";

#[derive(Debug, Clone)]
pub(crate) struct StructDirectFieldInfo {
    pub name: String,
    pub ty: TypeId,
    pub has_default: bool,
}

#[derive(Debug, Error, Diagnostic)]
pub enum TypeLowerError {
    #[error("未解析的类型：{name}")]
    #[diagnostic(code(scoop::typecheck::unresolved_type))]
    UnresolvedType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型参数数量不匹配：{name} 期望 {expected} 个，但提供了 {found} 个")]
    #[diagnostic(code(scoop::typecheck::type_arity_mismatch))]
    TypeArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "注解类 `{name}` 只允许用于注解位置（`@Name(...)`）或其它 annotation class 的 payload 类型，不能作为普通类型使用，也不能在运行期构造实例"
    )]
    #[diagnostic(code(scoop::typecheck::annotation_type_runtime_use_not_allowed))]
    AnnotationTypeRuntimeUseNotAllowed {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error(
        "legacy `Continuation<Resume, eff E>` 简写已移除；请显式写出 answer type：`Continuation<Resume, Answer, eff E>`"
    )]
    #[diagnostic(code(scoop::typecheck::continuation_legacy_effect_shorthand_removed))]
    ContinuationLegacyEffectShorthandRemoved {
        #[label("这里需要显式 answer type")]
        span: miette::SourceSpan,
    },

    #[error(
        "legacy `Continuation<Resume>` 简写已移除；请显式写出 answer type：`Continuation<Resume, Answer>`"
    )]
    #[diagnostic(code(scoop::typecheck::continuation_legacy_pure_shorthand_removed))]
    ContinuationLegacyPureShorthandRemoved {
        #[label("这里需要显式 answer type")]
        span: miette::SourceSpan,
    },

    #[error(
        "当前语言 contract 下，只有显式声明 effect row 形参的名义类型才允许 use-site effect row 实参（`eff ...`）；{name} 不满足该条件"
    )]
    #[diagnostic(code(scoop::typecheck::use_site_eff_arg_not_allowed))]
    UseSiteEffectRowArgNotAllowed {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的类型语法：{kind}")]
    #[diagnostic(code(scoop::typecheck::unsupported_type_ref))]
    UnsupportedTypeRef {
        kind: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型环境缺少符号：{fqn}")]
    #[diagnostic(code(scoop::typecheck::missing_type_symbol_in_env))]
    MissingTypeSymbolInEnv {
        fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("effect row 项必须是 effect 类型：{item}（得到 {found}）")]
    #[diagnostic(code(scoop::typecheck::effect_row_item_not_effect))]
    EffectRowItemNotEffect {
        item: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    /// 闭合 effect row（`E!`）不允许包含 row 变量（例如函数级 `<eff E>` 的 `E`）。
    ///
    /// 说明：当前阶段闭合 row 的主要用途是 program boundary（例如 `Pure!`）。
    /// 把 row 变量放进闭合 row 会让“闭合”的语义在调用点被重新打开，导致边界规则难以保持直观。
    #[error("闭合 effect row 不允许引用 row 变量：{name}")]
    #[diagnostic(code(scoop::typecheck::closed_effect_row_contains_row_var))]
    ClosedEffectRowContainsRowVar {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    /// 循环 typealias（直接或间接）：例如 `typealias A = B; typealias B = A`。
    #[error("循环的类型别名：{cycle}")]
    #[diagnostic(code(scoop::typecheck::cyclic_type_alias))]
    CyclicTypeAlias {
        cycle: String,
        #[label("别名声明在这里")]
        first: miette::SourceSpan,
        #[label("并且通过其它别名又回到这里")]
        second: miette::SourceSpan,
    },

    /// 声明处变型（`in`/`out`）位置规则违规（spec §3.2 / Appendix B.4）。
    #[error("变型位置不合法：类型参数 {param} 声明为 {declared}，但在 {position} 位置使用")]
    #[diagnostic(code(scoop::typecheck::variance_position_violation))]
    VariancePositionViolation {
        param: String,
        declared: &'static str,
        position: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    /// `where` 子句约束不满足（T0458）。
    #[error("泛型约束不满足：{type_fqn} 的类型实参 {arg} 不满足 {param} : {bound}")]
    #[diagnostic(code(scoop::typecheck::where_constraint_not_satisfied))]
    WhereConstraintNotSatisfied {
        type_fqn: String,
        param: String,
        arg: String,
        bound: String,
        #[label("这里的类型实参不满足 where 约束")]
        span: miette::SourceSpan,
    },

    #[error("`Ptr<T>` 的类型实参必须是 GC-free 值类型（不允许直接/间接包含 GC 引用）：{found}")]
    #[diagnostic(code(scoop::typecheck::ptr_pointee_must_be_gc_free))]
    PtrPointeeMustBeGcFree {
        found: String,
        #[label("这里的 T 不是 GC-free 值类型")]
        span: miette::SourceSpan,
    },

    #[error("`FunPtr<F>` 的类型实参必须是函数类型（例如 `(Int, Int) -> Int`），但得到 {found}")]
    #[diagnostic(code(scoop::typecheck::funptr_signature_must_be_function))]
    FunPtrSignatureMustBeFunction {
        found: String,
        #[label("这里的 F 不是函数类型")]
        span: miette::SourceSpan,
    },

    #[error(
        "`FunPtr<F>` 的类型实参必须是无 effect 的函数类型（省略 effect row、`/ Pure` 或 `/ Pure!`），但得到 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::funptr_signature_must_be_pure))]
    FunPtrSignatureMustBePure {
        found: String,
        #[label("这里的 F 不是无 effect 函数类型")]
        span: miette::SourceSpan,
    },

    #[error(
        "`FunPtr<F>` 的函数签名只接受当前 native ABI value surface：标量、`UIntPtr`、`Ptr<T>`、纯 `FunPtr<F>` token、tuple，以及 `@CLayout` struct；不接受 {found}"
    )]
    #[diagnostic(code(scoop::typecheck::funptr_signature_not_supported_by_native_abi))]
    FunPtrSignatureNotSupportedByNativeAbi {
        found: String,
        #[label("这里的类型不在当前 native ABI contract 中")]
        span: miette::SourceSpan,
    },

    #[error("value-only enum 的底层类型必须是整型标量：{enum_name} 的底层类型为 {found}")]
    #[diagnostic(code(scoop::typecheck::value_only_enum_underlying_not_integral))]
    ValueOnlyEnumUnderlyingNotIntegral {
        enum_name: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("value-only enum 的 variant 不允许声明字段：{variant_name}")]
    #[diagnostic(code(scoop::typecheck::value_only_enum_variant_fields_not_allowed))]
    ValueOnlyEnumVariantFieldsNotAllowed {
        variant_name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("value-only enum 的 variant 必须显式指定判别值：{variant_name}")]
    #[diagnostic(code(scoop::typecheck::value_only_enum_variant_missing_discriminant))]
    ValueOnlyEnumVariantMissingDiscriminant {
        variant_name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("value-only enum 的判别值必须是整型常量：{variant_name}")]
    #[diagnostic(code(scoop::typecheck::value_only_enum_variant_discriminant_not_int_const))]
    ValueOnlyEnumVariantDiscriminantNotIntConst {
        variant_name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

/// 泛型名义类型实例化的稳定 key（T1109：用于 `.cone` pre-specialize 类型实例命中计数）。
///
/// 说明：
/// - 该 key 使用 `(type_fqn, type_args)` 表示一个具体的 use-site 实例；
/// - 它当前只服务于 type-alias cache 与 `.cone` 预特化命中统计，因此仍只跟踪普通 type args；
///   nominal type 的 use-site effect row identity 继续保存在 `NominalType::eff` 中。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeInstantiationKey {
    pub fqn: String,
    pub type_args: Vec<TypeId>,
}

/// 对一个文件内出现的所有 “type position” 的 `TypeRef` 执行 lowering 并做最小校验。
///
/// 说明：
/// - 该函数是早期 typecheck phase 的一块可独立回归的最小能力（fixtures 会直接调用）；
/// - 当前只走声明头（fun/val/type/typealias）的类型引用，不进入函数体的表达式类型检查。
pub fn check_file_type_refs(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), TypeLowerError> {
    let mut ctx = TypeLowering::new(source, file, index, imports, env, types, builtins);

    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                // typealias 的 RHS 允许引用其自身的 type params（例如 `typealias Handler<T> = (T) -> Unit`）。
                ctx.push_type_params(&ta.type_params);
                let _ = ctx.lower_type_ref(&ta.ty)?;
                ctx.pop_type_params(&ta.type_params);
            }
            ast::Item::Fun(fun) => {
                ctx.push_type_params(&fun.type_params);
                let eff_binding = if let Some(eff_param) = &fun.eff_param {
                    let name = source.slice(eff_param.name.span).to_string();
                    let default = match eff_param.default.as_ref() {
                        Some(expr) => ctx.lower_effect_row_expr(Some(expr))?,
                        None => EffectRow::pure(),
                    };
                    ctx.push_effect_row_param_binding(name.clone(), default);
                    Some(name)
                } else {
                    None
                };
                if let Some(receiver) = &fun.receiver {
                    let _ = ctx.lower_type_ref(receiver)?;
                }
                for p in &fun.params {
                    if let Some(ty) = &p.ty {
                        let _ = ctx.lower_type_ref(ty)?;
                    }
                }
                if let Some(ret) = &fun.return_ty {
                    let _ = ctx.lower_type_ref(ret)?;
                }
                // T0458：`where` 子句中的 bound 同样属于 type position，需要参与 lowering。
                if let Some(w) = &fun.where_clause {
                    for c in &w.constraints {
                        let _ = ctx.lower_type_ref(&c.bound)?;
                    }
                }
                ctx.pop_type_params(&fun.type_params);
                if eff_binding.is_some() {
                    ctx.pop_effect_row_param_binding();
                }
            }
            ast::Item::ExtensionProperty(p) => {
                ctx.push_type_params(&p.type_params);
                let _ = ctx.lower_type_ref(&p.receiver)?;
                if let Some(ty) = &p.ty {
                    let _ = ctx.lower_type_ref(ty)?;
                }
                ctx.pop_type_params(&p.type_params);
            }
            ast::Item::Val(v) => {
                if let Some(ty) = &v.ty {
                    let _ = ctx.lower_type_ref(ty)?;
                }
            }
            ast::Item::Type(ty) => {
                ctx.check_type_decl_headers(ty)?;
            }
            ast::Item::Object(obj) => {
                ctx.check_object_decl_headers(obj)?;
            }
            // T1220a：package-level comptime if 在进入 typecheck 之前应被裁剪（TODO T1220b）。
            // 这里先忽略，避免在尚未接入裁剪逻辑前引入非预期崩溃。
            ast::Item::ComptimeIf(_) => {}
        }
    }

    Ok(())
}

/// 对一个文件内出现的所有 “type position” 的 `TypeRef` 执行 lowering，并返回“泛型类型实例化”的集合（T1109）。
pub fn check_file_type_refs_with_type_instantiation_keys(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<Vec<TypeInstantiationKey>, TypeLowerError> {
    let mut ctx = TypeLowering::new(source, file, index, imports, env, types, builtins);
    ctx.enable_type_instantiation_collection();

    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                let _ = ctx.lower_type_ref(&ta.ty)?;
            }
            ast::Item::Fun(fun) => {
                ctx.push_type_params(&fun.type_params);
                let eff_binding = if let Some(eff_param) = &fun.eff_param {
                    let name = source.slice(eff_param.name.span).to_string();
                    let default = match eff_param.default.as_ref() {
                        Some(expr) => ctx.lower_effect_row_expr(Some(expr))?,
                        None => EffectRow::pure(),
                    };
                    ctx.push_effect_row_param_binding(name.clone(), default);
                    Some(name)
                } else {
                    None
                };
                if let Some(receiver) = &fun.receiver {
                    let _ = ctx.lower_type_ref(receiver)?;
                }
                for p in &fun.params {
                    if let Some(ty) = &p.ty {
                        let _ = ctx.lower_type_ref(ty)?;
                    }
                }
                if let Some(ret) = &fun.return_ty {
                    let _ = ctx.lower_type_ref(ret)?;
                }
                if let Some(w) = &fun.where_clause {
                    for c in &w.constraints {
                        let _ = ctx.lower_type_ref(&c.bound)?;
                    }
                }
                ctx.pop_type_params(&fun.type_params);
                if eff_binding.is_some() {
                    ctx.pop_effect_row_param_binding();
                }
            }
            ast::Item::ExtensionProperty(p) => {
                ctx.push_type_params(&p.type_params);
                let _ = ctx.lower_type_ref(&p.receiver)?;
                if let Some(ty) = &p.ty {
                    let _ = ctx.lower_type_ref(ty)?;
                }
                ctx.pop_type_params(&p.type_params);
            }
            ast::Item::Val(v) => {
                if let Some(ty) = &v.ty {
                    let _ = ctx.lower_type_ref(ty)?;
                }
            }
            ast::Item::Type(ty) => {
                ctx.check_type_decl_headers(ty)?;
            }
            ast::Item::Object(obj) => {
                ctx.check_object_decl_headers(obj)?;
            }
            // T1220a：package-level comptime if 在进入 typecheck 之前应被裁剪（TODO T1220b）。
            ast::Item::ComptimeIf(_) => {}
        }
    }

    Ok(ctx.take_type_instantiation_keys())
}

/// `where T: Bound` 约束在 TypeLowering 中的运行时表示（T0130）。
///
/// 当进入泛型函数/类型声明的 body 时，把 where 子句的约束 push 到 `where_bound_scopes`；
/// 当 receiver 为 `TypeKind::Param` 时，查找该 param 的 bound 接口方法以驱动方法分发。
#[derive(Debug, Clone)]
pub(super) struct WhereBoundEntry {
    /// 被约束的 type param 名称（如 `T`）。
    pub param_name: String,
    /// bound 右侧的 type ref（如 `Show`）——在查找时再 lower，以便在正确的上下文中解析。
    pub bound: ast::TypeRef,
    /// 声明处文件（用于在正确的 source/package/import 上下文中 lower bound type ref）。
    pub decl_file: std::path::PathBuf,
}

pub(crate) struct TypeLowering<'a> {
    source: &'a SourceFile,
    index: &'a Index,
    imports: ImportTable,
    env: &'a TypeEnv,
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    pkg_prefix: String,
    /// 顶层 `val/var` 的可变性表（FQN → 是否为 `var`）。
    ///
    /// 说明：
    /// - 用于表达式 typecheck 阶段对 `x = ...`（lhs 为顶层符号）做最小“可写性”门禁；
    /// - 当前阶段该表只覆盖“创建该 TypeLowering 时传入的 file”的顶层声明（与 `top_level_types` 一致）。
    top_level_value_mutabilities: HashMap<String, bool>,
    /// Top-level `@Extern` value FQNs visible to this file.
    extern_global_fqns: HashSet<String>,
    /// `@Extern(..., abi = "scoop")` 函数声明位点（decl_file + name span）。
    extern_scoop_fun_decls: HashSet<(PathBuf, Span)>,
    /// type parameter 作用域栈：用于 lowering `T` 这类抽象类型引用。
    type_param_scopes: Vec<HashMap<String, TypeId>>,
    /// effect row parameter 作用域栈：用于 lowering `/ E` 这类 row 变量引用（T0509）。
    effect_row_param_scopes: Vec<HashMap<String, EffectRow>>,
    /// typealias 展开栈（用于循环检测；存储 alias FQN）。
    type_alias_stack: Vec<String>,
    /// 已展开 typealias 的缓存（(alias FQN + type args) → lowered TypeId）。
    ///
    /// 说明：
    /// - 非泛型 alias：`type_args` 为空；
    /// - 泛型 alias：同一个 alias 在不同 type args 下展开结果不同，因此 cache key 必须包含 type args。
    type_alias_cache: HashMap<TypeInstantiationKey, TypeId>,
    /// 当前是否处于“required effects 收集”模式（T0604）。
    ///
    /// 说明：只在检查函数体时启用；其它 typecheck phase 默认关闭，避免改变现有行为。
    effect_collection_enabled: bool,
    /// 是否允许发出 user-visible warning。
    ///
    /// 说明：
    /// - 默认开启：真实 use-site / declaration lowering 会保留 warning；
    /// - 跨文件辅助收集（例如为当前文件构建其它文件的签名/字段 side tables）会临时关闭，
    ///   避免 unrelated warnings 污染当前文件的诊断输出。
    warning_emission_enabled: bool,
    /// effect 收集的“抑制深度”：
    /// - 进入 lambda body 时会暂时抑制（lambda 的 effect 属于函数值本身，而非外层函数立即执行的效果）。
    /// - 未来若引入更多“非立即执行”的语境（例如 `const`/comptime），同样可复用该机制。
    effect_collection_suspend_depth: usize,
    /// 记录（effect TypeId, perform span）。
    performed_effects: Vec<(TypeId, Span)>,
    /// 当前文件中“表达式 span -> 推导后的 TypeId”。
    ///
    /// 用途：
    /// - typecheck 成功后写回到 `ast::File` 的 side table；
    /// - HIR lowering 读取该表，以恢复 `return [..]` / `x = [..]` / 无注解 `val x = [..]`
    ///   这类表达式位置的最终类型。
    inferred_expr_tys: HashMap<Span, TypeId>,
    /// 当前文件中“局部绑定 span -> 推导后的 TypeId”。
    ///
    /// 用途：
    /// - 保存 `handle` arm binder 这类不一定有显式类型注解、但 HIR/codegen 仍需要真实类型的绑定；
    /// - typecheck 成功后写回到 `ast::File` 的 side table，供 HIR lowering 复用。
    inferred_binding_tys: HashMap<Span, TypeId>,
    /// 当前文件中“函数声明 name span -> 推导后的内部返回 TypeId”。
    ///
    /// 用途：
    /// - 保存未显式声明返回类型的函数/成员函数的推断结果；
    /// - typecheck 成功后写回到 `ast::File` 的 side table，供 HIR lowering 复用。
    inferred_fun_return_tys: HashMap<Span, TypeId>,
    /// 当前文件中“perform span -> performed effect 实例 TypeId”。
    ///
    /// 用途：
    /// - 供 HIR lowering / LLVM effect codegen 读取 direct perform 等语义点的真实 effect 实例；
    /// - 与 `performed_effects` 不同：这里是稳定 side table，而不是当前函数体的临时 required-effects 收集缓冲。
    inferred_performed_effect_tys: HashMap<Span, TypeId>,
    /// 当前文件中“handle arm op span -> handled effect 实例 TypeId”。
    ///
    /// 用途：
    /// - parser 当前不支持显式 `Effect<T>.op(...)` handler head；
    /// - 因此 lowering/codegen 必须读取 typecheck 推导出的 handled effect 实例，而不是只看语法层路径。
    inferred_handle_arm_effect_tys: HashMap<Span, TypeId>,
    /// 当前文件中“handle arm op span -> 最终实例化后的 op type args”。
    ///
    /// 用途：
    /// - `Effect.op<T>(...)` 的 concrete op type args 不能在 HIR/MIR/effect-facts 阶段重新猜；
    /// - 这里直接保留 typecheck 最终实例化结果，供后续阶段稳定复用。
    inferred_handle_arm_op_type_args: HashMap<Span, Vec<TypeId>>,
    /// `receiver?.member` 在 typecheck 阶段补做出的成员解析结果。
    ///
    /// 用途：
    /// - resolver 无法仅凭 nullable receiver 的语法形态写回 `member.resolved`；
    /// - lowering/codegen 仍需要稳定的成员解析结果，因此这里按 `member.span` 记录。
    safe_member_access_resolutions: HashMap<Span, ast::ResolvedMemberRef>,
    /// typecheck 最终确认的“member span -> 成员解析结果”。
    ///
    /// 用途：
    /// - 普通 member access / member call / safe member access 的 lowering 统一优先读取这张表；
    /// - 解决 resolver 初始决议与 typecheck 晚解析结果不一致时的语义分裂，
    ///   例如 receiver lambda 中隐式 `this` 触发的 late-bound member resolution。
    typechecked_member_resolutions: HashMap<Span, ast::ResolvedMemberRef>,
    /// `receiver.[field]` 的 typechecked 静态字段 contract。
    splice_field_contracts: HashMap<Span, ast::SpliceFieldContract>,
    /// `base with { ... }` 的 typechecked copy-update contract。
    with_update_contracts: HashMap<Span, ast::WithUpdateContract>,
    /// assignment statement LHS 的 typechecked place contract。
    assign_place_contracts: HashMap<Span, ast::AssignPlaceContract>,
    /// typecheck 已确认的 `Continuation.resume` 调用点。
    ///
    /// 用途：
    /// - 作为 effect segmentation 的确定语义 side table；
    /// - 避免后续阶段再按 `resume` 这个名字或调用形状做 builtin 推断。
    continuation_resume_call_sites: HashSet<Span>,
    /// `Continuation.resume` 中 receiver continuation 的 effect row 非 Pure 的调用点。
    ///
    /// 用途：
    /// - 区分“仅会隐藏触发 `Raise<RuntimeError>`”与“resumed body 还会对外继续 suspend”；
    /// - effect segmentation 只对这批调用点走 call-boundary replay 主线，Pure continuation
    ///   则继续保留 self-contained `try/catch` / runtime-raise hidden-boundary 语义。
    non_pure_continuation_resume_call_sites: HashSet<Span>,
    /// 零参调用经 typed `Unit` sugar canonicalize 为显式 `Unit` 实参的调用点。
    ///
    /// 用途：
    /// - 保持 AST/parser 继续保留 `f()` / `k.resume()` 的原始 surface 形状；
    /// - 让 HIR lowering 能按 typecheck 的最终决议，把 typed HIR call 形状统一落成 `(..., Unit)`。
    zero_arg_unit_call_sugar_sites: HashSet<Span>,
    /// typecheck 选中的“顶层函数值”目标。
    ///
    /// 用途：
    /// - 记录 `foo` / `foo<T>` 在值位置被视作函数值时的精确目标；
    /// - HIR lowering 读取后把它们合成为 closure object，而不是误走顶层值读取路径。
    top_level_fun_value_refs: HashMap<Span, ast::TopLevelFunValueRef>,
    /// typecheck 选中的“顶层函数调用”绑定信息。
    ///
    /// 用途：
    /// - 记录普通顶层函数调用在 overload resolution / 泛型实例化之后的最终声明目标；
    /// - 供 const/comptime 等后续阶段按 typecheck 最终决议重放调用，而不是重新按名字/arity 猜测。
    top_level_fun_call_bindings: HashMap<Span, ast::TopLevelFunCallBinding>,
    /// typecheck 选中的 canonical call-arg binding。
    ///
    /// 用途：把 named/default/vararg/spread 实参绑定发布给 HIR，避免后续阶段重读 raw AST 形状。
    typechecked_call_arg_bindings: HashMap<Span, ast::CallArgBinding>,
    /// typecheck 选中的 effect-op 调用绑定信息。
    ///
    /// 用途：
    /// - 记录 effect-op 调用点按形参顺序归一化后的实参映射；
    /// - HIR lowering / codegen 读取后统一处理命名实参与多 payload transport。
    typechecked_effect_op_call_bindings: HashMap<Span, ast::EffectOpCallBinding>,
    /// typecheck 选中的 ctor 调用绑定信息。
    ///
    /// 用途：
    /// - 记录 direct class ctor call、class header super ctor call、secondary ctor delegation
    ///   的最终 ctor 目标与参数绑定；
    /// - HIR lowering / codegen 读取后统一执行命名实参重排与默认值补齐，避免再按 arity 猜测。
    typechecked_ctor_call_bindings: HashMap<Span, ast::CtorCallBinding>,
    /// 单态化（monomorphization）请求收集器（T0712）。
    ///
    /// 说明：
    /// - 该字段仅在“需要收集 monomorph 实例”的入口中启用；
    /// - typecheck 阶段遇到“泛型函数调用”时会把 (callee, type args) 记录下来，
    ///   供后续 monomorph pass 生成专用实例并做去重缓存。
    monomorph_requests: Option<MonomorphRequests>,

    /// 泛型类型实例化请求收集器（T1109）。
    ///
    /// 说明：
    /// - 该字段仅在“需要统计类型实例命中/缺失”的入口中启用；
    /// - 记录目标为：名义类型（nominal）+ 非空 type args。
    type_instantiation_requests: Option<TypeInstantiationRequests>,

    /// unsafe 上下文深度（T1003/T1004）。
    ///
    /// 说明：
    /// - 在 `@Unsafe` 函数体内，调用 `@Extern/@Unsafe` 函数是允许的；
    /// - 在非 unsafe context 中，这类调用会在表达式 typecheck 阶段报错（T1003）；
    /// - 未来 `@Unsafe do { ... }` block（T1004）会复用同一机制做局部 push/pop。
    unsafe_context_depth: usize,

    /// `@NoGC` 上下文深度（TODO T1005）。
    ///
    /// 说明：
    /// - 在 `@NoGC` 函数体内，编译器需要保守拒绝“可能分配”的行为；
    /// - 目前我们只实现最小静态门禁（调用点/已知装箱点），更完整分析留给后续任务；
    /// - 使用 depth 而不是 bool，便于未来扩展局部 `@NoGC { ... }` 或其它可嵌套语境。
    nogc_context_depth: usize,

    /// `const fun` 上下文深度（TODO T1211）。
    ///
    /// 说明：
    /// - `const fun` 的限制更接近”编译期可执行”的静态门禁（禁止调用非 const、禁止闭包、禁止分配等）；
    /// - 使用 depth 而不是 bool，便于未来扩展局部 `const { ... }` / `comptime` 等可嵌套语境。
    const_context_depth: usize,

    /// `where` 约束 bound 作用域栈（T0130）。
    ///
    /// 说明：
    /// - 每一层表示一个作用域（函数或类型声明）中的 where 约束。
    /// - 每个条目记录 `(type_param_name, bound_type_ref, decl_file)` ：
    ///   当接收者类型为 `TypeKind::Param` 时，查找该 param 的 bound 接口方法集合。
    /// - 与 `type_param_scopes` 对齐地 push/pop。
    where_bound_scopes: Vec<Vec<WhereBoundEntry>>,
    /// annotation class 类型 lowering 许可深度。
    ///
    /// 说明：
    /// - 默认禁止：annotation class 不应进入普通运行期类型系统；
    /// - 仅在 annotation payload type 这类 compile-time-only 语境中临时开启；
    /// - 使用深度而非 bool，便于嵌套 helper 共享同一上下文控制。
    annotation_type_usage_depth: usize,
    /// 具体 nominal `TypeId` 的 direct supertypes（已完成 type param substitution）。
    concrete_direct_supertypes: HashMap<TypeId, Vec<TypeId>>,
}

impl<'a> TypeLowering<'a> {
    pub(crate) fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        imports: &'a ImportTable,
        env: &'a TypeEnv,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
    ) -> Self {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        let top_level_value_mutabilities =
            collect_top_level_value_mutabilities(source, file, &pkg_prefix);
        let extern_global_fqns = collect_extern_global_fqns(source, file, env);
        let extern_scoop_fun_decls = collect_extern_scoop_fun_decls(source, file, env);

        let mut ctx = Self::new_with_ctx(
            source,
            index,
            env,
            types,
            builtins,
            pkg_prefix,
            imports.clone(),
        );
        ctx.top_level_value_mutabilities = top_level_value_mutabilities;
        ctx.extern_global_fqns = extern_global_fqns;
        ctx.extern_scoop_fun_decls = extern_scoop_fun_decls;
        ctx
    }

    /// 在指定的 package/import 上下文中创建 `TypeLowering`。
    ///
    /// 用途：
    /// - typealias 展开时切换到“别名声明处文件”的上下文（T0446）
    /// - enum layout/metadata 计算时按“enum 声明处文件”的上下文降低 variant 字段类型（T0449）
    pub(crate) fn new_with_ctx(
        source: &'a SourceFile,
        index: &'a Index,
        env: &'a TypeEnv,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
        pkg_prefix: String,
        imports: ImportTable,
    ) -> Self {
        Self {
            source,
            index,
            imports,
            env,
            types,
            builtins,
            pkg_prefix,
            top_level_value_mutabilities: HashMap::new(),
            extern_global_fqns: HashSet::new(),
            extern_scoop_fun_decls: HashSet::new(),
            type_param_scopes: Vec::new(),
            effect_row_param_scopes: Vec::new(),
            type_alias_stack: Vec::new(),
            type_alias_cache: HashMap::new(),
            effect_collection_enabled: false,
            warning_emission_enabled: true,
            effect_collection_suspend_depth: 0,
            performed_effects: Vec::new(),
            inferred_expr_tys: HashMap::new(),
            inferred_binding_tys: HashMap::new(),
            inferred_fun_return_tys: HashMap::new(),
            inferred_performed_effect_tys: HashMap::new(),
            inferred_handle_arm_effect_tys: HashMap::new(),
            inferred_handle_arm_op_type_args: HashMap::new(),
            safe_member_access_resolutions: HashMap::new(),
            typechecked_member_resolutions: HashMap::new(),
            splice_field_contracts: HashMap::new(),
            with_update_contracts: HashMap::new(),
            assign_place_contracts: HashMap::new(),
            continuation_resume_call_sites: HashSet::new(),
            non_pure_continuation_resume_call_sites: HashSet::new(),
            zero_arg_unit_call_sugar_sites: HashSet::new(),
            top_level_fun_value_refs: HashMap::new(),
            top_level_fun_call_bindings: HashMap::new(),
            typechecked_call_arg_bindings: HashMap::new(),
            typechecked_effect_op_call_bindings: HashMap::new(),
            typechecked_ctor_call_bindings: HashMap::new(),
            monomorph_requests: None,
            type_instantiation_requests: None,
            unsafe_context_depth: 0,
            nogc_context_depth: 0,
            const_context_depth: 0,
            where_bound_scopes: Vec::new(),
            annotation_type_usage_depth: 0,
            concrete_direct_supertypes: HashMap::new(),
        }
    }

    pub(super) fn pkg_prefix(&self) -> &str {
        &self.pkg_prefix
    }

    pub(super) fn is_top_level_value_mutable(&self, fqn: &str) -> bool {
        self.top_level_value_mutabilities
            .get(fqn)
            .copied()
            .unwrap_or(false)
    }

    pub(super) fn is_extern_global(&self, fqn: &str) -> bool {
        self.extern_global_fqns.contains(fqn)
    }

    pub(super) fn is_extern_scoop_fun_decl(&self, decl_file: &Path, decl_span: Span) -> bool {
        self.extern_scoop_fun_decls
            .contains(&(decl_file.to_path_buf(), decl_span))
    }

    pub(super) fn set_warning_emission_enabled(&mut self, enabled: bool) {
        self.warning_emission_enabled = enabled;
    }

    pub(super) fn with_warning_emission_suspended<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let saved = self.warning_emission_enabled;
        self.warning_emission_enabled = false;
        let out = f(self);
        self.warning_emission_enabled = saved;
        out
    }

    pub(crate) fn env(&self) -> &TypeEnv {
        self.env
    }

    pub(crate) fn concrete_direct_supertypes(&self, ty: TypeId) -> Option<&[TypeId]> {
        self.concrete_direct_supertypes
            .get(&ty)
            .map(|v| v.as_slice())
    }

    pub(crate) fn with_annotation_types_allowed<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.annotation_type_usage_depth += 1;
        let out = f(self);
        self.annotation_type_usage_depth = self.annotation_type_usage_depth.saturating_sub(1);
        out
    }

    fn annotation_types_allowed(&self) -> bool {
        self.annotation_type_usage_depth > 0
    }

    /// 计算并返回给定具体 nominal 类型的 direct supertypes（已完成 type/effect 实参 substitution）。
    ///
    /// 用途：
    /// - 供运行期 metadata（例如 parameterized interface itable）在编译期复用 typecheck 的
    ///   “具体化 supertype 链”主线，而不是再维护一套独立的参数替换逻辑。
    pub(crate) fn instantiated_direct_supertypes(
        &mut self,
        ty: TypeId,
    ) -> Result<Vec<TypeId>, TypeLowerError> {
        let (fqn, args) = match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                (nominal.fqn.clone(), nominal.args.clone())
            }
            _ => return Ok(Vec::new()),
        };

        self.ensure_concrete_direct_supertypes(ty, &fqn, &args)?;
        Ok(self
            .concrete_direct_supertypes
            .get(&ty)
            .cloned()
            .unwrap_or_default())
    }

    pub(super) fn push_unsafe_context(&mut self) {
        self.unsafe_context_depth += 1;
    }

    pub(super) fn pop_unsafe_context(&mut self) {
        debug_assert!(self.unsafe_context_depth > 0);
        self.unsafe_context_depth = self.unsafe_context_depth.saturating_sub(1);
    }

    pub(super) fn in_unsafe_context(&self) -> bool {
        self.unsafe_context_depth > 0
    }

    /// 在一个临时区域中“抑制 unsafe 上下文”（spec §15.9.5）。
    ///
    /// 用途：
    /// - `@Safe do { ... }`：即使处于外层 unsafe context，也要在该区域内禁止 unsafe primitives / `@Extern` / `@Unsafe` 调用；
    /// - `@Safe { ... }` closure：其 body 也要按 safe 语义检查；
    /// - `@Safe` 内仍允许嵌套 `@Unsafe do { ... }` 重新开启 unsafe（局部化）。
    pub(super) fn with_unsafe_context_suspended<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.unsafe_context_depth;
        self.unsafe_context_depth = 0;
        let out = f(self);
        self.unsafe_context_depth = saved;
        out
    }

    pub(super) fn with_safe_lambda_context<R>(
        &mut self,
        lam: &ast::LambdaExpr,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if lam.is_safe() {
            self.with_unsafe_context_suspended(f)
        } else {
            f(self)
        }
    }

    pub(super) fn push_nogc_context(&mut self) {
        self.nogc_context_depth += 1;
    }

    pub(super) fn pop_nogc_context(&mut self) {
        debug_assert!(self.nogc_context_depth > 0);
        self.nogc_context_depth = self.nogc_context_depth.saturating_sub(1);
    }

    pub(super) fn in_nogc_context(&self) -> bool {
        self.nogc_context_depth > 0
    }

    /// 在一个临时区域中“抑制 `@NoGC` 上下文”。
    ///
    /// 用途：
    /// - lambda body 的代码并不在外层函数执行时立即运行；
    ///   为避免把 `@NoGC` 的限制错误地施加到 lambda body 上（产生大量假阳性），
    ///   这里提供一个最小的“暂停”机制。
    pub(super) fn with_nogc_context_suspended<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.nogc_context_depth;
        self.nogc_context_depth = 0;
        let out = f(self);
        self.nogc_context_depth = saved;
        out
    }

    pub(super) fn push_const_context(&mut self) {
        self.const_context_depth += 1;
    }

    pub(super) fn pop_const_context(&mut self) {
        debug_assert!(self.const_context_depth > 0);
        self.const_context_depth = self.const_context_depth.saturating_sub(1);
    }

    pub(super) fn in_const_context(&self) -> bool {
        self.const_context_depth > 0
    }

    /// 推入一层 where 约束 bound 作用域（T0130）。
    ///
    /// 在进入泛型函数体或泛型类型成员体时调用，把 `where T: Bound` 的约束记录下来，
    /// 以便在 typecheck 期间对 `TypeKind::Param` 接收者的方法调用进行 bound 驱动的分发。
    pub(super) fn push_where_bounds(&mut self, bounds: Vec<WhereBoundEntry>) {
        self.where_bound_scopes.push(bounds);
    }

    /// 弹出最近一层 where 约束 bound 作用域（T0130）。
    pub(super) fn pop_where_bounds(&mut self) {
        let _ = self.where_bound_scopes.pop();
    }

    /// 查找所有 where 约束中以 `param_name` 为目标的 bound（T0130）。
    ///
    /// 从内层到外层扫描所有作用域，收集所有命中的条目（一个 type param 可能有多个 bound）。
    pub(super) fn lookup_where_bounds_for_param(&self, param_name: &str) -> Vec<&WhereBoundEntry> {
        let mut out = Vec::new();
        for scope in self.where_bound_scopes.iter().rev() {
            for entry in scope {
                if entry.param_name == param_name {
                    out.push(entry);
                }
            }
        }
        out
    }

    /// 开启 monomorph 请求收集（T0712）。
    ///
    /// 说明：默认情况下 typecheck 不收集这些信息，以避免在仅做语义检查时产生额外分配。
    pub(super) fn enable_monomorph_collection(&mut self) {
        self.monomorph_requests = Some(MonomorphRequests::default());
    }

    /// 取出并清空当前收集到的 monomorph requests。
    pub(super) fn take_monomorph_requests(&mut self) -> Vec<MonomorphRequest> {
        self.monomorph_requests
            .take()
            .map(|r| r.into_vec())
            .unwrap_or_default()
    }

    /// 开启“泛型类型实例化”请求收集（T1109）。
    pub(super) fn enable_type_instantiation_collection(&mut self) {
        self.type_instantiation_requests = Some(TypeInstantiationRequests::default());
    }

    /// 取出并清空当前收集到的 type instantiation keys。
    pub(super) fn take_type_instantiation_keys(&mut self) -> Vec<TypeInstantiationKey> {
        self.type_instantiation_requests
            .take()
            .map(|r| r.into_vec())
            .unwrap_or_default()
    }

    fn record_type_instantiation(&mut self, fqn: &str, type_args: &[TypeId]) {
        let Some(req) = self.type_instantiation_requests.as_mut() else {
            return;
        };
        if type_args.is_empty() {
            return;
        }
        req.record(TypeInstantiationKey {
            fqn: fqn.to_string(),
            type_args: type_args.to_vec(),
        });
    }

    /// 记录一次“泛型函数实例化调用”的 monomorph key（T0712）。
    ///
    /// 当前阶段约束：
    /// - 调用点必须显式传入被选中声明的 `decl_file/decl_span`，避免 imported/sysroot generic fun
    ///   被误记成“当前文件声明”；
    /// - `type_args` 与 `eff_args` 共同构成调用点请求的实例身份；
    ///   - 对 generic owner member/getter，`type_args` 允许以前缀形式携带 owner-specialization
    ///     的 concrete args，再拼接函数自身 type args；
    /// - effect-only generic fun（没有 type args，只有 effect row args）也必须进入请求集合。
    pub(super) fn record_monomorph_call(
        &mut self,
        callee_fqn: String,
        callee_decl_file: &Path,
        callee_decl_span: Span,
        type_args: &[TypeId],
        eff_args: &[EffectRow],
        call_span: Span,
    ) {
        let Some(req) = self.monomorph_requests.as_mut() else {
            return;
        };
        if type_args.is_empty() && eff_args.is_empty() {
            return;
        }

        let symbol = MonomorphSymbol {
            fqn: callee_fqn,
            decl_file: callee_decl_file.to_path_buf(),
            decl_span: callee_decl_span,
        };
        let key = MonomorphKey {
            symbol,
            type_args: type_args.to_vec(),
            eff_args: eff_args.to_vec(),
        };
        req.record(MonomorphRequest::new(
            key,
            self.source.path().to_path_buf(),
            call_span,
        ));
    }

    pub(super) fn push_effect_row_param_binding(&mut self, name: String, row: EffectRow) {
        let mut scope = HashMap::new();
        scope.insert(name, row);
        self.effect_row_param_scopes.push(scope);
    }

    pub(super) fn push_effect_row_param_marker_binding(&mut self, name: String, decl_span: Span) {
        let marker = self.intern_effect_row_param_marker(name.clone(), decl_span);
        self.push_effect_row_param_binding(name, EffectRow::new(vec![marker]));
    }

    fn intern_effect_row_param_marker(&mut self, name: String, decl_span: Span) -> TypeId {
        self.types.intern(TypeKind::Param(TypeParamType {
            name,
            decl_file: PathBuf::from(crate::hir::EFFECT_ROW_PARAM_DECL_FILE),
            decl_span,
        }))
    }

    pub(super) fn pop_effect_row_param_binding(&mut self) {
        let _ = self.effect_row_param_scopes.pop();
    }

    pub(super) fn index(&self) -> &Index {
        self.index
    }

    pub(super) fn imports(&self) -> &ImportTable {
        &self.imports
    }

    pub(super) fn is_object_type(&self, fqn: &str) -> bool {
        self.index.object_types.contains(fqn)
    }

    /// 在“声明处文件”的 source/package/import 上下文中执行一个闭包。
    ///
    /// 用途：
    /// - 某些跨文件 typecheck 路径需要在保留当前 type/effect param scope 的同时，
    ///   临时借用声明处文件的源码与 import 规则来解析 `TypeRef` 片段；
    /// - 例如 enum payload 字段定义在 `core.scoop`，但构造调用发生在 `task.scoop` 或用户文件。
    pub(super) fn with_decl_file_context<R>(
        &mut self,
        decl_file: &Path,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let saved_source = self.source;
        let saved_pkg_prefix = std::mem::replace(&mut self.pkg_prefix, pkg_prefix);
        let saved_imports = std::mem::replace(&mut self.imports, imports);

        self.source = decl_source;
        let out = f(self);
        self.source = saved_source;
        self.pkg_prefix = saved_pkg_prefix;
        self.imports = saved_imports;
        out
    }

    /// 在“声明处文件”的 package/import 上下文中 lower 一个 `TypeRef`。
    ///
    /// 用途：
    /// - 从 `Index` 侧的签名信息（如 ctor params）进行 typecheck 时，
    ///   需要按声明处的解析规则（import/star import）来降低类型引用；
    /// - 该方法会在缺少 env 上下文时回退到当前 `TypeLowering` 的上下文（保持健壮性）。
    pub(super) fn lower_type_ref_in_decl_file(
        &mut self,
        decl_file: &Path,
        ty: &ast::TypeRef,
    ) -> Result<TypeId, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );
        ctx.annotation_type_usage_depth = self.annotation_type_usage_depth;
        ctx.lower_type_ref(ty)
    }

    /// T0125：在”声明处文件”的 package/import 上下文中 lower 一个 `TypeRef`，并将指定的
    /// type param 名称注册为 `TypeKind::Param` scope（用于泛型 class ctor 参数类型解析）。
    pub(super) fn lower_type_ref_in_decl_file_with_fresh_type_params(
        &mut self,
        decl_file: &Path,
        type_param_names: &[String],
        ty: &ast::TypeRef,
    ) -> Result<TypeId, TypeLowerError> {
        if type_param_names.is_empty() {
            return self.lower_type_ref_in_decl_file(decl_file, ty);
        }

        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );
        ctx.annotation_type_usage_depth = self.annotation_type_usage_depth;

        // 为每个 type param name 创建一个 fresh TypeKind::Param。
        let mut scope: HashMap<String, TypeId> = HashMap::new();
        for name in type_param_names {
            let id = ctx.types.ty_param(crate::ty::TypeParamType {
                name: name.clone(),
                decl_file: decl_file.to_path_buf(),
                decl_span: crate::span::Span::new(0, 0),
            });
            scope.insert(name.clone(), id);
        }
        ctx.type_param_scopes.push(scope);

        let out = ctx.lower_type_ref(ty);
        let _ = ctx.type_param_scopes.pop();
        out
    }

    /// 在”声明处文件”的 package/import 上下文中 lower 一个 `TypeRef`，并注入 use-site type args。
    ///
    /// 用途（T0458）：
    /// - `where` 子句满足性检查需要把 bound 中出现的 `T` 等 type param 引用替换为具体实参类型；
    /// - 这里用 `push_type_param_bindings` 直接把 name → TypeId 映射写入 lowering 作用域。
    pub(super) fn lower_type_ref_in_decl_file_with_bindings(
        &mut self,
        decl_file: &Path,
        bindings: impl IntoIterator<Item = (String, TypeId)>,
        ty: &ast::TypeRef,
    ) -> Result<TypeId, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );
        ctx.annotation_type_usage_depth = self.annotation_type_usage_depth;
        ctx.push_type_param_bindings(bindings);
        let out = ctx.lower_type_ref(ty);
        ctx.pop_type_param_bindings();
        out
    }

    pub(crate) fn lower_struct_direct_field_infos(
        &mut self,
        struct_fqn: &str,
        concrete_args: &[TypeId],
        _use_span: Span,
    ) -> Result<Vec<StructDirectFieldInfo>, TypeLowerError> {
        let Some(ctors) = self.index.constructors.get(struct_fqn) else {
            return Ok(Vec::new());
        };
        let Some(primary_ctor) = ctors
            .iter()
            .find(|ctor| ctor.kind == ConstructorKind::Primary)
        else {
            return Ok(Vec::new());
        };

        let type_param_names = self
            .env
            .type_symbol(struct_fqn)
            .map(|sym| sym.type_param_names.clone())
            .unwrap_or_default();
        let bindings: Vec<(String, TypeId)> = type_param_names
            .into_iter()
            .zip(concrete_args.iter().copied())
            .collect();

        let mut out = Vec::with_capacity(primary_ctor.params.len());
        for param in &primary_ctor.params {
            // Upstream gate: `typecheck::check_file_headers` rejects struct/class
            // primary-ctor params without a type annotation via
            // `TypeHeaderError::MissingTypeAnnotation`. Reaching this site with
            // `param.ty == None` therefore violates that upstream contract.
            let ty_ref = param.ty.as_ref().unwrap_or_else(|| {
                unreachable!(
                    "primary ctor param without type annotation should have been rejected by check_file_headers",
                )
            });
            let ty = self.lower_type_ref_in_decl_file_with_bindings(
                &primary_ctor.decl_file,
                bindings.iter().cloned(),
                ty_ref,
            )?;
            out.push(StructDirectFieldInfo {
                name: param.name.clone(),
                ty,
                has_default: param.has_default,
            });
        }
        Ok(out)
    }

    /// 在“声明处文件”的 package/import 上下文中 lower 一个 effect row expression，并注入 use-site type args。
    ///
    /// 用途（T0511）：
    /// - nominal type 的 `eff` row 参数默认值写在声明处；
    /// - use-site 省略 `Type<eff ...>` 时，需要在声明文件的解析规则下计算默认 row，
    ///   并把其中出现的 type params（如 `Raise<T>`）替换为使用点的具体类型实参。
    pub(super) fn lower_effect_row_expr_in_decl_file_with_bindings(
        &mut self,
        decl_file: &Path,
        bindings: impl IntoIterator<Item = (String, TypeId)>,
        expr: Option<&ast::EffectRowExpr>,
    ) -> Result<EffectRow, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );
        ctx.push_type_param_bindings(bindings);
        let out = ctx.lower_effect_row_expr(expr);
        ctx.pop_type_param_bindings();
        out
    }

    /// 在“声明处文件”的 package/import 上下文中 lower 一个 effect row expression，并同时注入：
    /// - use-site type param 绑定（`T` → 具体 TypeId）
    /// - effect row param 绑定（`E` → 具体 EffectRow）
    ///
    /// 用途（T0609）：
    /// - override/interface impl 的 effect row 检查需要先对 receiver 的 use-site type args 与
    ///   `<eff E>` row 参数做 substitution，再比较 `R_over ⊆ R_base`。
    pub(crate) fn lower_effect_row_expr_in_decl_file_with_scopes(
        &mut self,
        decl_file: &Path,
        type_bindings: impl IntoIterator<Item = (String, TypeId)>,
        eff_bindings: impl IntoIterator<Item = (String, EffectRow)>,
        expr: Option<&ast::EffectRowExpr>,
    ) -> Result<EffectRow, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );
        ctx.annotation_type_usage_depth = self.annotation_type_usage_depth;

        ctx.push_type_param_bindings(type_bindings);
        let mut pushed_eff = 0usize;
        for (name, row) in eff_bindings {
            ctx.push_effect_row_param_binding(name, row);
            pushed_eff += 1;
        }

        let out = ctx.lower_effect_row_expr(expr);

        for _ in 0..pushed_eff {
            ctx.pop_effect_row_param_binding();
        }
        ctx.pop_type_param_bindings();
        out
    }

    /// 在“声明处文件”的 package/import 上下文中 lower 一个 TypeRef，并同时注入：
    /// - use-site type param 绑定（`T` → 具体 TypeId）
    /// - effect row param 绑定（`E` → 具体 EffectRow）
    ///
    /// 用途：
    /// - 跨文件（sysroot / cone 依赖）的函数签名收集：当参数/返回类型里出现 `/ E` 时，
    ///   需要在 lowering 阶段先把 `E` 绑定到默认值（缺省 Pure），以便类型可以被正确构造，
    ///   并在调用点再用推断出的 `E_arg` 做实例化替换。
    pub(crate) fn lower_type_ref_in_decl_file_with_scopes(
        &mut self,
        decl_file: &Path,
        type_bindings: impl IntoIterator<Item = (String, TypeId)>,
        eff_bindings: impl IntoIterator<Item = (String, EffectRow)>,
        ty: &ast::TypeRef,
    ) -> Result<TypeId, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );

        ctx.push_type_param_bindings(type_bindings);
        let mut pushed_eff = 0usize;
        for (name, row) in eff_bindings {
            ctx.push_effect_row_param_binding(name, row);
            pushed_eff += 1;
        }

        let out = ctx.lower_type_ref(ty);

        for _ in 0..pushed_eff {
            ctx.pop_effect_row_param_binding();
        }
        ctx.pop_type_param_bindings();

        out
    }

    /// 直接注入一组“使用点 type param 绑定”（name → TypeId）。
    ///
    /// 说明：
    /// - 与 `push_type_params` 相比，这里不分配新的 `TypeParamType`，而是把 `T` 直接映射到
    ///   “已实例化的实参类型”；
    /// - 该能力主要用于“在非 AST 语境内重用 TypeLowering”（例如 layout/metadata 计算）。
    pub(crate) fn push_type_param_bindings(
        &mut self,
        bindings: impl IntoIterator<Item = (String, TypeId)>,
    ) {
        let mut scope = HashMap::new();
        for (name, id) in bindings {
            scope.insert(name, id);
        }
        self.type_param_scopes.push(scope);
    }

    pub(crate) fn pop_type_param_bindings(&mut self) {
        let _ = self.type_param_scopes.pop();
    }

    pub(crate) fn lower_type_ref(&mut self, ty: &ast::TypeRef) -> Result<TypeId, TypeLowerError> {
        match ty {
            ast::TypeRef::Path(p) => self.lower_type_path(p),
            ast::TypeRef::Tuple(t) => {
                if t.elements.is_empty() {
                    return Ok(self.builtins.unit);
                }
                let mut elements = Vec::with_capacity(t.elements.len());
                for e in &t.elements {
                    elements.push(self.lower_type_ref(e)?);
                }
                Ok(self.types.ty_tuple(elements))
            }
            ast::TypeRef::Nullable { inner, .. } => {
                let inner = self.lower_type_ref(inner)?;
                Ok(self.types.ty_option(inner))
            }
            ast::TypeRef::Star { .. } => {
                // spec §3.3：`*` 是真实的 star projection，而不是普通 `Any`。
                //
                // 运行时读视图等价于 boxed `Any?`，但 typecheck 仍需保留“只读 / 禁写”语义，
                // 因此这里用独立的 `TypeKind::StarProjection` 表示。
                Ok(self.ty_star_projection())
            }
            ast::TypeRef::EffectRowArg { span, .. } => Err(TypeLowerError::UnsupportedTypeRef {
                kind: "use-site effect row arg (`eff ...`)",
                span: (*span).into(),
            }),
            ast::TypeRef::Function(f) => {
                let receiver = match &f.receiver {
                    Some(r) => Some(self.lower_type_ref(r)?),
                    None => None,
                };
                let mut params = Vec::with_capacity(f.params.len());
                for p in &f.params {
                    params.push(self.lower_type_ref(p)?);
                }
                let return_ty = self.lower_type_ref(&f.return_ty)?;
                let effects = self.lower_effect_row_expr(f.effects.as_ref())?;
                let effects_closed = f.effects.as_ref().is_some_and(|r| r.closed);
                Ok(self
                    .types
                    .ty_function(receiver, params, return_ty, effects, effects_closed))
            }
        }
    }

    pub(super) fn lower_effect_row_expr(
        &mut self,
        expr: Option<&ast::EffectRowExpr>,
    ) -> Result<EffectRow, TypeLowerError> {
        let Some(expr) = expr else {
            // spec §5.8.2：缺省效果为 Pure。
            return Ok(EffectRow::pure());
        };
        if expr.terms.is_empty() {
            return Ok(EffectRow::pure());
        }

        let mut terms: Vec<TypeId> = Vec::with_capacity(expr.terms.len());
        for term in &expr.terms {
            // T0509：effect row variable（`E`）在 lowering 阶段展开为其绑定的 row。
            if term.segments.len() == 1 && term.args.is_empty() {
                let name = self.source.slice(term.segments[0].span);
                // spec §5.8.4：闭合 row 的语义要求它是“不可逃逸”的边界；
                // 因此这里禁止在闭合 row 内直接引用 row 变量（例如 `E!`）。
                if expr.closed
                    && self
                        .effect_row_param_scopes
                        .iter()
                        .rev()
                        .any(|s| s.contains_key(name))
                {
                    return Err(TypeLowerError::ClosedEffectRowContainsRowVar {
                        name: name.to_string(),
                        span: term.span.into(),
                    });
                }
                if let Some(bound) = self
                    .effect_row_param_scopes
                    .iter()
                    .rev()
                    .find_map(|s| s.get(name))
                {
                    terms.extend(bound.terms.iter().copied());
                    continue;
                }
            }

            // effect item 的语法复用 `TypePath`；这里复用 TypeRef lowering，再做 kind 检查。
            let ty = self.lower_type_ref(&ast::TypeRef::Path(term.clone()))?;
            let ok = match self.type_kind(ty) {
                TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                    matches!(
                        self.nominal_decl_kind(&nominal.fqn),
                        Some(ast::TypeKind::Effect)
                    )
                }
                _ => false,
            };

            if !ok {
                return Err(TypeLowerError::EffectRowItemNotEffect {
                    item: self.source.slice(term.span).to_string(),
                    found: self.fmt_type(ty),
                    span: term.span.into(),
                });
            }

            terms.push(ty);
        }

        Ok(EffectRow::new(terms))
    }

    pub(super) fn lower_effect_row_expr_preserving_params(
        &mut self,
        expr: Option<&ast::EffectRowExpr>,
    ) -> Result<EffectRow, TypeLowerError> {
        let Some(expr) = expr else {
            return Ok(EffectRow::pure());
        };
        if expr.terms.is_empty() {
            return Ok(EffectRow::pure());
        }

        let mut terms: Vec<TypeId> = Vec::with_capacity(expr.terms.len());
        for term in &expr.terms {
            if term.segments.len() == 1 && term.args.is_empty() {
                let name = self.source.slice(term.segments[0].span);
                if expr.closed
                    && self
                        .effect_row_param_scopes
                        .iter()
                        .rev()
                        .any(|s| s.contains_key(name))
                {
                    return Err(TypeLowerError::ClosedEffectRowContainsRowVar {
                        name: name.to_string(),
                        span: term.span.into(),
                    });
                }
                if self
                    .effect_row_param_scopes
                    .iter()
                    .rev()
                    .any(|s| s.contains_key(name))
                {
                    if let Some(bound) = self
                        .effect_row_param_scopes
                        .iter()
                        .rev()
                        .find_map(|s| s.get(name))
                    {
                        terms.extend(bound.terms.iter().copied());
                    }
                    continue;
                }
            }

            let ty = self.lower_type_ref(&ast::TypeRef::Path(term.clone()))?;
            match self.types.kind(ty) {
                TypeKind::Ref(RefTypeKind::Nominal(_)) => terms.push(ty),
                _ => {
                    return Err(TypeLowerError::EffectRowItemNotEffect {
                        item: self.source.slice(term.span).to_string(),
                        found: self.fmt_type(ty),
                        span: term.span.into(),
                    });
                }
            }
        }

        Ok(EffectRow::new(terms))
    }

    pub(super) fn begin_effect_collection(&mut self) {
        self.effect_collection_enabled = true;
        self.effect_collection_suspend_depth = 0;
        self.performed_effects.clear();
    }

    pub(super) fn finish_effect_collection(&mut self) -> Vec<(TypeId, Span)> {
        self.effect_collection_enabled = false;
        self.effect_collection_suspend_depth = 0;
        std::mem::take(&mut self.performed_effects)
    }

    pub(super) fn with_effect_collection_suspended<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.effect_collection_suspend_depth += 1;
        let out = f(self);
        self.effect_collection_suspend_depth =
            self.effect_collection_suspend_depth.saturating_sub(1);
        out
    }

    pub(super) fn with_nested_effect_collection<R, E>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<R, E>,
    ) -> Result<(R, Vec<(TypeId, Span)>), E> {
        let saved_enabled = self.effect_collection_enabled;
        let saved_suspend = self.effect_collection_suspend_depth;
        let saved_effects = std::mem::take(&mut self.performed_effects);

        self.effect_collection_enabled = true;
        self.effect_collection_suspend_depth = 0;

        let result = f(self);

        let collected = std::mem::take(&mut self.performed_effects);
        self.effect_collection_enabled = saved_enabled;
        self.effect_collection_suspend_depth = saved_suspend;
        self.performed_effects = saved_effects;

        result.map(|value| (value, collected))
    }

    pub(super) fn record_performed_effect(&mut self, effect: TypeId, span: Span) {
        if !self.effect_collection_enabled {
            return;
        }
        if self.effect_collection_suspend_depth > 0 {
            return;
        }
        self.performed_effects.push((effect, span));
    }

    pub(super) fn record_inferred_expr_ty(&mut self, span: Span, ty: TypeId) {
        self.inferred_expr_tys.insert(span, ty);
    }

    pub(super) fn record_inferred_binding_ty(&mut self, span: Span, ty: TypeId) {
        self.inferred_binding_tys.insert(span, ty);
    }

    pub(super) fn record_inferred_fun_return_ty(&mut self, span: Span, ty: TypeId) {
        self.inferred_fun_return_tys.insert(span, ty);
    }

    pub(super) fn record_inferred_performed_effect_ty(&mut self, span: Span, ty: TypeId) {
        self.inferred_performed_effect_tys.insert(span, ty);
    }

    pub(super) fn record_inferred_handle_arm_effect_ty(&mut self, span: Span, ty: TypeId) {
        self.inferred_handle_arm_effect_tys.insert(span, ty);
    }

    pub(super) fn record_inferred_handle_arm_op_type_args(
        &mut self,
        span: Span,
        type_args: Vec<TypeId>,
    ) {
        self.inferred_handle_arm_op_type_args
            .insert(span, type_args);
    }

    pub(super) fn record_safe_member_access_resolution(
        &mut self,
        member_span: Span,
        resolved: ast::ResolvedMemberRef,
    ) {
        self.safe_member_access_resolutions
            .insert(member_span, resolved);
    }

    pub(super) fn record_typechecked_member_resolution(
        &mut self,
        member_span: Span,
        resolved: ast::ResolvedMemberRef,
    ) {
        match &resolved {
            ast::ResolvedMemberRef::Value { fqn }
            | ast::ResolvedMemberRef::ExtensionValue { fqn } => {
                self.emit_deprecated_value_use(fqn, member_span, "属性");
            }
            ast::ResolvedMemberRef::Fun { .. } | ast::ResolvedMemberRef::ExtensionFun { .. } => {}
        }
        self.typechecked_member_resolutions
            .insert(member_span, resolved);
    }

    pub(super) fn record_splice_field_contract(
        &mut self,
        expr_span: Span,
        contract: ast::SpliceFieldContract,
    ) {
        self.splice_field_contracts.insert(expr_span, contract);
    }

    pub(super) fn record_with_update_contract(
        &mut self,
        expr_span: Span,
        contract: ast::WithUpdateContract,
    ) {
        self.with_update_contracts.insert(expr_span, contract);
    }

    pub(super) fn record_assign_place_contract(
        &mut self,
        assign_span: Span,
        contract: ast::AssignPlaceContract,
    ) {
        self.assign_place_contracts.insert(assign_span, contract);
    }

    pub(super) fn emit_deprecated_type_use(&self, fqn: &str, span: Span) {
        if !self.warning_emission_enabled {
            return;
        }
        let Some(info) = self.env.deprecated_type(fqn) else {
            return;
        };
        let _warning_suppressions = self.install_warning_suppressions_for_current_source();
        warnings::emit(CompileWarning::deprecated_use(
            self.source,
            span,
            "类型",
            fqn,
            &info.message,
            info.replace_with.as_deref(),
        ));
    }

    pub(super) fn emit_deprecated_value_use(
        &self,
        fqn: &str,
        span: Span,
        subject_kind: &'static str,
    ) {
        if !self.warning_emission_enabled {
            return;
        }
        let Some(info) = self.env.deprecated_value(fqn) else {
            return;
        };
        let _warning_suppressions = self.install_warning_suppressions_for_current_source();
        warnings::emit(CompileWarning::deprecated_use(
            self.source,
            span,
            subject_kind,
            fqn,
            &info.message,
            info.replace_with.as_deref(),
        ));
    }

    pub(super) fn emit_deprecated_fun_use(
        &self,
        fqn: &str,
        decl_file: &Path,
        decl_span: Span,
        use_span: Span,
    ) {
        if !self.warning_emission_enabled {
            return;
        }
        let Some(info) = self.env.deprecated_fun(decl_file, decl_span) else {
            return;
        };
        let _warning_suppressions = self.install_warning_suppressions_for_current_source();
        warnings::emit(CompileWarning::deprecated_use(
            self.source,
            use_span,
            "函数",
            fqn,
            &info.message,
            info.replace_with.as_deref(),
        ));
    }

    fn install_warning_suppressions_for_current_source(&self) -> warnings::WarningSuppressionGuard {
        let suppressions = self
            .env
            .file_ast(self.source.path())
            .map(|file| collect_file_warning_suppressions(self.source, file))
            .unwrap_or_default();
        warnings::install_suppressions(suppressions)
    }

    pub(super) fn record_continuation_resume_call_site(&mut self, call_span: Span, non_pure: bool) {
        self.continuation_resume_call_sites.insert(call_span);
        if non_pure {
            self.non_pure_continuation_resume_call_sites
                .insert(call_span);
        }
    }

    pub(super) fn record_zero_arg_unit_call_sugar_site(&mut self, call_span: Span) {
        self.zero_arg_unit_call_sugar_sites.insert(call_span);
    }

    pub(super) fn ty_continuation(
        &mut self,
        resume_ty: TypeId,
        answer_ty: TypeId,
        effects: EffectRow,
    ) -> TypeId {
        self.types
            .intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: "scoop.core.Continuation".to_string(),
                args: vec![resume_ty, answer_ty],
                eff: Some(effects),
            })))
    }

    pub(super) fn ty_continuation_answer_hole(&mut self, span: Span) -> TypeId {
        self.types.ty_param(TypeParamType {
            name: "_".to_string(),
            decl_file: PathBuf::from(CONTINUATION_ANSWER_HOLE_DECL_FILE),
            decl_span: span,
        })
    }

    pub(super) fn is_continuation_answer_hole(&self, ty: TypeId) -> bool {
        matches!(
            self.types.kind(ty),
            TypeKind::Param(TypeParamType {
                name,
                decl_file,
                ..
            }) if name == "_" && decl_file == &PathBuf::from(CONTINUATION_ANSWER_HOLE_DECL_FILE)
        )
    }

    pub(super) fn record_top_level_fun_value_ref(
        &mut self,
        expr_span: Span,
        fqn: String,
        decl_file: std::path::PathBuf,
        decl_span: Span,
        type_args: Vec<TypeId>,
        eff_args: Vec<EffectRow>,
    ) {
        self.top_level_fun_value_refs.insert(
            expr_span,
            ast::TopLevelFunValueRef {
                fqn,
                decl_file,
                decl_span,
                type_args,
                eff_args,
            },
        );
    }

    pub(super) fn record_top_level_fun_call_binding(
        &mut self,
        call_span: Span,
        binding: ast::TopLevelFunCallBinding,
    ) {
        self.top_level_fun_call_bindings.insert(call_span, binding);
    }

    pub(super) fn record_typechecked_call_arg_binding(
        &mut self,
        call_span: Span,
        binding: ast::CallArgBinding,
    ) {
        self.typechecked_call_arg_bindings
            .insert(call_span, binding);
    }

    pub(super) fn record_typechecked_effect_op_call_binding(
        &mut self,
        call_span: Span,
        arg_mapping: Vec<usize>,
        op_type_args: Vec<TypeId>,
    ) {
        self.typechecked_effect_op_call_bindings.insert(
            call_span,
            ast::EffectOpCallBinding {
                arg_mapping,
                op_type_args,
            },
        );
    }

    pub(super) fn record_typechecked_ctor_call_binding(
        &mut self,
        call_span: Span,
        owner_fqn: String,
        ctor_span: Option<Span>,
        arg_mapping: Vec<Option<usize>>,
    ) {
        self.typechecked_ctor_call_bindings.insert(
            call_span,
            ast::CtorCallBinding {
                owner_fqn,
                ctor_span,
                arg_mapping,
            },
        );
    }

    pub(super) fn take_inferred_expr_tys(&mut self) -> HashMap<Span, TypeId> {
        std::mem::take(&mut self.inferred_expr_tys)
    }

    pub(super) fn take_inferred_binding_tys(&mut self) -> HashMap<Span, TypeId> {
        std::mem::take(&mut self.inferred_binding_tys)
    }

    pub(super) fn take_inferred_fun_return_tys(&mut self) -> HashMap<Span, TypeId> {
        std::mem::take(&mut self.inferred_fun_return_tys)
    }

    pub(super) fn take_inferred_performed_effect_tys(&mut self) -> HashMap<Span, TypeId> {
        std::mem::take(&mut self.inferred_performed_effect_tys)
    }

    pub(super) fn take_inferred_handle_arm_effect_tys(&mut self) -> HashMap<Span, TypeId> {
        std::mem::take(&mut self.inferred_handle_arm_effect_tys)
    }

    pub(super) fn take_inferred_handle_arm_op_type_args(&mut self) -> HashMap<Span, Vec<TypeId>> {
        std::mem::take(&mut self.inferred_handle_arm_op_type_args)
    }

    pub(super) fn take_safe_member_access_resolutions(
        &mut self,
    ) -> HashMap<Span, ast::ResolvedMemberRef> {
        std::mem::take(&mut self.safe_member_access_resolutions)
    }

    pub(super) fn take_typechecked_member_resolutions(
        &mut self,
    ) -> HashMap<Span, ast::ResolvedMemberRef> {
        std::mem::take(&mut self.typechecked_member_resolutions)
    }

    pub(super) fn take_splice_field_contracts(
        &mut self,
    ) -> HashMap<Span, ast::SpliceFieldContract> {
        std::mem::take(&mut self.splice_field_contracts)
    }

    pub(super) fn take_with_update_contracts(&mut self) -> HashMap<Span, ast::WithUpdateContract> {
        std::mem::take(&mut self.with_update_contracts)
    }

    pub(super) fn take_assign_place_contracts(
        &mut self,
    ) -> HashMap<Span, ast::AssignPlaceContract> {
        std::mem::take(&mut self.assign_place_contracts)
    }

    pub(super) fn take_continuation_resume_call_sites(&mut self) -> HashSet<Span> {
        std::mem::take(&mut self.continuation_resume_call_sites)
    }

    pub(super) fn take_non_pure_continuation_resume_call_sites(&mut self) -> HashSet<Span> {
        std::mem::take(&mut self.non_pure_continuation_resume_call_sites)
    }

    pub(super) fn take_zero_arg_unit_call_sugar_sites(&mut self) -> HashSet<Span> {
        std::mem::take(&mut self.zero_arg_unit_call_sugar_sites)
    }

    pub(super) fn take_top_level_fun_value_refs(
        &mut self,
    ) -> HashMap<Span, ast::TopLevelFunValueRef> {
        std::mem::take(&mut self.top_level_fun_value_refs)
    }

    pub(super) fn take_top_level_fun_call_bindings(
        &mut self,
    ) -> HashMap<Span, ast::TopLevelFunCallBinding> {
        std::mem::take(&mut self.top_level_fun_call_bindings)
    }

    pub(super) fn take_typechecked_call_arg_bindings(
        &mut self,
    ) -> HashMap<Span, ast::CallArgBinding> {
        std::mem::take(&mut self.typechecked_call_arg_bindings)
    }

    pub(super) fn take_typechecked_effect_op_call_bindings(
        &mut self,
    ) -> HashMap<Span, ast::EffectOpCallBinding> {
        std::mem::take(&mut self.typechecked_effect_op_call_bindings)
    }

    pub(super) fn take_typechecked_ctor_call_bindings(
        &mut self,
    ) -> HashMap<Span, ast::CtorCallBinding> {
        std::mem::take(&mut self.typechecked_ctor_call_bindings)
    }

    pub(crate) fn fmt_type(&self, id: TypeId) -> String {
        self.types.display(id).to_string()
    }

    pub(crate) fn types(&self) -> &TypeStore {
        self.types
    }

    /// 返回给定 `TypeId` 在 `TypeStore` 中的具体 kind（clone）。
    ///
    /// 说明：typecheck 的某些表达式语义（例如 `with` 更新）需要区分：
    /// - 是否为值类型/引用类型
    /// - 是否为名义值类型（struct/enum）
    pub(crate) fn type_kind(&self, id: TypeId) -> TypeKind {
        self.types.kind(id).clone()
    }

    fn is_integral_scalar_type(&self, id: TypeId) -> bool {
        matches!(
            self.type_kind(id),
            TypeKind::Value(ValueTypeKind::Int)
                | TypeKind::Value(ValueTypeKind::UInt)
                | TypeKind::Value(ValueTypeKind::IntN(_))
                | TypeKind::Value(ValueTypeKind::UIntN(_))
        )
    }

    fn is_interface_type(&self, id: TypeId) -> bool {
        match self.type_kind(id) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                matches!(
                    self.nominal_decl_kind(&nominal.fqn),
                    Some(ast::TypeKind::Interface)
                )
            }
            _ => false,
        }
    }

    /// 若给定 FQN 对应 nominal type，返回其声明的 `TypeKind`（struct/enum/class/interface/effect）。
    ///
    /// 用途：对“语义上只对某类 nominal type 生效”的规则做最小判定，例如：
    /// - `with` 更新当前阶段仅支持 `struct`
    pub(super) fn nominal_decl_kind(&self, fqn: &str) -> Option<ast::TypeKind> {
        let sym = self.env.type_symbol(fqn)?;
        match sym.kind {
            TypeSymbolKind::Nominal(kind) => Some(kind),
            TypeSymbolKind::TypeAlias => None,
        }
    }

    pub(super) fn is_ref(&self, id: TypeId) -> bool {
        self.types.is_ref(id)
    }

    pub(super) fn star_projection_read_ty(&mut self) -> TypeId {
        self.types.ty_option(self.builtins.any)
    }

    pub(super) fn ty_star_projection(&mut self) -> TypeId {
        let read_ty = self.star_projection_read_ty();
        self.types.ty_star_projection(read_ty)
    }

    pub(super) fn star_projection_read_view(&self, id: TypeId) -> Option<TypeId> {
        match self.types.kind(id) {
            TypeKind::StarProjection(star) => Some(star.read_ty),
            _ => None,
        }
    }

    pub(super) fn is_star_projection(&self, id: TypeId) -> bool {
        matches!(self.types.kind(id), TypeKind::StarProjection(_))
    }

    /// `*` 的运行时读视图兼容性：
    /// - 引用类型可直接作为 `Any?` 读取；
    /// - `Option<Ref>` 复用 nullable-ref niche，同样可作为 `Any?` 读取；
    /// - 其它值类型需要显式 boxing，因此不允许隐式上转到 `*`。
    pub(super) fn is_star_projection_read_compatible(&self, id: TypeId) -> bool {
        match self.types.kind(id) {
            TypeKind::StarProjection(_) | TypeKind::Ref(_) => true,
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                matches!(self.types.kind(*inner), TypeKind::Ref(_))
            }
            _ => false,
        }
    }

    pub(super) fn ty_option(&mut self, inner: TypeId) -> TypeId {
        self.types.ty_option(inner)
    }

    pub(super) fn ty_tuple(&mut self, elements: Vec<TypeId>) -> TypeId {
        self.types.ty_tuple(elements)
    }

    pub(super) fn ty_union(&mut self, variants: Vec<TypeId>) -> TypeId {
        self.types.ty_union(variants)
    }

    pub(super) fn ty_function(
        &mut self,
        receiver: Option<TypeId>,
        params: Vec<TypeId>,
        return_ty: TypeId,
        effects: EffectRow,
        effects_closed: bool,
    ) -> TypeId {
        self.types
            .ty_function(receiver, params, return_ty, effects, effects_closed)
    }

    pub(super) fn intern_type_kind(&mut self, kind: TypeKind) -> TypeId {
        self.types.intern(kind)
    }

    /// 将一个声明处的 `TypeParam` 构造成 `TypeId`（`TypeKind::Param`）。
    ///
    /// 用途：
    /// - 在 typecheck 阶段某些场景（例如 class member body）需要构造 `Box<T>` 这类 “仍未实例化的泛型类型”，
    ///   此时 `T` 应当是 `TypeKind::Param` 而不是 `Any` 等占位。
    pub(super) fn ty_param_from_decl(&mut self, p: &ast::TypeParam) -> TypeId {
        let name = self.source.slice(p.name.span).to_string();
        self.types.ty_param(TypeParamType {
            name,
            decl_file: self.source.path().to_path_buf(),
            decl_span: p.name.span,
        })
    }

    pub(super) fn ty_param_named(
        &mut self,
        name: String,
        decl_file: PathBuf,
        decl_span: Span,
    ) -> TypeId {
        self.types.ty_param(TypeParamType {
            name,
            decl_file,
            decl_span,
        })
    }

    pub(super) fn push_type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            return;
        }

        let mut scope: HashMap<String, TypeId> = HashMap::new();
        for p in params {
            let name = self.source.slice(p.name.span).to_string();
            let id = self.types.ty_param(TypeParamType {
                name: name.clone(),
                decl_file: self.source.path().to_path_buf(),
                decl_span: p.name.span,
            });
            scope.insert(name, id);
        }
        self.type_param_scopes.push(scope);
    }

    pub(super) fn pop_type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            return;
        }
        let _ = self.type_param_scopes.pop();
    }

    fn lookup_type_param(&self, name: &str) -> Option<TypeId> {
        for scope in self.type_param_scopes.iter().rev() {
            if let Some(id) = scope.get(name).copied() {
                return Some(id);
            }
        }
        None
    }

    /// 将“已解析出的类型 FQN + 已 lowering 的 type args”构造成 `TypeId`。
    ///
    /// 说明：
    /// - `lower_type_ref`/`lower_type_path` 以 AST 为入口，会递归 lowering type args；
    /// - enum variant 构造等场景（T0426）需要先对 type args 做 substitution/推断，
    ///   再把结果组装回一个 `TypeId`，因此提供该辅助方法。
    pub(super) fn lower_type_fqn_with_args(
        &mut self,
        fqn: String,
        args: Vec<TypeId>,
        span: Span,
    ) -> Result<TypeId, TypeLowerError> {
        self.lower_type_fqn_with_args_and_eff(fqn, args, None, span)
    }

    pub(super) fn lower_type_fqn_with_args_and_eff(
        &mut self,
        fqn: String,
        args: Vec<TypeId>,
        explicit_eff: Option<EffectRow>,
        span: Span,
    ) -> Result<TypeId, TypeLowerError> {
        self.emit_deprecated_type_use(&fqn, span);

        // 先对少数 builtin/special-case 做 lowering（不依赖 sysroot 声明/TypeEnv）。
        match fqn.as_str() {
            "scoop.core.Any" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.any);
            }
            "scoop.core.String" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.string);
            }
            "scoop.core.Unit" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.unit);
            }
            "scoop.core.Nothing" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.nothing);
            }
            "scoop.core.Bool" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.bool_);
            }
            "scoop.core.Char" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.char_);
            }
            "scoop.core.Float64" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.float64);
            }
            "scoop.core.Float32" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.float32);
            }
            "scoop.core.Int" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.int);
            }
            // T1027：internal atomics（`__AtomicInt`）——与 `Int` 相同布局的内部原子整型。
            //
            // 说明：
            // - source-level 通过 sysroot 声明（可能是 typealias 或 intrinsic struct）；
            // - typecheck 内部把它降低为与 `Int` 完全一致的 builtin 类型，避免后端出现额外 ABI 分歧。
            "scoop.unsafe.__AtomicInt" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.int);
            }
            "scoop.core.UInt" => {
                check_arity(&fqn, 0, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.builtins.uint);
            }
            "scoop.core.Option" => {
                check_arity(&fqn, 1, args.len(), span)?;
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                return Ok(self.types.ty_option(args[0]));
            }
            _ => {}
        }

        let expected = self.env.type_param_count(&fqn).ok_or_else(|| {
            TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.clone(),
                span: span.into(),
            }
        })?;
        let found = args.len();
        if expected != found {
            return Err(TypeLowerError::TypeArityMismatch {
                name: fqn,
                expected,
                found,
                span: span.into(),
            });
        }

        // T1011：`Ptr<T>` 的 pointee 必须是 GC-free 值类型（保守：宁可拒绝也不放过）。
        if fqn == PTR_FQN
            && let Some(pointee) = args.first().copied()
        {
            self.check_ptr_pointee_gc_free(pointee, span)?;
        }
        // `FunPtr<F>`：F 必须是无 effect，且 receiver/参数/返回值属于当前 native value contract；
        // 占位 type param 会在实例化时再次校验。
        if fqn == FUNPTR_FQN
            && let Some(sig) = args.first().copied()
        {
            self.check_funptr_signature_contract(sig, span)?;
        }

        // 一般名义类型：保留为 nominal type（早期阶段不展开/不做布局分析）。
        let Some(sym) = self.env.type_symbol(&fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn,
                span: span.into(),
            });
        };

        if sym.is_annotation_class && !self.annotation_types_allowed() {
            return Err(TypeLowerError::AnnotationTypeRuntimeUseNotAllowed {
                name: fqn,
                span: span.into(),
            });
        }

        match sym.kind {
            TypeSymbolKind::TypeAlias => {
                if explicit_eff.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: span.into(),
                    });
                }
                self.lower_type_alias_fqn(&fqn, &args, span)
            }
            TypeSymbolKind::Nominal(kind) => {
                self.check_where_constraints_on_instantiation(&fqn, sym, &args, span)?;
                let eff = match (&sym.eff_param, explicit_eff) {
                    (None, None) => None,
                    (None, Some(_)) => {
                        return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                            name: fqn,
                            span: span.into(),
                        });
                    }
                    (Some(_), Some(row)) => Some(row),
                    (Some(eff_param), None) => {
                        let bindings = sym
                            .type_param_names
                            .iter()
                            .cloned()
                            .zip(args.iter().copied())
                            .collect::<Vec<_>>();
                        Some(self.lower_effect_row_expr_in_decl_file_with_bindings(
                            &sym.decl_file,
                            bindings,
                            eff_param.default.as_ref(),
                        )?)
                    }
                };
                self.record_type_instantiation(&fqn, &args);
                let nominal = NominalType { fqn, args, eff };
                let id = match kind {
                    ast::TypeKind::Struct | ast::TypeKind::Enum => self
                        .types
                        .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
                    ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => self
                        .types
                        .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
                };
                let (nominal_fqn, nominal_args) = match self.types.kind(id) {
                    TypeKind::Value(ValueTypeKind::Nominal(n))
                    | TypeKind::Ref(RefTypeKind::Nominal(n)) => (n.fqn.clone(), n.args.clone()),
                    _ => unreachable!("nominal lowering produced non-nominal type"),
                };
                self.ensure_concrete_direct_supertypes(id, &nominal_fqn, &nominal_args)?;
                Ok(id)
            }
        }
    }

    fn check_where_constraints_on_instantiation(
        &mut self,
        type_fqn: &str,
        sym: &TypeSymbol,
        args: &[TypeId],
        use_span: Span,
    ) -> Result<(), TypeLowerError> {
        if sym.where_constraints.is_empty() {
            return Ok(());
        }

        // name -> concrete type arg
        let bindings = sym
            .type_param_names
            .iter()
            .cloned()
            .zip(args.iter().copied())
            .collect::<Vec<_>>();

        for c in &sym.where_constraints {
            let Some(arg_ty) = args.get(c.param_index).copied() else {
                continue;
            };

            // 当实参本身仍是“未知 kind 的 type param”（例如在泛型声明内部出现 `Box<T>`）时，
            // 我们把 where 约束视为 **假设** 而不是 **需要此刻验证的条件**。
            //
            // 更完整的“约束传播/求解”（例如要求 `T` 也声明 `where T: Bound`）留给后续推断阶段（T05）。
            if matches!(self.type_kind(arg_ty), TypeKind::Param(_)) {
                continue;
            }

            // 在声明处文件上下文中 lowering bound，并用 use-site type args 对其中出现的 `T` 做 substitution。
            let bound_ty = self.lower_type_ref_in_decl_file_with_bindings(
                &sym.decl_file,
                bindings.iter().cloned(),
                &c.bound,
            )?;

            if is_type_assignable(arg_ty, bound_ty, self, self.builtins) {
                continue;
            }

            let param = sym
                .type_param_names
                .get(c.param_index)
                .cloned()
                .unwrap_or_else(|| format!("#{}", c.param_index + 1));

            return Err(TypeLowerError::WhereConstraintNotSatisfied {
                type_fqn: type_fqn.to_string(),
                param,
                arg: self.fmt_type(arg_ty),
                bound: self.fmt_type(bound_ty),
                span: use_span.into(),
            });
        }

        Ok(())
    }

    fn check_ptr_pointee_gc_free(
        &mut self,
        pointee: TypeId,
        span: Span,
    ) -> Result<(), TypeLowerError> {
        // T1012：sysroot 的 unsafe intrinsics（例如 `ptrToUIntPtr/uintPtrToPtr`）在签名中会出现
        // `Ptr<T>`（T 为函数 type param）。在当前阶段我们仍保持“用户代码中 `Ptr<type param>`
        // 保守拒绝”的策略，但允许 **sysroot 文件** 中声明的 type param 通过该检查：
        // - sysroot 的这些声明本身无函数体（intrinsic），不会在未实例化时执行不安全行为；
        // - 真正的 use-site `Ptr<Int>` / `Ptr<String>` 仍会在类型 lowering 时被检查与拒绝。
        if let TypeKind::Param(p) = self.type_kind(pointee)
            && p.decl_file
                .components()
                .any(|c| c.as_os_str() == std::ffi::OsStr::new("sysroot"))
        {
            return Ok(());
        }

        let mut visiting: HashSet<TypeId> = HashSet::new();
        let mut memo: HashMap<TypeId, bool> = HashMap::new();

        if self.is_gc_free_value_type_inner(pointee, &mut visiting, &mut memo)? {
            return Ok(());
        }

        Err(TypeLowerError::PtrPointeeMustBeGcFree {
            found: self.fmt_type(pointee),
            span: span.into(),
        })
    }

    pub(crate) fn check_funptr_signature_contract(
        &mut self,
        sig: TypeId,
        span: Span,
    ) -> Result<(), TypeLowerError> {
        match self.type_kind(sig).clone() {
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                if fun.effects.is_pure() {
                    let mut visiting: HashSet<TypeId> = HashSet::new();
                    let mut memo: HashMap<TypeId, bool> = HashMap::new();

                    if let Some(receiver) = fun.receiver
                        && !self.is_native_abi_value_type_inner(
                            receiver,
                            true,
                            &mut visiting,
                            &mut memo,
                        )?
                    {
                        return Err(TypeLowerError::FunPtrSignatureNotSupportedByNativeAbi {
                            found: self.fmt_type(receiver),
                            span: span.into(),
                        });
                    }

                    for param in fun.params {
                        if !self.is_native_abi_value_type_inner(
                            param,
                            true,
                            &mut visiting,
                            &mut memo,
                        )? {
                            return Err(TypeLowerError::FunPtrSignatureNotSupportedByNativeAbi {
                                found: self.fmt_type(param),
                                span: span.into(),
                            });
                        }
                    }

                    if !self.is_native_abi_value_type_inner(
                        fun.return_ty,
                        true,
                        &mut visiting,
                        &mut memo,
                    )? {
                        return Err(TypeLowerError::FunPtrSignatureNotSupportedByNativeAbi {
                            found: self.fmt_type(fun.return_ty),
                            span: span.into(),
                        });
                    }

                    Ok(())
                } else {
                    Err(TypeLowerError::FunPtrSignatureMustBePure {
                        found: self.fmt_type(sig),
                        span: span.into(),
                    })
                }
            }
            // 允许占位 type param（例如 sysroot 或泛型声明中的 `FunPtr<F>`）；一旦实例化成具体类型会再次校验。
            TypeKind::Param(_) => Ok(()),
            _ => Err(TypeLowerError::FunPtrSignatureMustBeFunction {
                found: self.fmt_type(sig),
                span: span.into(),
            }),
        }
    }

    pub(crate) fn is_native_abi_value_type(&mut self, ty: TypeId) -> Result<bool, TypeLowerError> {
        let mut visiting: HashSet<TypeId> = HashSet::new();
        let mut memo: HashMap<TypeId, bool> = HashMap::new();
        self.is_native_abi_value_type_inner(ty, false, &mut visiting, &mut memo)
    }

    fn is_native_abi_value_type_inner(
        &mut self,
        id: TypeId,
        allow_type_params: bool,
        visiting: &mut HashSet<TypeId>,
        memo: &mut HashMap<TypeId, bool>,
    ) -> Result<bool, TypeLowerError> {
        if let Some(v) = memo.get(&id).copied() {
            return Ok(v);
        }

        // 防御性：native surface contract 同样不接受递归值类型穿过 ABI 边界。
        if !visiting.insert(id) {
            memo.insert(id, false);
            return Ok(false);
        }

        let ok = match self.type_kind(id).clone() {
            TypeKind::Ref(_) => false,
            TypeKind::StarProjection(_) => false,
            TypeKind::Param(_) => allow_type_params,
            TypeKind::Value(v) => match v {
                ValueTypeKind::Nothing => false,
                ValueTypeKind::Unit
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_) => true,
                ValueTypeKind::Option(_) => false,
                ValueTypeKind::Tuple(elements) => {
                    let mut ok = true;
                    for element in elements {
                        if !self.is_native_abi_value_type_inner(
                            element,
                            allow_type_params,
                            visiting,
                            memo,
                        )? {
                            ok = false;
                            break;
                        }
                    }
                    ok
                }
                ValueTypeKind::Nominal(nominal) => self.is_native_abi_nominal_value_type(
                    &nominal,
                    allow_type_params,
                    visiting,
                    memo,
                )?,
            },
        };

        visiting.remove(&id);
        memo.insert(id, ok);
        Ok(ok)
    }

    fn is_native_abi_nominal_value_type(
        &mut self,
        nominal: &NominalType,
        allow_type_params: bool,
        visiting: &mut HashSet<TypeId>,
        memo: &mut HashMap<TypeId, bool>,
    ) -> Result<bool, TypeLowerError> {
        if nominal.fqn == PTR_FQN {
            return Ok(true);
        }

        if nominal.fqn == FUNPTR_FQN {
            let Some(sig) = nominal.args.first().copied() else {
                return Ok(false);
            };
            return self.funptr_signature_matches_native_abi_surface(
                sig,
                allow_type_params,
                visiting,
                memo,
            );
        }

        match self.nominal_decl_kind(&nominal.fqn) {
            Some(ast::TypeKind::Struct) => {
                self.is_native_abi_clayout_struct(nominal, allow_type_params, visiting, memo)
            }
            _ => Ok(false),
        }
    }

    fn funptr_signature_matches_native_abi_surface(
        &mut self,
        sig: TypeId,
        allow_type_params: bool,
        visiting: &mut HashSet<TypeId>,
        memo: &mut HashMap<TypeId, bool>,
    ) -> Result<bool, TypeLowerError> {
        match self.type_kind(sig).clone() {
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                if !fun.effects.is_pure() {
                    return Ok(false);
                }

                if let Some(receiver) = fun.receiver
                    && !self.is_native_abi_value_type_inner(
                        receiver,
                        allow_type_params,
                        visiting,
                        memo,
                    )?
                {
                    return Ok(false);
                }

                for param in fun.params {
                    if !self.is_native_abi_value_type_inner(
                        param,
                        allow_type_params,
                        visiting,
                        memo,
                    )? {
                        return Ok(false);
                    }
                }

                self.is_native_abi_value_type_inner(
                    fun.return_ty,
                    allow_type_params,
                    visiting,
                    memo,
                )
            }
            TypeKind::Param(_) => Ok(allow_type_params),
            _ => Ok(false),
        }
    }

    fn is_native_abi_clayout_struct(
        &mut self,
        nominal: &NominalType,
        allow_type_params: bool,
        visiting: &mut HashSet<TypeId>,
        memo: &mut HashMap<TypeId, bool>,
    ) -> Result<bool, TypeLowerError> {
        let Some(sym) = self.env.type_symbol(&nominal.fqn) else {
            return Ok(false);
        };

        let Some(decl_source) = self.env.source(&sym.decl_file) else {
            return Ok(false);
        };

        let Ok(file) = crate::parser::parse_file(decl_source) else {
            return Ok(false);
        };

        let Some(decl) = find_type_decl_by_fqn(decl_source, &file, &nominal.fqn) else {
            return Ok(false);
        };

        if !type_decl_has_clayout_annotation(decl_source, decl) {
            return Ok(false);
        }

        let bindings = sym
            .type_param_names
            .iter()
            .cloned()
            .zip(nominal.args.iter().copied())
            .collect::<Vec<_>>();

        if let Some(primary) = &decl.primary_ctor {
            for param in &primary.params {
                let Some(ty_ref) = param.ty.as_ref() else {
                    return Ok(false);
                };
                let field_ty = self.lower_type_ref_in_decl_file_with_bindings(
                    &sym.decl_file,
                    bindings.iter().cloned(),
                    ty_ref,
                )?;
                if !self.is_native_abi_value_type_inner(
                    field_ty,
                    allow_type_params,
                    visiting,
                    memo,
                )? {
                    return Ok(false);
                }
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(property) = member else {
                    continue;
                };
                if !property.is_direct_field() {
                    continue;
                }
                let Some(ty_ref) = property.ty.as_ref() else {
                    return Ok(false);
                };
                let field_ty = self.lower_type_ref_in_decl_file_with_bindings(
                    &sym.decl_file,
                    bindings.iter().cloned(),
                    ty_ref,
                )?;
                if !self.is_native_abi_value_type_inner(
                    field_ty,
                    allow_type_params,
                    visiting,
                    memo,
                )? {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    pub(crate) fn is_gc_free_value_type(&mut self, ty: TypeId) -> Result<bool, TypeLowerError> {
        let mut visiting: HashSet<TypeId> = HashSet::new();
        let mut memo: HashMap<TypeId, bool> = HashMap::new();
        self.is_gc_free_value_type_inner(ty, &mut visiting, &mut memo)
    }

    fn is_gc_free_value_type_inner(
        &mut self,
        id: TypeId,
        visiting: &mut HashSet<TypeId>,
        memo: &mut HashMap<TypeId, bool>,
    ) -> Result<bool, TypeLowerError> {
        if let Some(v) = memo.get(&id).copied() {
            return Ok(v);
        }

        // 防御性：遇到循环类型（直接或间接）时保守拒绝。
        if !visiting.insert(id) {
            memo.insert(id, false);
            return Ok(false);
        }

        let ok = match self.type_kind(id) {
            TypeKind::Ref(_) => false,
            TypeKind::StarProjection(_) => false,
            // 保守：类型参数可能实例化为引用类型，因此视为 non-GC-free。
            TypeKind::Param(_) => false,
            TypeKind::Value(v) => match v {
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_) => true,
                ValueTypeKind::Option(inner) => {
                    self.is_gc_free_value_type_inner(inner, visiting, memo)?
                }
                ValueTypeKind::Tuple(elements) => {
                    let mut ok = true;
                    for e in elements {
                        if !self.is_gc_free_value_type_inner(e, visiting, memo)? {
                            ok = false;
                            break;
                        }
                    }
                    ok
                }
                ValueTypeKind::Nominal(nominal) => {
                    self.is_gc_free_nominal_value_type(&nominal, visiting, memo)?
                }
            },
        };

        visiting.remove(&id);
        memo.insert(id, ok);
        Ok(ok)
    }

    fn is_gc_free_nominal_value_type(
        &mut self,
        nominal: &NominalType,
        visiting: &mut HashSet<TypeId>,
        memo: &mut HashMap<TypeId, bool>,
    ) -> Result<bool, TypeLowerError> {
        // `Ptr<T>` 本身也递归施加同样的 GC-free 约束：
        // - `Ptr<String>` 直接非法；
        // - `Ptr<Ptr<String>>` 也应当非法（否则可在内存中间接存放 GC 引用）。
        if nominal.fqn == PTR_FQN {
            let Some(pointee) = nominal.args.first().copied() else {
                return Ok(false);
            };
            return self.is_gc_free_value_type_inner(pointee, visiting, memo);
        }

        let kind = self.nominal_decl_kind(&nominal.fqn);
        match kind {
            Some(ast::TypeKind::Struct) => self.is_gc_free_nominal_struct(nominal, visiting, memo),
            Some(ast::TypeKind::Enum) => self.is_gc_free_nominal_enum(nominal, visiting, memo),
            // 理论上 nominal value type 只会是 struct/enum；其它情况保守拒绝。
            _ => Ok(false),
        }
    }

    fn is_gc_free_nominal_enum(
        &mut self,
        nominal: &NominalType,
        visiting: &mut HashSet<TypeId>,
        memo: &mut HashMap<TypeId, bool>,
    ) -> Result<bool, TypeLowerError> {
        let Some(decl) = self.env.enum_decl(&nominal.fqn) else {
            return Ok(false);
        };
        let Some(sym) = self.env.type_symbol(&nominal.fqn) else {
            return Ok(false);
        };

        // name -> concrete type arg
        let bindings = sym
            .type_param_names
            .iter()
            .cloned()
            .zip(nominal.args.iter().copied())
            .collect::<Vec<_>>();

        for v in &decl.variants {
            for f in &v.fields {
                let field_ty = self.lower_type_ref_in_decl_file_with_bindings(
                    &decl.decl_file,
                    bindings.iter().cloned(),
                    &f.ty,
                )?;
                if !self.is_gc_free_value_type_inner(field_ty, visiting, memo)? {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    fn is_gc_free_nominal_struct(
        &mut self,
        nominal: &NominalType,
        visiting: &mut HashSet<TypeId>,
        memo: &mut HashMap<TypeId, bool>,
    ) -> Result<bool, TypeLowerError> {
        let Some(sym) = self.env.type_symbol(&nominal.fqn) else {
            return Ok(false);
        };

        let Some(decl_source) = self.env.source(&sym.decl_file) else {
            return Ok(false);
        };

        let Ok(file) = crate::parser::parse_file(decl_source) else {
            // 理论上不会发生：该文件已在 index/env 构建时成功 parse 过；这里保守拒绝。
            return Ok(false);
        };

        let Some(decl) = find_type_decl_by_fqn(decl_source, &file, &nominal.fqn) else {
            return Ok(false);
        };

        // name -> concrete type arg
        let bindings = sym
            .type_param_names
            .iter()
            .cloned()
            .zip(nominal.args.iter().copied())
            .collect::<Vec<_>>();

        // 1) primary ctor params：对 struct 视作字段（与 resolver/typecheck 现阶段语义一致）。
        if let Some(primary) = &decl.primary_ctor {
            for p in &primary.params {
                let Some(ty_ref) = p.ty.as_ref() else {
                    return Ok(false);
                };
                let field_ty = self.lower_type_ref_in_decl_file_with_bindings(
                    &sym.decl_file,
                    bindings.iter().cloned(),
                    ty_ref,
                )?;
                if !self.is_gc_free_value_type_inner(field_ty, visiting, memo)? {
                    return Ok(false);
                }
            }
        }

        // 2) body properties：对 struct 同样视作字段声明（保守：全部参与 GC-free 判定）。
        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(p) = member else {
                    continue;
                };
                if !p.is_direct_field() {
                    continue;
                }
                let Some(ty_ref) = p.ty.as_ref() else {
                    return Ok(false);
                };
                let field_ty = self.lower_type_ref_in_decl_file_with_bindings(
                    &sym.decl_file,
                    bindings.iter().cloned(),
                    ty_ref,
                )?;
                if !self.is_gc_free_value_type_inner(field_ty, visiting, memo)? {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    fn lower_type_path(&mut self, path: &ast::TypePath) -> Result<TypeId, TypeLowerError> {
        // 单段名且无实参：优先解析为当前作用域的 type parameter（它会 shadow 顶层同名 type）。
        if path.segments.len() == 1 && path.args.is_empty() {
            let name = self.source.slice(path.segments[0].span);
            if let Some(id) = self.lookup_type_param(name) {
                return Ok(id);
            }
        }

        let fqn = match self.resolve_type_path_fqn(path) {
            Ok(fqn) => fqn,
            Err(TypeLowerError::UnresolvedType { name, span }) => {
                let Some(builtin_fqn) = implicit_builtin_type_fqn(&name) else {
                    return Err(TypeLowerError::UnresolvedType { name, span });
                };
                builtin_fqn.to_string()
            }
            Err(other) => return Err(other),
        };
        self.emit_deprecated_type_use(&fqn, path.span);

        // T0253/T0511：
        // - use-site effect row 实参（`eff ...`）在 AST 中被建模为 `TypeRef::EffectRowArg`；
        // - 它不属于“类型参数”（arity），但会影响 nominal type identity 与后续调用检查。
        let mut eff_arg: Option<&ast::EffectRowExpr> = None;
        for a in &path.args {
            if let ast::TypeRef::EffectRowArg { row, .. } = a {
                // parser 已保证最多一个且位于末尾；这里保持健壮性取第一个。
                eff_arg.get_or_insert(row);
            }
        }
        let type_args = path
            .args
            .iter()
            .filter(|a| !matches!(a, ast::TypeRef::EffectRowArg { .. }))
            .collect::<Vec<_>>();
        let ptr_pointee_arg_span = if fqn == PTR_FQN {
            type_args.first().map(|arg| arg.span()).unwrap_or(path.span)
        } else {
            path.span
        };

        // 先对少数 builtin/special-case 做 lowering（不依赖 sysroot 声明/TypeEnv）。
        match fqn.as_str() {
            // `Any`：引用类型的顶层 supertype。
            "scoop.core.Any" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.any);
            }
            // `String`：内建字符串类型。
            "scoop.core.String" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.string);
            }
            // `Unit`：0 元 tuple。
            "scoop.core.Unit" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.unit);
            }
            // `Nothing`：bottom type。
            "scoop.core.Nothing" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.nothing);
            }
            // `Bool`：内建布尔类型。
            "scoop.core.Bool" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.bool_);
            }
            // `Char`：内建 Unicode scalar value。
            "scoop.core.Char" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.char_);
            }
            "scoop.core.Float64" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.float64);
            }
            "scoop.core.Float32" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.float32);
            }
            // `Int/UInt`：word-sized 整数。
            "scoop.core.Int" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.int);
            }
            // T1027：internal atomics（`__AtomicInt`）——与 `Int` 相同布局的内部原子整型。
            "scoop.unsafe.__AtomicInt" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.int);
            }
            "scoop.core.UInt" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.uint);
            }
            // `Option<T>`：值类型；同时也是 `T?` 的 desugar 目标。
            "scoop.core.Option" => {
                check_arity(&fqn, 1, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                let inner = self.lower_type_ref(type_args[0])?;
                return Ok(self.types.ty_option(inner));
            }
            _ => {}
        }

        let continuation_legacy_effect_shorthand =
            fqn == "scoop.core.Continuation" && type_args.len() == 1 && eff_arg.is_some();
        if continuation_legacy_effect_shorthand {
            return Err(TypeLowerError::ContinuationLegacyEffectShorthandRemoved {
                span: path.span.into(),
            });
        }
        let continuation_legacy_pure_shorthand =
            fqn == "scoop.core.Continuation" && type_args.len() == 1;
        if continuation_legacy_pure_shorthand {
            return Err(TypeLowerError::ContinuationLegacyPureShorthandRemoved {
                span: path.span.into(),
            });
        }
        let expected = self.env.type_param_count(&fqn).ok_or_else(|| {
            TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.clone(),
                span: path.span.into(),
            }
        })?;
        let found = type_args.len();
        if expected != found {
            return Err(TypeLowerError::TypeArityMismatch {
                name: fqn,
                expected,
                found,
                span: path.span.into(),
            });
        }

        // 一般名义类型：保留为 nominal type（早期阶段不展开/不做布局分析）。
        let Some(sym) = self.env.type_symbol(&fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn,
                span: path.span.into(),
            });
        };

        if sym.is_annotation_class && !self.annotation_types_allowed() {
            return Err(TypeLowerError::AnnotationTypeRuntimeUseNotAllowed {
                name: fqn,
                span: path.span.into(),
            });
        }

        let args = type_args
            .iter()
            .map(|a| self.lower_type_ref(a))
            .collect::<Result<Vec<_>, _>>()?;

        // T1011：`Ptr<T>` 的 pointee 必须是 GC-free 值类型（保守：宁可拒绝也不放过）。
        if fqn == PTR_FQN
            && let Some(pointee) = args.first().copied()
        {
            self.check_ptr_pointee_gc_free(pointee, ptr_pointee_arg_span)?;
        }
        // `FunPtr<F>`：F 必须是无 effect，且 receiver/参数/返回值属于当前 native value contract；
        // 占位 type param 会在实例化时再次校验。
        if fqn == FUNPTR_FQN
            && let Some(sig) = args.first().copied()
        {
            self.check_funptr_signature_contract(sig, ptr_pointee_arg_span)?;
        }

        match sym.kind {
            TypeSymbolKind::TypeAlias => {
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                self.lower_type_alias_fqn(&fqn, &args, path.span)
            }
            TypeSymbolKind::Nominal(kind) => {
                self.check_where_constraints_on_instantiation(&fqn, sym, &args, path.span)?;
                let eff = match (&sym.eff_param, eff_arg) {
                    (None, None) => None,
                    (None, Some(_)) => {
                        return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                            name: fqn,
                            span: path.span.into(),
                        });
                    }
                    (Some(_), Some(expr)) => Some(self.lower_effect_row_expr(Some(expr))?),
                    (Some(eff_param), None) => {
                        let bindings = sym
                            .type_param_names
                            .iter()
                            .cloned()
                            .zip(args.iter().copied())
                            .collect::<Vec<_>>();
                        Some(self.lower_effect_row_expr_in_decl_file_with_bindings(
                            &sym.decl_file,
                            bindings,
                            eff_param.default.as_ref(),
                        )?)
                    }
                };
                self.record_type_instantiation(&fqn, &args);
                let nominal = NominalType { fqn, args, eff };
                let id = match kind {
                    ast::TypeKind::Struct | ast::TypeKind::Enum => self
                        .types
                        .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
                    ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => self
                        .types
                        .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
                };
                let (nominal_fqn, nominal_args) = match self.types.kind(id) {
                    TypeKind::Value(ValueTypeKind::Nominal(n))
                    | TypeKind::Ref(RefTypeKind::Nominal(n)) => (n.fqn.clone(), n.args.clone()),
                    _ => unreachable!("nominal lowering produced non-nominal type"),
                };
                self.ensure_concrete_direct_supertypes(id, &nominal_fqn, &nominal_args)?;
                Ok(id)
            }
        }
    }

    fn ensure_concrete_direct_supertypes(
        &mut self,
        nominal_ty: TypeId,
        fqn: &str,
        args: &[TypeId],
    ) -> Result<(), TypeLowerError> {
        if self.concrete_direct_supertypes.contains_key(&nominal_ty) {
            return Ok(());
        }

        // 先插入占位，避免递归实例化（例如 `Foo<T> : Bar<Foo<T>>`）重复进入。
        self.concrete_direct_supertypes
            .insert(nominal_ty, Vec::new());

        let Some(sym) = self.env.type_symbol(fqn) else {
            return Ok(());
        };

        let bindings = sym
            .type_param_names
            .iter()
            .cloned()
            .zip(args.iter().copied())
            .collect::<Vec<_>>();

        let mut instantiated: Vec<TypeId> = Vec::new();
        if let Some(super_infos) = self.env.direct_supertype_infos(fqn) {
            for super_info in super_infos {
                let super_ty = self.lower_type_ref_in_decl_file_with_bindings(
                    &sym.decl_file,
                    bindings.iter().cloned(),
                    &super_info.ty,
                )?;
                if !instantiated.contains(&super_ty) {
                    instantiated.push(super_ty);
                }
            }
        }

        self.concrete_direct_supertypes
            .insert(nominal_ty, instantiated);
        Ok(())
    }

    fn lower_type_alias_fqn(
        &mut self,
        fqn: &str,
        type_args: &[TypeId],
        use_span: Span,
    ) -> Result<TypeId, TypeLowerError> {
        let key = TypeInstantiationKey {
            fqn: fqn.to_string(),
            type_args: type_args.to_vec(),
        };
        if let Some(id) = self.type_alias_cache.get(&key).copied() {
            return Ok(id);
        }

        if let Some(pos) = self.type_alias_stack.iter().position(|x| x == fqn) {
            // 构造 cycle chain：A -> B -> ... -> A
            let mut chain = self.type_alias_stack[pos..].to_vec();
            chain.push(fqn.to_string());
            let cycle = chain.join(" -> ");

            // 至少指出两个声明点：cycle 起点 + 当前栈顶（即“把我们带回去”的别名）。
            let first_fqn = chain.first().map(|s| s.as_str()).unwrap_or(fqn);
            let second_fqn = self
                .type_alias_stack
                .last()
                .map(|s| s.as_str())
                .unwrap_or(fqn);

            let first = self
                .env
                .type_alias(first_fqn)
                .map(|i| i.name_span)
                .unwrap_or(use_span);
            let second = self
                .env
                .type_alias(second_fqn)
                .map(|i| i.name_span)
                .unwrap_or(use_span);

            return Err(TypeLowerError::CyclicTypeAlias {
                cycle,
                first: first.into(),
                second: second.into(),
            });
        }

        let Some(info) = self.env.type_alias(fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.to_string(),
                span: use_span.into(),
            });
        };

        let Some(decl_source) = self.env.source(&info.decl_file) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.to_string(),
                span: use_span.into(),
            });
        };

        // 对 `.cone` 注入的合成 source：它可能只有文本切片能力，但不一定有 package/import 上下文。
        // 为保证下游仍可展开别名，这里在缺省时使用空上下文：
        // - rhs 类型在 ScoopIR 中会被导出为 FQN（不依赖 imports）；
        // - type params 通过 `type_args` 做显式绑定，不应捕获 use-site 的 scope。
        let (decl_pkg_prefix, decl_imports) = self
            .env
            .file_type_context(&info.decl_file)
            .map(|ctx| (ctx.pkg_prefix.clone(), ctx.imports.clone()))
            .unwrap_or_else(|| (String::new(), ImportTable::default()));

        let alias_sym =
            self.env
                .type_symbol(fqn)
                .ok_or_else(|| TypeLowerError::MissingTypeSymbolInEnv {
                    fqn: fqn.to_string(),
                    span: use_span.into(),
                })?;
        let type_bindings = alias_sym
            .type_param_names
            .iter()
            .cloned()
            .zip(type_args.iter().copied())
            .collect::<Vec<_>>();

        self.type_alias_stack.push(fqn.to_string());

        // 在“别名声明处文件”的 package/import 规则下展开 RHS。
        //
        // 注意：
        // - typealias 只允许通过 `Name<...>` 的显式 type args 实例化；
        // - 展开时不应捕获 use-site 的 type/effect param scopes（否则会把 RHS 里的同名标识符错误解析为外层参数）。
        let saved_source = self.source;
        let saved_pkg_prefix = std::mem::take(&mut self.pkg_prefix);
        let saved_imports = std::mem::take(&mut self.imports);
        let saved_type_param_scopes = std::mem::take(&mut self.type_param_scopes);
        let saved_eff_param_scopes = std::mem::take(&mut self.effect_row_param_scopes);

        self.source = decl_source;
        self.pkg_prefix = decl_pkg_prefix;
        self.imports = decl_imports;
        self.push_type_param_bindings(type_bindings);

        let lowered_rhs = self.lower_type_ref(&info.ty);

        // 恢复 use-site 上下文（无论 RHS lowering 成功与否）。
        self.pop_type_param_bindings();
        self.source = saved_source;
        self.pkg_prefix = saved_pkg_prefix;
        self.imports = saved_imports;
        self.type_param_scopes = saved_type_param_scopes;
        self.effect_row_param_scopes = saved_eff_param_scopes;

        let _ = self.type_alias_stack.pop();

        let rhs_id = lowered_rhs?;
        self.type_alias_cache.insert(key, rhs_id);
        Ok(rhs_id)
    }

    fn check_type_decl_headers(&mut self, ty: &ast::TypeDecl) -> Result<(), TypeLowerError> {
        // `TypeDecl` 的 type params 在其 header/body 的所有 type position 内可见。
        self.push_type_params(&ty.type_params);
        let ty_eff_binding = if let Some(eff_param) = &ty.eff_param {
            let name = self.source.slice(eff_param.name.span).to_string();
            let default = match eff_param.default.as_ref() {
                Some(expr) => self.lower_effect_row_expr(Some(expr))?,
                None => EffectRow::pure(),
            };
            self.push_effect_row_param_binding(name, default);
            true
        } else {
            false
        };

        // 变型位置规则（Appendix B.4）：在 lowering 过程中做最小静态校验。
        self.check_type_decl_variance_rules(ty)?;

        // T0458：`where` 子句中的 bound 也需要参与 lowering（arity/存在性等由 lowering 负责）。
        if let Some(w) = &ty.where_clause {
            for c in &w.constraints {
                let _ = self.lower_type_ref(&c.bound)?;
            }
        }

        // 主构造头参数类型
        if let Some(primary_ctor) = &ty.primary_ctor {
            let annotation_payload_context = ty.kind == ast::TypeKind::Class
                && ty.modifiers.contains(&ast::Modifier::Annotation);
            for p in &primary_ctor.params {
                if let Some(param_ty_ref) = &p.ty {
                    if annotation_payload_context {
                        let _ = self.with_annotation_types_allowed(|lower| {
                            lower.lower_type_ref(param_ty_ref)
                        })?;
                    } else {
                        let _ = self.lower_type_ref(param_ty_ref)?;
                    }
                }
            }
        }

        // 继承/实现列表类型。
        //
        // spec §2.3.2.1：value-only enum 使用 `enum E: Int { ... }` 的 `:` 后类型作为“底层整型表示”。
        // 这不是继承关系，因此这里做最小门禁：
        // - 仍会对该 TypeRef 做 lowering（保证存在性/arity 等），但会额外要求其为整型标量；
        // - 具体“是否允许额外 interface supertypes”留给后续任务细化（当前不需要）。
        let mut first_super: Option<(TypeId, Span)> = None;
        for (idx, st) in ty.supertypes.iter().enumerate() {
            let id = self.lower_type_ref(&st.ty)?;
            if idx == 0 {
                first_super = Some((id, st.ty.span()));
            }
        }

        let is_value_only_enum =
            matches!(ty.kind, ast::TypeKind::Enum) && first_super.is_some() && {
                let first_is_interface =
                    first_super.is_some_and(|(id, _)| self.is_interface_type(id));
                let has_discriminant = ty.body.as_ref().is_some_and(|body| {
                    body.members.iter().any(|m| {
                        matches!(m, ast::TypeMember::EnumVariant(v) if v.discriminant.is_some())
                    })
                });
                // 消歧策略：
                // - 若第一个 supertype 是 interface，默认视为 interface 实现列表；
                // - 否则视为 value-only enum 的底层类型（并要求其为整型标量）；
                // - 若出现 `A = 0` 判别值语法，则强制走 value-only enum 路径（即使底层类型写错了，也给出更直接的诊断）。
                !first_is_interface || has_discriminant
            };

        if is_value_only_enum {
            let Some((underlying, span)) = first_super else {
                unreachable!("is_value_only_enum implies first_super exists");
            };
            if !self.is_integral_scalar_type(underlying) {
                let enum_name = self.source.slice(ty.name.span).to_string();
                return Err(TypeLowerError::ValueOnlyEnumUnderlyingNotIntegral {
                    enum_name,
                    found: self.fmt_type(underlying),
                    span: span.into(),
                });
            }
        }

        // 成员签名类型（property/fun/nested type）
        let Some(body) = &ty.body else {
            self.pop_type_params(&ty.type_params);
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    if is_value_only_enum {
                        let variant_name = self.source.slice(v.name.span).to_string();
                        if !v.params.is_empty() {
                            return Err(TypeLowerError::ValueOnlyEnumVariantFieldsNotAllowed {
                                variant_name,
                                span: v.name.span.into(),
                            });
                        }

                        let Some(discriminant) = &v.discriminant else {
                            return Err(TypeLowerError::ValueOnlyEnumVariantMissingDiscriminant {
                                variant_name,
                                span: v.name.span.into(),
                            });
                        };

                        if !is_int_const_expr(discriminant) {
                            return Err(
                                TypeLowerError::ValueOnlyEnumVariantDiscriminantNotIntConst {
                                    variant_name,
                                    span: discriminant.span.into(),
                                },
                            );
                        }
                    }

                    for p in &v.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Property(p) => {
                    if let Some(ty) = &p.ty {
                        let _ = self.lower_type_ref(ty)?;
                    }
                }
                ast::TypeMember::InitBlock(_b) => {
                    // init block 属于初始化执行体；当前阶段 type lowering 仅处理声明头与成员签名。
                }
                ast::TypeMember::SecondaryCtor(ctor) => {
                    // 次构造器参数类型同样属于成员签名类型（T0257）。
                    for p in &ctor.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Fun(f) => {
                    self.push_type_params(&f.type_params);
                    let fun_eff_binding = if let Some(eff_param) = &f.eff_param {
                        let name = self.source.slice(eff_param.name.span).to_string();
                        let default = match eff_param.default.as_ref() {
                            Some(expr) => self.lower_effect_row_expr(Some(expr))?,
                            None => EffectRow::pure(),
                        };
                        self.push_effect_row_param_binding(name, default);
                        true
                    } else {
                        false
                    };
                    if let Some(receiver) = &f.receiver {
                        let _ = self.lower_type_ref(receiver)?;
                    }
                    for p in &f.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                    if let Some(ret) = &f.return_ty {
                        let _ = self.lower_type_ref(ret)?;
                    }
                    if let Some(w) = &f.where_clause {
                        for c in &w.constraints {
                            let _ = self.lower_type_ref(&c.bound)?;
                        }
                    }
                    if fun_eff_binding {
                        self.pop_effect_row_param_binding();
                    }
                    self.pop_type_params(&f.type_params);
                }
                ast::TypeMember::Type(nested) => {
                    self.check_type_decl_headers(nested)?;
                }
                ast::TypeMember::Object(obj) => {
                    self.check_object_decl_headers(obj)?;
                }
            }
        }

        if ty_eff_binding {
            self.pop_effect_row_param_binding();
        }
        self.pop_type_params(&ty.type_params);
        Ok(())
    }

    fn check_object_decl_headers(&mut self, obj: &ast::ObjectDecl) -> Result<(), TypeLowerError> {
        // 继承/实现列表类型
        for st in &obj.supertypes {
            let _ = self.lower_type_ref(&st.ty)?;
        }

        let Some(body) = &obj.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    for p in &v.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Property(p) => {
                    if let Some(ty) = &p.ty {
                        let _ = self.lower_type_ref(ty)?;
                    }
                }
                ast::TypeMember::InitBlock(_b) => {}
                ast::TypeMember::SecondaryCtor(ctor) => {
                    for p in &ctor.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Fun(f) => {
                    self.push_type_params(&f.type_params);
                    let fun_eff_binding = if let Some(eff_param) = &f.eff_param {
                        let name = self.source.slice(eff_param.name.span).to_string();
                        let default = match eff_param.default.as_ref() {
                            Some(expr) => self.lower_effect_row_expr(Some(expr))?,
                            None => EffectRow::pure(),
                        };
                        self.push_effect_row_param_binding(name, default);
                        true
                    } else {
                        false
                    };
                    if let Some(receiver) = &f.receiver {
                        let _ = self.lower_type_ref(receiver)?;
                    }
                    for p in &f.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                    if let Some(ret) = &f.return_ty {
                        let _ = self.lower_type_ref(ret)?;
                    }
                    if fun_eff_binding {
                        self.pop_effect_row_param_binding();
                    }
                    self.pop_type_params(&f.type_params);
                }
                ast::TypeMember::Type(nested) => {
                    self.check_type_decl_headers(nested)?;
                }
                ast::TypeMember::Object(nested) => {
                    self.check_object_decl_headers(nested)?;
                }
            }
        }

        Ok(())
    }

    /// 检查声明处变型（`in`/`out`）的最小位置规则（Appendix B.4）。
    ///
    /// 当前阶段只覆盖 type body 的“公开签名”层：
    /// - member fun：参数为 in-position，返回值为 out-position（receiver 视作第一个参数）
    /// - member property：`val` 为 out-position，`var` 视为 in+out（invariant）
    fn check_type_decl_variance_rules(&mut self, ty: &ast::TypeDecl) -> Result<(), TypeLowerError> {
        // 无需为“全 invariant”声明做额外遍历。
        if ty.type_params.iter().all(|p| p.variance.is_none()) {
            return Ok(());
        }

        // 只关心显式标注了 `in/out` 的参数；invariant 参数不参与位置限制。
        let mut variance_params: HashMap<String, ast::TypeParamVariance> = HashMap::new();
        for p in &ty.type_params {
            let Some(v) = p.variance else {
                continue;
            };
            let name = self.source.slice(p.name.span).to_string();
            variance_params.insert(name, v);
        }
        if variance_params.is_empty() {
            return Ok(());
        }

        let Some(body) = &ty.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    // enum variant 的 payload 字段类型会出现在构造器参数位置（in-position）。
                    for p in &v.params {
                        let Some(ty) = &p.ty else {
                            continue;
                        };
                        self.check_type_ref_variance(ty, VariancePos::In, &variance_params)?;
                    }
                }
                ast::TypeMember::Property(p) => {
                    let Some(ty) = &p.ty else {
                        continue;
                    };
                    let pos = match p.kind {
                        ast::ValKind::Val => VariancePos::Out,
                        ast::ValKind::Var => VariancePos::Invariant,
                    };
                    self.check_type_ref_variance(ty, pos, &variance_params)?;
                }
                ast::TypeMember::InitBlock(_b) => {}
                ast::TypeMember::SecondaryCtor(ctor) => {
                    for p in &ctor.params {
                        let Some(ty) = &p.ty else {
                            continue;
                        };
                        self.check_type_ref_variance(ty, VariancePos::In, &variance_params)?;
                    }
                }
                ast::TypeMember::Fun(f) => {
                    // receiver 视作第一个参数：in-position。
                    if let Some(receiver) = &f.receiver {
                        self.check_type_ref_variance(receiver, VariancePos::In, &variance_params)?;
                    }
                    for p in &f.params {
                        let Some(ty) = &p.ty else {
                            continue;
                        };
                        self.check_type_ref_variance(ty, VariancePos::In, &variance_params)?;
                    }
                    if let Some(ret) = &f.return_ty {
                        self.check_type_ref_variance(ret, VariancePos::Out, &variance_params)?;
                    }
                }
                ast::TypeMember::Type(_) | ast::TypeMember::Object(_) => {
                    // nested 声明会在其自身的 check 中处理；
                    // object 声明不引入新的声明处变型信息，且其成员是否属于“public API”有待
                    // 更完整的可见性/inner 规则确定，因此当前阶段不做递归检查。
                }
            }
        }

        Ok(())
    }

    fn check_type_ref_variance(
        &self,
        ty: &ast::TypeRef,
        pos: VariancePos,
        variance_params: &HashMap<String, ast::TypeParamVariance>,
    ) -> Result<(), TypeLowerError> {
        match ty {
            ast::TypeRef::Path(p) => {
                // 1) 直接引用当前声明的 type param（`T`）。
                if p.segments.len() == 1 && p.args.is_empty() {
                    let name = self.source.slice(p.segments[0].span);
                    if let Some(declared) = variance_params.get(name).copied() {
                        let ok = match (declared, pos) {
                            (ast::TypeParamVariance::Out, VariancePos::Out) => true,
                            (ast::TypeParamVariance::In, VariancePos::In) => true,
                            // invariant position 同时要求 in/out 都可用，因此对 in/out 均不合法
                            // （例如 `var x: T` 会同时把 T 用于 getter/setter）。
                            _ => false,
                        };
                        if !ok {
                            let declared = match declared {
                                ast::TypeParamVariance::Out => "out",
                                ast::TypeParamVariance::In => "in",
                            };
                            return Err(TypeLowerError::VariancePositionViolation {
                                param: name.to_string(),
                                declared,
                                position: pos.as_str(),
                                span: p.span.into(),
                            });
                        }
                    }
                    return Ok(());
                }

                // 2) 递归检查 type args：按被引用类型的声明处 variance 组合位置（Kotlin-like）。
                //
                // 说明：
                // - use-site effect row args 不属于 type args（arity），因此在这里忽略；
                // - `*` 不引入 type param 引用（但作为 `Any` view 会在 lowering 时处理）。
                let type_args = p
                    .args
                    .iter()
                    .filter(|a| !matches!(a, ast::TypeRef::EffectRowArg { .. }))
                    .collect::<Vec<_>>();

                if type_args.is_empty() {
                    return Ok(());
                }

                let fqn = match self.resolve_type_path_fqn(p) {
                    Ok(fqn) => fqn,
                    Err(TypeLowerError::UnresolvedType { name, .. }) => {
                        implicit_builtin_type_fqn(&name)
                            .unwrap_or(name.as_str())
                            .to_string()
                    }
                    Err(other) => return Err(other),
                };

                let declared_variances = self.env.type_param_variances(&fqn);

                for (idx, arg) in type_args.iter().copied().enumerate() {
                    let param_variance = declared_variances
                        .and_then(|v| v.get(idx).copied())
                        .unwrap_or(None);
                    let composed = pos.compose(param_variance);
                    self.check_type_ref_variance(arg, composed, variance_params)?;
                }

                Ok(())
            }
            ast::TypeRef::Tuple(t) => {
                for e in &t.elements {
                    self.check_type_ref_variance(e, pos, variance_params)?;
                }
                Ok(())
            }
            ast::TypeRef::Star { .. } => Ok(()),
            ast::TypeRef::EffectRowArg { row, .. } => {
                // effect row expr 的项复用 type path 语法；它们不是 type param，因此忽略。
                // 未来若引入 `eff` 的 row 变量/约束系统，再在对应任务中处理。
                let _ = row;
                Ok(())
            }
            ast::TypeRef::Function(f) => {
                // 函数类型：参数（含 receiver）逆变，返回值协变。
                let param_pos = pos.flip();
                if let Some(r) = &f.receiver {
                    self.check_type_ref_variance(r, param_pos, variance_params)?;
                }
                for p in &f.params {
                    self.check_type_ref_variance(p, param_pos, variance_params)?;
                }
                self.check_type_ref_variance(&f.return_ty, pos, variance_params)?;
                Ok(())
            }
            ast::TypeRef::Nullable { inner, .. } => {
                self.check_type_ref_variance(inner, pos, variance_params)
            }
        }
    }

    pub(super) fn resolve_type_path_fqn(
        &self,
        path: &ast::TypePath,
    ) -> Result<String, TypeLowerError> {
        let segments = path
            .segments
            .iter()
            .map(|id| id.text(self.source))
            .collect::<Vec<_>>();
        let local = segments.join(".");

        let mut candidates = Vec::new();
        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, local));
        }
        // 允许显式写 FQN：`scoop.core.Any`
        candidates.push(local.clone());

        // 单段名字才走 import 规则（与 resolve 阶段保持一致）。
        if segments.len() == 1 {
            let name = segments[0];

            if let Some(fqns) = self.imports.ty.explicit.get(name) {
                candidates.extend(fqns.iter().cloned());
            }

            for prefix in &self.imports.star {
                candidates.push(format!("{prefix}.{name}"));
            }
        }

        candidates.sort();
        candidates.dedup();

        for fqn in candidates {
            let Some(syms) = self.index.by_fqn.get(&fqn) else {
                continue;
            };
            let Some(sym) = syms.ty.as_ref() else {
                continue;
            };
            if is_symbol_visible_from(self.source, sym) {
                return Ok(fqn);
            }
        }

        Err(TypeLowerError::UnresolvedType {
            name: local,
            span: path.span.into(),
        })
    }

    /// 在 typecheck 阶段按“路径段名”解析 type path 对应的 FQN。
    ///
    /// 说明：
    /// - `resolve_type_path_fqn` 依赖 `TypePath` 里的 `Ident.span` 从 `self.source` 切片；
    /// - 但某些场景（例如 sysroot enum variant 的字段类型，T0426）持有的是“来自其它源文件”的
    ///   `TypeRef`/`TypePath`，其 span 不能再用于当前文件切片；
    /// - 因此提供该按字符串段名解析的辅助入口（仍复用当前使用点的 package/import/可见性规则）。
    pub(super) fn resolve_type_path_fqn_by_name(
        &self,
        segments: &[String],
        use_span: Span,
    ) -> Result<String, TypeLowerError> {
        let local = segments.join(".");

        let mut candidates = Vec::new();
        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, local));
        }
        // 允许显式写 FQN：`scoop.core.Any`
        candidates.push(local.clone());

        // 单段名字才走 import 规则（与 resolve 阶段保持一致）。
        if segments.len() == 1 {
            let name = segments[0].as_str();

            if let Some(fqns) = self.imports.ty.explicit.get(name) {
                candidates.extend(fqns.iter().cloned());
            }

            for prefix in &self.imports.star {
                candidates.push(format!("{prefix}.{name}"));
            }
        }

        candidates.sort();
        candidates.dedup();

        for fqn in candidates {
            let Some(syms) = self.index.by_fqn.get(&fqn) else {
                continue;
            };
            let Some(sym) = syms.ty.as_ref() else {
                continue;
            };
            if is_symbol_visible_from(self.source, sym) {
                return Ok(fqn);
            }
        }

        Err(TypeLowerError::UnresolvedType {
            name: local,
            span: use_span.into(),
        })
    }
}

fn find_type_decl_by_fqn<'a>(
    source: &SourceFile,
    file: &'a ast::File,
    target_fqn: &str,
) -> Option<&'a ast::TypeDecl> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    for item in &file.items {
        match item {
            ast::Item::Type(ty) => {
                if let Some(found) =
                    find_type_decl_in_type_decl(source, ty, &pkg_prefix, target_fqn)
                {
                    return Some(found);
                }
            }
            ast::Item::Object(obj) => {
                if let Some(found) =
                    find_type_decl_in_object_decl(source, obj, &pkg_prefix, target_fqn)
                {
                    return Some(found);
                }
            }
            ast::Item::TypeAlias(_)
            | ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::ComptimeIf(_) => {}
        }
    }

    None
}

fn type_decl_has_clayout_annotation(source: &SourceFile, decl: &ast::TypeDecl) -> bool {
    decl.annotations.iter().any(|ann| {
        let segs = ann
            .path
            .iter()
            .map(|id| id.text(source))
            .collect::<Vec<_>>();
        matches!(segs.as_slice(), ["CLayout"]) || segs.join(".") == CLAYOUT_FQN
    })
}

fn find_type_decl_in_type_decl<'a>(
    source: &SourceFile,
    decl: &'a ast::TypeDecl,
    prefix: &str,
    target_fqn: &str,
) -> Option<&'a ast::TypeDecl> {
    let local_name = source.slice(decl.name.span);
    let type_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    if type_fqn == target_fqn {
        return Some(decl);
    }

    let Some(body) = &decl.body else {
        return None;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                if let Some(found) =
                    find_type_decl_in_type_decl(source, nested, &type_fqn, target_fqn)
                {
                    return Some(found);
                }
            }
            ast::TypeMember::Object(obj) => {
                if let Some(found) =
                    find_type_decl_in_object_decl(source, obj, &type_fqn, target_fqn)
                {
                    return Some(found);
                }
            }
            _ => {}
        }
    }

    None
}

fn find_type_decl_in_object_decl<'a>(
    source: &SourceFile,
    obj: &'a ast::ObjectDecl,
    prefix: &str,
    target_fqn: &str,
) -> Option<&'a ast::TypeDecl> {
    let local_name = match (&obj.name, obj.kind) {
        (Some(name), _) => source.slice(name.span).to_string(),
        (None, ast::ObjectKind::Companion) => "Companion".to_string(),
        (None, ast::ObjectKind::Object) => return None,
    };

    let obj_fqn = if prefix.is_empty() {
        local_name.to_string()
    } else {
        format!("{prefix}.{local_name}")
    };

    let Some(body) = &obj.body else {
        return None;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                if let Some(found) =
                    find_type_decl_in_type_decl(source, nested, &obj_fqn, target_fqn)
                {
                    return Some(found);
                }
            }
            ast::TypeMember::Object(nested_obj) => {
                if let Some(found) =
                    find_type_decl_in_object_decl(source, nested_obj, &obj_fqn, target_fqn)
                {
                    return Some(found);
                }
            }
            _ => {}
        }
    }

    None
}

fn is_symbol_visible_from(source: &SourceFile, symbol: &crate::resolve::Symbol) -> bool {
    match symbol.visibility {
        Visibility::Public | Visibility::Internal => true,
        Visibility::Private => symbol.decl_file == source.path(),
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

fn collect_top_level_value_mutabilities(
    source: &SourceFile,
    file: &ast::File,
    pkg_prefix: &str,
) -> HashMap<String, bool> {
    let mut map: HashMap<String, bool> = HashMap::new();

    for item in &file.items {
        let ast::Item::Val(v) = item else {
            continue;
        };

        let ast::ValBinding::Name(name) = &v.binding else {
            continue;
        };

        let local_name = source.slice(name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };

        map.insert(fqn, v.kind == ast::ValKind::Var);
    }

    map
}

fn collect_extern_global_fqns(
    source: &SourceFile,
    file: &ast::File,
    env: &TypeEnv,
) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_extern_global_fqns_in_file(source, file, &mut out);

    for (path, stored_file) in env.files() {
        if path.as_path() == source.path() {
            continue;
        }
        let Some(stored_source) = env.source(path) else {
            continue;
        };
        collect_extern_global_fqns_in_file(stored_source, stored_file, &mut out);
    }

    out
}

fn collect_extern_scoop_fun_decls(
    source: &SourceFile,
    file: &ast::File,
    env: &TypeEnv,
) -> HashSet<(PathBuf, Span)> {
    let mut out = HashSet::new();
    collect_extern_scoop_fun_decls_in_file(source, file, &mut out);

    for (path, stored_file) in env.files() {
        if path.as_path() == source.path() {
            continue;
        }
        let Some(stored_source) = env.source(path) else {
            continue;
        };
        collect_extern_scoop_fun_decls_in_file(stored_source, stored_file, &mut out);
    }

    out
}

fn collect_extern_global_fqns_in_file(
    source: &SourceFile,
    file: &ast::File,
    out: &mut HashSet<String>,
) {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    for item in &file.items {
        let ast::Item::Val(v) = item else {
            continue;
        };
        if !BuiltinAnnotationFlags::from_annotations(source, &v.annotations).is_extern {
            continue;
        }
        let ast::ValBinding::Name(name) = &v.binding else {
            continue;
        };
        let local_name = source.slice(name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };
        out.insert(fqn);
    }
}

fn collect_extern_scoop_fun_decls_in_file(
    source: &SourceFile,
    file: &ast::File,
    out: &mut HashSet<(PathBuf, Span)>,
) {
    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };
        if !BuiltinAnnotationFlags::from_annotations(source, &fun.annotations).is_extern {
            continue;
        }
        if !extern_fun_annotations_use_scoop_abi(source, &fun.annotations) {
            continue;
        }
        out.insert((source.path().to_path_buf(), fun.name.span));
    }
}

fn extern_fun_annotations_use_scoop_abi(
    source: &SourceFile,
    annotations: &[ast::AnnotationUse],
) -> bool {
    annotations
        .iter()
        .filter(|ann| annotation_is_extern_use(source, ann))
        .flat_map(|ann| ann.args.iter())
        .find_map(|arg| extern_annotation_named_arg_string_value(source, arg, "abi"))
        .is_some_and(|abi| abi.eq_ignore_ascii_case("scoop"))
}

fn annotation_is_extern_use(source: &SourceFile, ann: &ast::AnnotationUse) -> bool {
    let segs = ann
        .path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>();
    matches!(segs.as_slice(), ["Extern"] | ["scoop", "core", "Extern"])
}

fn extern_annotation_named_arg_string_value(
    source: &SourceFile,
    arg: &ast::AnnotationArg,
    key: &str,
) -> Option<String> {
    match &arg.name {
        Some(name) if name.text(source) == key => {
            annotation_string_literal_text(source, &arg.value)
        }
        Some(_) => None,
        None => match &arg.value.kind {
            ast::ExprKind::Assign { lhs, rhs, .. } => {
                let ast::ExprKind::Ident(id) = &lhs.kind else {
                    return None;
                };
                if source.slice(id.span) != key {
                    return None;
                }
                annotation_string_literal_text(source, rhs.as_ref())
            }
            _ => None,
        },
    }
}

fn annotation_string_literal_text(source: &SourceFile, expr: &ast::Expr) -> Option<String> {
    if !matches!(expr.kind, ast::ExprKind::StringLit) {
        return None;
    }
    let raw = source.slice(expr.span);
    let value = raw
        .strip_prefix("\"\"\"")
        .and_then(|text| text.strip_suffix("\"\"\""))
        .or_else(|| {
            raw.strip_prefix('"')
                .and_then(|text| text.strip_suffix('"'))
        })
        .unwrap_or(raw);
    Some(value.to_string())
}

/// 单态化（monomorphization）请求集合：去重 + 保留稳定顺序。
#[derive(Debug, Default)]
struct MonomorphRequests {
    seen: HashSet<MonomorphRequest>,
    ordered: Vec<MonomorphRequest>,
}

/// 泛型类型实例化请求集合：去重 + 保留稳定顺序（T1109）。
#[derive(Debug, Default)]
struct TypeInstantiationRequests {
    seen: HashSet<TypeInstantiationKey>,
    ordered: Vec<TypeInstantiationKey>,
}

impl TypeInstantiationRequests {
    fn record(&mut self, key: TypeInstantiationKey) {
        if self.seen.insert(key.clone()) {
            self.ordered.push(key);
        }
    }

    fn into_vec(self) -> Vec<TypeInstantiationKey> {
        self.ordered
    }
}

impl MonomorphRequests {
    fn record(&mut self, request: MonomorphRequest) {
        if self.seen.insert(request.clone()) {
            self.ordered.push(request);
        }
    }

    fn into_vec(self) -> Vec<MonomorphRequest> {
        self.ordered
    }
}

fn implicit_builtin_type_fqn(local_or_fqn: &str) -> Option<&'static str> {
    match local_or_fqn {
        // allow both `Int` and `scoop.core.Int` spellings
        "Any" | "scoop.core.Any" => Some("scoop.core.Any"),
        "String" | "scoop.core.String" => Some("scoop.core.String"),
        "Unit" | "scoop.core.Unit" => Some("scoop.core.Unit"),
        "Nothing" | "scoop.core.Nothing" => Some("scoop.core.Nothing"),
        "Bool" | "scoop.core.Bool" => Some("scoop.core.Bool"),
        "Char" | "scoop.core.Char" => Some("scoop.core.Char"),
        "Float64" | "scoop.core.Float64" => Some("scoop.core.Float64"),
        "Float32" | "scoop.core.Float32" => Some("scoop.core.Float32"),
        "Int" | "scoop.core.Int" => Some("scoop.core.Int"),
        "UInt" | "scoop.core.UInt" => Some("scoop.core.UInt"),
        "Int8" | "scoop.core.Int8" => Some("scoop.core.Int8"),
        "Int16" | "scoop.core.Int16" => Some("scoop.core.Int16"),
        "Int32" | "scoop.core.Int32" => Some("scoop.core.Int32"),
        "Int64" | "scoop.core.Int64" => Some("scoop.core.Int64"),
        "UInt8" | "scoop.core.UInt8" => Some("scoop.core.UInt8"),
        "UInt16" | "scoop.core.UInt16" => Some("scoop.core.UInt16"),
        "UInt32" | "scoop.core.UInt32" => Some("scoop.core.UInt32"),
        "UInt64" | "scoop.core.UInt64" => Some("scoop.core.UInt64"),
        "Option" | "scoop.core.Option" => Some("scoop.core.Option"),
        "Continuation" | "scoop.core.Continuation" => Some("scoop.core.Continuation"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VariancePos {
    /// out-position（协变位置）：例如返回类型、`val` 属性类型。
    Out,
    /// in-position（逆变位置）：例如参数类型、receiver 类型。
    In,
    /// in+out（不变位置）：例如 `var` 属性类型。
    Invariant,
}

impl VariancePos {
    fn as_str(self) -> &'static str {
        match self {
            VariancePos::Out => "out",
            VariancePos::In => "in",
            VariancePos::Invariant => "invariant",
        }
    }

    fn flip(self) -> Self {
        match self {
            VariancePos::Out => VariancePos::In,
            VariancePos::In => VariancePos::Out,
            VariancePos::Invariant => VariancePos::Invariant,
        }
    }

    fn compose(self, declared: Option<ast::TypeParamVariance>) -> Self {
        let Some(declared) = declared else {
            // invariant type parameter：无论外部位置如何，都会把内部位置“压扁”为 invariant。
            return VariancePos::Invariant;
        };

        // Kotlin-like variance composition：
        // - out(+1) / in(-1)；组合是乘法；invariant(0) 会使整体变 invariant。
        match (self, declared) {
            (VariancePos::Invariant, _) => VariancePos::Invariant,
            (_, ast::TypeParamVariance::Out) => self,
            (VariancePos::Out, ast::TypeParamVariance::In) => VariancePos::In,
            (VariancePos::In, ast::TypeParamVariance::In) => VariancePos::Out,
        }
    }
}

fn check_arity(fqn: &str, expected: usize, found: usize, span: Span) -> Result<(), TypeLowerError> {
    if expected == found {
        return Ok(());
    }
    Err(TypeLowerError::TypeArityMismatch {
        name: fqn.to_string(),
        expected,
        found,
        span: span.into(),
    })
}

fn is_int_const_expr(expr: &ast::Expr) -> bool {
    match &expr.kind {
        ast::ExprKind::IntLit => true,
        ast::ExprKind::Unary {
            op: ast::UnaryOp::Neg,
            expr: inner,
            ..
        } => matches!(inner.kind, ast::ExprKind::IntLit),
        _ => false,
    }
}
