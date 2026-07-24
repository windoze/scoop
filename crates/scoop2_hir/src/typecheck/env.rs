//! [`TypeEnv`]：typecheck 的类型查询环境。
//!
//! 持有 [`TypeStore`]（类型存储）、对 resolve [`Index`](crate::resolve::Index) 与
//! [`Interner`] 的引用，并提供**内建类型表**（标量 / String / Unit / Nothing）。
//! nominal 类型（class/struct/enum/...）的 ref/value 由 [`Index::category`] 决定。

use std::collections::{HashMap, HashSet};

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{FileId, Interner, Symbol};

use crate::resolve::imports::ImportTable;
use crate::resolve::index::Index;
use crate::syntax::ast::{File, ItemKind, TypeMember, TypeMemberKind, TypeParamList, ValBinding};
use crate::ty::{TypeId, TypeParamType, TypeStore};

use super::lower::TypeLowering;

/// 一个函数签名（已降级；M2 用于单候选调用）。
#[derive(Clone, Debug)]
pub struct Signature {
    pub params: Vec<TypeId>,
    pub return_ty: TypeId,
    /// 类型参数个数（>0 表示泛型；M3 才支持实例化）。
    pub type_param_count: usize,
    /// 参数名（与 params 等长）。
    pub param_names: Vec<Symbol>,
}

/// 顶层函数的注解属性（release-hook 等 cross-reference 校验用）。
#[derive(Clone, Copy, Default, Debug)]
pub struct FunAttrs {
    pub is_nogc: bool,
    /// `@Extern` 且 ABI 为缺省或 `"c"`（native C ABI leaf）。
    pub is_native_extern: bool,
}

/// typecheck 类型环境。
pub struct TypeEnv<'i> {
    pub store: TypeStore,
    pub index: &'i Index,
    pub interner: &'i Interner,
    /// FQN → 函数签名重载集（顶层函数；M2）。
    signatures: HashMap<Symbol, Vec<Signature>>,
    /// 类型 FQN → (成员名 → 成员类型)。属性 / 字段（含主构造 param-property）。
    members: HashMap<Symbol, HashMap<Symbol, TypeId>>,
    /// 类型 FQN → 主构造参数类型列表。
    ctors: HashMap<Symbol, Vec<TypeId>>,
    /// 类型 FQN → (方法名 → 签名重载集)。成员函数（含扩展）。
    member_signatures: HashMap<Symbol, HashMap<Symbol, Vec<Signature>>>,
    /// 带 `@CLayout` 注解的 struct FQN 集合（native `@Extern` ABI 允许的 nominal 值类型）。
    clayout_structs: HashSet<Symbol>,
    /// 顶层函数 FQN → 注解属性（release-hook cross-reference 校验用）。
    fun_attrs: HashMap<Symbol, FunAttrs>,
    /// typealias FQN → (RHS TypeRef, 类型参数名序列)。
    type_aliases: HashMap<Symbol, (crate::syntax::ast::TypeRef, Vec<Symbol>)>,
    /// 类型 FQN → (类型参数名序列, where 约束 [(参数名, bound)])。
    #[allow(clippy::type_complexity)]
    type_constraints:
        HashMap<Symbol, (Vec<Symbol>, Vec<(Symbol, crate::syntax::ast::GenericBound)>)>,
    /// 顶层 val/var 简单名 → 已降级类型（供表达式引用解析）。
    top_level_vals: HashMap<Symbol, TypeId>,
    /// enum FQN → variant 名列表（when 穷尽性检查用）。
    enum_variants: HashMap<Symbol, Vec<Symbol>>,
}

impl<'i> TypeEnv<'i> {
    pub fn new(index: &'i Index, interner: &'i Interner) -> Self {
        Self {
            store: TypeStore::new(),
            index,
            interner,
            signatures: HashMap::new(),
            members: HashMap::new(),
            ctors: HashMap::new(),
            member_signatures: HashMap::new(),
            clayout_structs: HashSet::new(),
            fun_attrs: HashMap::new(),
            type_aliases: HashMap::new(),
            type_constraints: HashMap::new(),
            top_level_vals: HashMap::new(),
            enum_variants: HashMap::new(),
        }
    }

    /// nominal 子类型（传递超类型链）。
    pub fn fqn_is_subtype(&self, sub: Symbol, sup: Symbol) -> bool {
        if sub == sup {
            return true;
        }
        self.index
            .supertypes_of(sub)
            .iter()
            .any(|&s| self.fqn_is_subtype(s, sup))
    }

    /// 顶层 val 类型查询（表达式引用解析用）。
    pub fn top_level_val(&self, name: Symbol) -> Option<TypeId> {
        self.top_level_vals.get(&name).copied()
    }

    /// enum variant 列表查询（when 穷尽性用）。
    pub fn enum_variants(&self, fqn: Symbol) -> Option<&[Symbol]> {
        self.enum_variants.get(&fqn).map(|v| v.as_slice())
    }

    /// 类型约束查询（where-satisfaction 用）。
    #[allow(clippy::type_complexity)]
    pub fn type_constraints(
        &self,
        fqn: Symbol,
    ) -> Option<(&[Symbol], &[(Symbol, crate::syntax::ast::GenericBound)])> {
        self.type_constraints
            .get(&fqn)
            .map(|(n, c)| (n.as_slice(), c.as_slice()))
    }

    /// typealias 查询（展开用）。
    pub fn type_alias(&self, fqn: Symbol) -> Option<(&crate::syntax::ast::TypeRef, &[Symbol])> {
        self.type_aliases.get(&fqn).map(|(t, p)| (t, p.as_slice()))
    }

    /// `@CLayout` struct 查询（native `@Extern` ABI 用）。
    pub fn is_clayout_struct(&self, fqn: Symbol) -> bool {
        self.clayout_structs.contains(&fqn)
    }

    /// 顶层函数注解属性查询（release-hook 用）。
    pub fn fun_attrs(&self, fqn: Symbol) -> Option<FunAttrs> {
        self.fun_attrs.get(&fqn).copied()
    }

    /// 顶层函数（非扩展、非成员）的签名重载集。
    pub fn signatures(&self, fqn: Symbol) -> Option<&[Signature]> {
        self.signatures.get(&fqn).map(|v| v.as_slice())
    }

    /// 类型上的成员函数签名重载集（`type_fqn.method_name`）。
    pub fn member_signatures(&self, type_fqn: Symbol, method: Symbol) -> Option<&[Signature]> {
        self.member_signatures
            .get(&type_fqn)
            .and_then(|m| m.get(&method).map(|v| v.as_slice()))
    }

    /// 类型的主构造参数类型列表。
    pub fn ctor_params(&self, fqn: Symbol) -> Option<&[TypeId]> {
        self.ctors.get(&fqn).map(|v| v.as_slice())
    }

    /// 类型的属性 / 字段成员类型（`type_fqn.member_name`）。
    pub fn member_type(&self, type_fqn: Symbol, member_name: Symbol) -> Option<TypeId> {
        self.members
            .get(&type_fqn)
            .and_then(|m| m.get(&member_name).copied())
    }

    /// 类型的全部字段成员类型（GC-free 递归校验用）。
    pub fn member_types(&self, type_fqn: Symbol) -> Option<&HashMap<Symbol, TypeId>> {
        self.members.get(&type_fqn)
    }

    /// 内建标量 / String / Unit / Nothing 名字 → [`TypeId`]。
    /// 接受短名（`Int`）或全限定（`scoop.core.Int`）。
    /// 不含 `Option`/`Array`——它们是 prelude nominal，由 [`super::lower`] 经 Index 解析。
    pub fn builtin(&mut self, name: &str) -> Option<TypeId> {
        let n = name.strip_prefix("scoop.core.").unwrap_or(name);
        let s = &mut self.store;
        Some(match n {
            "Int" => s.int(),
            "UInt" | "UIntPtr" => s.uint(),
            "Bool" => s.bool(),
            "Char" => s.char(),
            "Float64" | "Double" => s.float64(),
            "Float32" => s.float32(),
            "Int8" => s.int_n(8),
            "Int16" | "Short" => s.int_n(16),
            "Int32" => s.int_n(32),
            "Int64" | "Long" => s.int_n(64),
            "UInt8" | "Byte" => s.uint_n(8),
            "UInt16" | "UShort" => s.uint_n(16),
            "UInt32" => s.uint_n(32),
            "UInt64" | "ULong" => s.uint_n(64),
            "String" => s.string(),
            "Unit" => s.unit(),
            "Nothing" => s.nothing(),
            _ => return None,
        })
    }

    /// nominal 类型是否引用类型（按 [`Index::category`]）。
    pub fn is_reference_nominal(&self, fqn: Symbol) -> bool {
        self.index.category(fqn).is_some_and(|c| c.is_reference())
    }
}

/// 把文件的**顶层函数**（非扩展、非成员）签名降级并登记进 `env.signatures`。
/// 成员函数 / 构造器 / 扩展函数的签名在成员调用里程碑补齐。
pub fn register_top_level_signatures(
    env: &mut TypeEnv,
    file: &File,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        let ItemKind::Fun(d) = &item.kind else {
            continue;
        };
        if d.receiver.is_some() {
            continue; // 扩展函数：M2 暂不
        }
        let name_text = env.interner.resolve(d.name.symbol);
        let fqn_text = if package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{package_prefix}.{name_text}")
        };
        let Some(fqn) = env.interner.get(&fqn_text) else {
            continue;
        };
        // 类型参数映射（用于降低签名中的类型参数引用）。
        let tp_map: HashMap<Symbol, TypeParamType> = d
            .type_params
            .iter()
            .flat_map(|tpl| tpl.params.iter())
            .map(|p| {
                (
                    p.name.symbol,
                    TypeParamType {
                        name: p.name.symbol,
                        file: FileId(0),
                        span: p.name.span,
                    },
                )
            })
            .collect();
        let tpc = tp_map.len();
        let unit_ty = env.store.unit();
        let sig = {
            let mut lower =
                TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
            let params: Vec<TypeId> = d
                .params
                .iter()
                .map(|p| match &p.ty {
                    Some(t) => lower.lower(t),
                    None => unit_ty,
                })
                .collect();
            let return_ty = match &d.return_ty {
                Some(t) => lower.lower(t),
                None => unit_ty,
            };
            Signature {
                param_names: d.params.iter().map(|p| p.name.symbol).collect(),
                params,
                return_ty,
                type_param_count: tpc,
            }
        };
        env.signatures.entry(fqn).or_default().push(sig);
        env.fun_attrs.insert(
            fqn,
            FunAttrs {
                is_nogc: has_annotation(&d.annotations, "NoGC", env.interner),
                is_native_extern: is_native_c_extern(&d.annotations, env.interner),
            },
        );
    }
}

/// 注解路径末段文本匹配。
fn has_annotation(
    anns: &[crate::syntax::ast::AnnotationUse],
    name: &str,
    interner: &Interner,
) -> bool {
    anns.iter().any(|a| {
        a.path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == name)
    })
}

/// `@Extern` 且 ABI 为缺省或 `"c"`（native leaf）。
fn is_native_c_extern(anns: &[crate::syntax::ast::AnnotationUse], interner: &Interner) -> bool {
    use crate::syntax::ast::ExprKind;
    let Some(ext) = anns.iter().find(|a| {
        a.path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == "Extern")
    }) else {
        return false;
    };
    // 若给出 `abi=`，必须是 "c"。
    for arg in &ext.args {
        if arg
            .name
            .as_ref()
            .is_some_and(|n| interner.resolve(n.symbol) == "abi")
        {
            if let ExprKind::StringLit(s) = &arg.value.kind {
                return s.value.trim().eq_ignore_ascii_case("c");
            }
            return false;
        }
    }
    true
}

/// 把文件的类型 / object 的**属性成员**（含主构造 param-property）类型降级并登记进
/// `env.members`。成员函数 / variant 不在此（它们不是值成员读取的目标）。
pub fn register_members(
    env: &mut TypeEnv,
    file: &File,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        match &item.kind {
            ItemKind::Type(d) => {
                let owner = fqn_of(env, package_prefix, d.name.symbol);
                // 主构造 param-property（`class C(val x: T)`）。
                if let Some(ctor) = &d.primary_ctor {
                    for cp in &ctor.params {
                        if cp.property.is_some()
                            && let Some(ty) = &cp.ty
                        {
                            lower_and_store_member(
                                env,
                                owner,
                                cp.name.symbol,
                                ty,
                                imports,
                                package_prefix,
                                d.type_params.as_ref(),
                                diags,
                            );
                        }
                    }
                }
                if let Some(body) = &d.body {
                    register_body_members(
                        env,
                        owner,
                        &body.members,
                        d.type_params.as_ref(),
                        imports,
                        package_prefix,
                        diags,
                    );
                }
            }
            ItemKind::Object(d) => {
                if let Some(name) = &d.name
                    && let Some(body) = &d.body
                {
                    let owner = fqn_of(env, package_prefix, name.symbol);
                    register_body_members(
                        env,
                        owner,
                        &body.members,
                        None,
                        imports,
                        package_prefix,
                        diags,
                    );
                }
            }
            _ => {}
        }
    }
}

/// 登记类型体成员：属性 → 成员类型；嵌套类型 / object 递归；companion 成员挂到 owner。
fn register_body_members(
    env: &mut TypeEnv,
    owner: Symbol,
    members: &[TypeMember],
    type_params: Option<&TypeParamList>,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for m in members {
        match &m.kind {
            TypeMemberKind::Property(d) => {
                if let Some(ty) = &d.ty {
                    lower_and_store_member(
                        env,
                        owner,
                        d.name.symbol,
                        ty,
                        imports,
                        package_prefix,
                        type_params,
                        diags,
                    );
                }
                // 无类型标注的属性（推断）→ M2 暂不登记（需 init 推断，后续里程碑）。
            }
            TypeMemberKind::Object(d) => {
                if d.companion {
                    if let Some(b) = &d.body {
                        register_body_members(
                            env,
                            owner,
                            &b.members,
                            None,
                            imports,
                            package_prefix,
                            diags,
                        );
                    }
                } else if let Some(name) = &d.name
                    && let Some(b) = &d.body
                {
                    let nested = fqn_under(env, owner, name.symbol);
                    register_body_members(
                        env,
                        nested,
                        &b.members,
                        None,
                        imports,
                        package_prefix,
                        diags,
                    );
                }
            }
            TypeMemberKind::Type(d) => {
                if let Some(b) = &d.body {
                    let nested = fqn_under(env, owner, d.name.symbol);
                    register_body_members(
                        env,
                        nested,
                        &b.members,
                        d.type_params.as_ref(),
                        imports,
                        package_prefix,
                        diags,
                    );
                }
            }
            TypeMemberKind::Fun(d) => {
                let tp_map = build_tp_map(type_params);
                let unit_ty = env.store.unit();
                let sig = {
                    let mut lower =
                        TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
                    let params: Vec<TypeId> = d
                        .params
                        .iter()
                        .map(|p| match &p.ty {
                            Some(t) => lower.lower(t),
                            None => unit_ty,
                        })
                        .collect();
                    let return_ty = match &d.return_ty {
                        Some(t) => lower.lower(t),
                        None => unit_ty,
                    };
                    Signature {
                        param_names: d.params.iter().map(|p| p.name.symbol).collect(),
                        params,
                        return_ty,
                        type_param_count: 0,
                    }
                };
                env.member_signatures
                    .entry(owner)
                    .or_default()
                    .entry(d.name.symbol)
                    .or_default()
                    .push(sig);
            }
            TypeMemberKind::EnumVariant(_)
            | TypeMemberKind::InitBlock(_)
            | TypeMemberKind::SecondaryCtor(_) => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_and_store_member(
    env: &mut TypeEnv,
    owner: Symbol,
    name: Symbol,
    ty: &crate::syntax::ast::TypeRef,
    imports: &ImportTable,
    package_prefix: &str,
    type_params: Option<&TypeParamList>,
    diags: &mut DiagnosticSink,
) {
    let tp_map = build_tp_map(type_params);
    let lowered = {
        let mut lower = TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
        lower.lower(ty)
    };
    env.members.entry(owner).or_default().insert(name, lowered);
}

fn build_tp_map(tpl: Option<&TypeParamList>) -> HashMap<Symbol, TypeParamType> {
    let mut map = HashMap::new();
    if let Some(tpl) = tpl {
        for p in &tpl.params {
            map.insert(
                p.name.symbol,
                TypeParamType {
                    name: p.name.symbol,
                    file: FileId(0),
                    span: p.name.span,
                },
            );
        }
    }
    map
}

fn fqn_of(env: &TypeEnv, package_prefix: &str, name: Symbol) -> Symbol {
    let name_text = env.interner.resolve(name);
    let fqn_text = if package_prefix.is_empty() {
        name_text.to_string()
    } else {
        format!("{package_prefix}.{name_text}")
    };
    env.interner.get(&fqn_text).unwrap_or(name)
}

fn fqn_under(env: &TypeEnv, owner: Symbol, name: Symbol) -> Symbol {
    let owner_text = env.interner.resolve(owner);
    let name_text = env.interner.resolve(name);
    env.interner
        .get(&format!("{owner_text}.{name_text}"))
        .unwrap_or(name)
}

/// 收集 enum 的 variant 名（FQN → [variant_name]），供 when 穷尽性检查。
pub fn register_enum_variants(env: &mut TypeEnv, file: &File, package_prefix: &str) {
    for item in &file.items {
        let ItemKind::Type(d) = &item.kind else {
            continue;
        };
        if !matches!(d.kind, crate::syntax::ast::TypeKind::Enum) {
            continue;
        }
        let fqn = fqn_of(env, package_prefix, d.name.symbol);
        let mut variants: Vec<Symbol> = Vec::new();
        if let Some(body) = &d.body {
            for m in &body.members {
                if let TypeMemberKind::EnumVariant(ev) = &m.kind {
                    variants.push(ev.name.symbol);
                }
            }
        }
        env.enum_variants.insert(fqn, variants);
    }
}

/// 收集顶层 val/var 的类型（简单名 → TypeId），供表达式引用解析。
pub fn register_top_level_vals(
    env: &mut TypeEnv,
    file: &File,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        let ItemKind::Val(d) = &item.kind else {
            continue;
        };
        let ValBinding::Name(name) = &d.binding else {
            continue;
        };
        if let Some(ty_ref) = &d.ty {
            let ty = {
                let mut lower = TypeLowering::new(
                    env,
                    imports,
                    HashMap::new(),
                    package_prefix.to_string(),
                    diags,
                );
                lower.lower(ty_ref)
            };
            env.top_level_vals.insert(name.symbol, ty);
        }
    }
}

/// 收集类型/函数的 where 约束（FQN → (参数名序列, 约束)），供类型实参满足性检查。
pub fn register_type_constraints(env: &mut TypeEnv, file: &File, package_prefix: &str) {
    use crate::syntax::ast::{GenericBound, ItemKind, TypeParamList, WhereClause};
    let collect_one = |env: &mut TypeEnv,
                       name: Symbol,
                       type_params: Option<&TypeParamList>,
                       where_clause: Option<&WhereClause>| {
        let owner = fqn_of(env, package_prefix, name);
        let param_names: Vec<Symbol> = type_params
            .map(|tp| tp.params.iter().map(|p| p.name.symbol).collect())
            .unwrap_or_default();
        let mut constraints: Vec<(Symbol, GenericBound)> = Vec::new();
        if let Some(wc) = where_clause {
            for c in &wc.constraints {
                constraints.push((c.name.symbol, c.bound.clone()));
            }
        }
        if let Some(tp) = type_params {
            for p in &tp.params {
                if let Some(b) = &p.bound {
                    constraints.push((p.name.symbol, b.clone()));
                }
            }
        }
        env.type_constraints
            .insert(owner, (param_names, constraints));
    };
    for item in &file.items {
        match &item.kind {
            ItemKind::Type(d) => {
                collect_one(
                    env,
                    d.name.symbol,
                    d.type_params.as_ref(),
                    d.where_clause.as_ref(),
                );
            }
            ItemKind::Fun(d) if d.receiver.is_none() => {
                collect_one(
                    env,
                    d.name.symbol,
                    d.type_params.as_ref(),
                    d.where_clause.as_ref(),
                );
            }
            _ => {}
        }
    }
}

/// 收集文件中的 typealias（FQN → RHS + 类型参数名），供 lower 展开。
pub fn register_type_aliases(env: &mut TypeEnv, file: &File, package_prefix: &str) {
    for item in &file.items {
        let ItemKind::TypeAlias(d) = &item.kind else {
            continue;
        };
        let owner = fqn_of(env, package_prefix, d.name.symbol);
        let params: Vec<Symbol> = d
            .type_params
            .as_ref()
            .map(|tp| tp.params.iter().map(|p| p.name.symbol).collect())
            .unwrap_or_default();
        env.type_aliases.insert(owner, (d.ty.clone(), params));
    }
}

/// 收集文件中带 `@CLayout` 注解的 struct FQN（native `@Extern` ABI 允许的 nominal 值类型）。
pub fn register_clayout_structs(env: &mut TypeEnv, file: &File, package_prefix: &str) {
    for item in &file.items {
        let ItemKind::Type(d) = &item.kind else {
            continue;
        };
        if d.kind != crate::syntax::ast::TypeKind::Struct {
            continue;
        }
        if d.annotations
            .iter()
            .any(|a| annotation_last_text(a, env.interner) == Some("CLayout"))
        {
            env.clayout_structs
                .insert(fqn_of(env, package_prefix, d.name.symbol));
        }
    }
}

/// 注解路径末段文本（用于 `@CLayout` 等内建注解识别）。
fn annotation_last_text<'i>(
    ann: &crate::syntax::ast::AnnotationUse,
    interner: &'i Interner,
) -> Option<&'i str> {
    ann.path.segments.last().map(|s| interner.resolve(s.symbol))
}

/// 把类型的主构造参数类型降级并登记进 `env.ctors`（用于 `Type(args)` 构造器调用）。
pub fn register_constructors(
    env: &mut TypeEnv,
    file: &File,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        let ItemKind::Type(d) = &item.kind else {
            continue;
        };
        let owner = fqn_of(env, package_prefix, d.name.symbol);
        if let Some(ctor) = &d.primary_ctor {
            let tp_map = build_tp_map(d.type_params.as_ref());
            let unit_ty = env.store.unit();
            let params: Vec<TypeId> = ctor
                .params
                .iter()
                .map(|cp| match &cp.ty {
                    Some(t) => {
                        let mut lower = TypeLowering::new(
                            env,
                            imports,
                            tp_map.clone(),
                            package_prefix.to_string(),
                            diags,
                        );
                        lower.lower(t)
                    }
                    None => unit_ty,
                })
                .collect();
            env.ctors.insert(owner, params);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::index::Index;

    #[test]
    fn builtin_scalars_and_qualified_form() {
        let idx = Index::new();
        let it = Interner::new();
        let mut env = TypeEnv::new(&idx, &it);
        let i = env.builtin("Int").unwrap();
        let i2 = env.builtin("scoop.core.Int").unwrap();
        assert_eq!(i, i2, "short and qualified resolve to same TypeId");
        assert!(env.store.is_value(i));
        let s = env.builtin("String").unwrap();
        assert!(env.store.is_reference(s));
        let u = env.builtin("Unit").unwrap();
        assert!(env.store.is_unit(u));
        let n = env.builtin("Nothing").unwrap();
        assert!(env.store.is_nothing(n));
        assert!(env.builtin("NotAType").is_none());
        assert_eq!(env.builtin("Byte").unwrap(), env.builtin("UInt8").unwrap());
        assert_eq!(
            env.builtin("Double").unwrap(),
            env.builtin("Float64").unwrap()
        );
        let _ = it;
    }
}
