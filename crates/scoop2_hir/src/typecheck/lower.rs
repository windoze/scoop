//! [`TypeLowering`]：`ast::TypeRef → TypeId`（类型引用降级）。
//!
//! 复用 resolve 阶段已验证的名字解析（builtin / 类型参数 / 当前包顶层 / import），
//! 把每个 `TypeRef` 降级为 [`TypeStore`](crate::ty::TypeStore) 中的 [`TypeId`]。
//! nominal 的 ref/value 由 [`Index::category`](crate::resolve::Index::category) 决定。
//!
//! 当前覆盖：内建标量 / String / Unit / Nothing；类型参数；`Option<T>`（与 `T?`
//! 一致，降级为 `Option(TypeId)`）；用户 nominal（ref/value）；元组 / 函数类型 /
//! 接收者函数类型 / 可空。effect 行降级暂为 `Pure`（M7 增量补齐）。

use std::collections::HashMap;

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{Interner, Span, Symbol};

use crate::resolve::imports::ImportTable;
use crate::syntax::ast::{TypeArg, TypeArgKind, TypePath, TypeRef, TypeRefKind};
use crate::ty::{EffectRow, FunctionType, NominalType, TypeId, TypeKind, TypeParamType};

use super::diagnostics;
use super::env::TypeEnv;

/// 类型引用降级器。
pub struct TypeLowering<'a, 'i> {
    env: &'a mut TypeEnv<'i>,
    imports: &'a ImportTable,
    /// 当前声明的类型参数：名字 → 身份。
    type_params: HashMap<Symbol, TypeParamType>,
    package_prefix: String,
    diags: &'a mut DiagnosticSink,
    /// typealias 展开期间的类型参数替换（别名参数 → 实参 TypeId）。
    subst: HashMap<Symbol, TypeId>,
    /// typealias 展开深度（递归环保险）。
    alias_depth: u32,
}

impl<'a, 'i> TypeLowering<'a, 'i> {
    pub fn new(
        env: &'a mut TypeEnv<'i>,
        imports: &'a ImportTable,
        type_params: HashMap<Symbol, TypeParamType>,
        package_prefix: String,
        diags: &'a mut DiagnosticSink,
    ) -> Self {
        Self {
            env,
            imports,
            type_params,
            package_prefix,
            diags,
            subst: HashMap::new(),
            alias_depth: 0,
        }
    }

    /// 降级一个 `TypeRef`。无法解析时记录诊断并返回 `Nothing`（bottom，宽容降级）。
    pub fn lower(&mut self, ty: &TypeRef) -> TypeId {
        match &ty.kind {
            TypeRefKind::Path { path, args } => self.lower_path(path, args, ty.span),
            TypeRefKind::Unit => self.env.store.unit(),
            TypeRefKind::Tuple(elems) => {
                let elems: Vec<TypeId> = elems.iter().map(|e| self.lower(e)).collect();
                self.env.store.tuple(elems)
            }
            TypeRefKind::Function {
                params,
                ret,
                effect,
            } => {
                let params: Vec<TypeId> = params.iter().map(|p| self.lower(p)).collect();
                let ret = self.lower(ret);
                let effects = self.lower_effect_row(effect.as_ref());
                self.env.store.function(FunctionType {
                    receiver: None,
                    params,
                    return_ty: ret,
                    effects,
                    closed: effect.as_ref().is_some_and(|e| e.closed.is_some()),
                })
            }
            TypeRefKind::ReceiverFunction {
                receiver,
                params,
                ret,
                effect,
            } => {
                let receiver = self.lower(receiver);
                let params: Vec<TypeId> = params.iter().map(|p| self.lower(p)).collect();
                let ret = self.lower(ret);
                let effects = self.lower_effect_row(effect.as_ref());
                self.env.store.function(FunctionType {
                    receiver: Some(receiver),
                    params,
                    return_ty: ret,
                    effects,
                    closed: effect.as_ref().is_some_and(|e| e.closed.is_some()),
                })
            }
            TypeRefKind::Nullable(inner) => {
                let inner = self.lower(inner);
                self.env.store.option(inner)
            }
        }
    }

    /// 宽容降级 effect 行：每项解析为 nominal TypeId（不报 unresolved_type_ref）。
    /// 未解析的项跳过（不计入行）；`Pure` / 空行 → 空 EffectRow。
    fn lower_effect_row(
        &mut self,
        effect: Option<&crate::syntax::ast::EffectRowExpr>,
    ) -> EffectRow {
        let Some(eff) = effect else {
            return EffectRow::pure();
        };
        let mut terms: Vec<TypeId> = Vec::new();
        for term in &eff.terms {
            // Pure 项不计入行。
            if term
                .path
                .segments
                .last()
                .is_some_and(|s| self.env.interner.resolve(s.symbol) == "Pure")
            {
                continue;
            }
            // 尝试解析为 nominal（不带类型实参——effect 行按 FQN 短名比较）。
            if let Some(fqn) = self.resolve_type_fqn(&term.path) {
                let nominal = NominalType {
                    fqn,
                    args: vec![],
                    eff: None,
                };
                let ty = if self.env.is_reference_nominal(fqn) {
                    self.env.store.ref_nominal(nominal)
                } else {
                    self.env.store.value_nominal(nominal)
                };
                terms.push(ty);
            }
            // 未解析 → 跳过（宽容，不报错）。
        }
        EffectRow::from_terms(terms)
    }

    fn lower_path(&mut self, path: &TypePath, args: &[TypeArg], span: Span) -> TypeId {
        let name_text = path_text(path, self.env.interner);

        // 1. 内建标量 / String / Unit / Nothing / Any（无类型实参）。
        if args.is_empty() {
            if let Some(b) = self.env.builtin(&name_text) {
                return b;
            }
            // Any → Ref(RefTypeKind::Any)（spec P2 §2.1 根引用类型）。
            let stripped = name_text.strip_prefix("scoop.core.").unwrap_or(&name_text);
            if stripped == "Any" {
                return self.env.store.any();
            }
        }

        // 2. Option<T>（与 T? 一致）。
        if is_option_name(&name_text) {
            let inner = self
                .lower_type_args_one(args, span)
                .unwrap_or_else(|| self.env.store.nothing());
            return self.env.store.option(inner);
        }

        // 3. 类型参数（含 typealias 展开期间的实参替换）。
        if path.segments.len() == 1 {
            let sym = path.segments[0].symbol;
            if let Some(sub) = self.subst.get(&sym).copied() {
                return sub;
            }
            if let Some(tp) = self.type_params.get(&sym) {
                return self.env.store.param(*tp);
            }
        }

        // 4. nominal：解析 FQN，按 category 决定 ref/value。
        let Some(fqn) = self.resolve_type_fqn(path) else {
            self.diags
                .push(diagnostics::unresolved_type_ref(&name_text, span));
            return self.env.store.nothing();
        };
        // 5. typealias 展开：FQN 是 typealias 时降级其 RHS（类型实参绑定到别名参数）。
        if self.alias_depth < 32
            && let Some((rhs_ref, param_names_ref)) = self.env.type_alias(fqn)
        {
            let rhs = rhs_ref.clone();
            let param_names: Vec<Symbol> = param_names_ref.to_vec();
            let arg_types: Vec<TypeId> = args
                .iter()
                .filter_map(|a| match &a.kind {
                    TypeArgKind::Type(t) => Some(self.lower(t)),
                    _ => None,
                })
                .collect();
            let saved_subst = std::mem::take(&mut self.subst);
            for (i, name) in param_names.iter().enumerate() {
                if let Some(arg) = arg_types.get(i) {
                    self.subst.insert(*name, *arg);
                }
            }
            self.alias_depth += 1;
            let expanded = self.lower(&rhs);
            self.alias_depth -= 1;
            self.subst = saved_subst;
            return expanded;
        }
        let lowered_args: Vec<TypeId> = args
            .iter()
            .filter_map(|a| match &a.kind {
                TypeArgKind::Type(t) => Some(self.lower(t)),
                _ => None, // Star / Effect 实参：M0 不降级。
            })
            .collect();
        // 6. where 约束满足性检查：类型实参必须满足声明处的 where / 直接 bound。
        self.check_type_arg_constraints(fqn, &lowered_args, span);
        // Continuation legacy shorthand 检查：必须有至少 2 个非 eff 类型实参。
        let name = self.env.interner.resolve(fqn);
        let stripped = name.strip_prefix("scoop.core.").unwrap_or(name);
        if stripped == "Continuation" {
            let non_eff_count = args
                .iter()
                .filter(|a| !matches!(a.kind, TypeArgKind::Effect(_)))
                .count();
            let has_eff_arg = args
                .iter()
                .any(|a| matches!(a.kind, TypeArgKind::Effect(_)));
            if non_eff_count < 2 {
                if has_eff_arg {
                    self.diags.push(
                        super::diagnostics::continuation_legacy_effect_shorthand_removed(span),
                    );
                } else {
                    self.diags
                        .push(super::diagnostics::continuation_legacy_pure_shorthand_removed(span));
                }
            }
        }
        // 提取 use-site eff 实参（`Disposable<eff Async>`）→ NominalType.eff。
        let nominal_eff = args.iter().find_map(|a| match &a.kind {
            TypeArgKind::Effect(e) => Some(self.lower_effect_row(Some(e))),
            _ => None,
        });
        // use-site eff 实参只能在声明了 eff 形参的类型上使用。
        if nominal_eff.is_some() && !self.env.has_eff_param(fqn) {
            let name = self.env.interner.resolve(fqn);
            let stripped = name.strip_prefix("scoop.core.").unwrap_or(name);
            // Continuation 是 compiler-owned 但声明了 eff param（prelude）。
            if stripped != "Continuation" {
                self.diags
                    .push(super::diagnostics::use_site_eff_arg_not_allowed(span));
            }
        }
        let nominal = NominalType {
            fqn,
            args: lowered_args,
            eff: nominal_eff,
        };
        if self.env.is_reference_nominal(fqn) {
            self.env.store.ref_nominal(nominal)
        } else {
            self.env.store.value_nominal(nominal)
        }
    }

    /// 检查类型实参是否满足声明处的 where 约束（保守：仅在确定违反时报错）。
    fn check_type_arg_constraints(&mut self, fqn: Symbol, args: &[TypeId], span: Span) {
        use crate::syntax::ast::GenericBound;
        let Some((param_names, constraints)) = self.env.type_constraints(fqn) else {
            return;
        };
        let param_names: Vec<Symbol> = param_names.to_vec();
        let constraints: Vec<(Symbol, GenericBound)> = constraints.to_vec();
        for (name, bound) in &constraints {
            let Some(idx) = param_names.iter().position(|n| n == name) else {
                continue;
            };
            let Some(&arg) = args.get(idx) else {
                continue;
            };
            let violated = match bound {
                GenericBound::Ref(_) => !matches!(self.env.store.kind(arg), TypeKind::Ref(_)),
                GenericBound::Value(_) => !matches!(self.env.store.kind(arg), TypeKind::Value(_)),
                GenericBound::Type(t) => {
                    // 实参必须是 bound 类型的子类型。
                    let bound_ty = {
                        let mut lower = TypeLowering::new(
                            self.env,
                            self.imports,
                            HashMap::new(),
                            self.package_prefix.clone(),
                            self.diags,
                        );
                        lower.lower(t)
                    };
                    !self.arg_satisfies_bound(arg, bound_ty)
                }
            };
            if violated {
                self.diags.push(diagnostics::where_constraint_not_satisfied(
                    &bound_desc(bound, self.env.interner),
                    span,
                ));
                return;
            }
        }
    }

    /// 实参是否满足 bound 类型（子类型 / 相等 / TypeParam lenient）。
    fn arg_satisfies_bound(&self, arg: TypeId, bound: TypeId) -> bool {
        if arg == bound {
            return true;
        }
        let ak = self.env.store.kind(arg);
        let bk = self.env.store.kind(bound);
        // TypeParam：lenient（泛型推迟）。
        if matches!(ak, TypeKind::Param(_)) || matches!(bk, TypeKind::Param(_)) {
            return true;
        }
        // Nothing 是唯一可以作为任何类型子类型的值类型（bottom type）。
        if self.env.store.is_nothing(arg) {
            return true;
        }
        // nominal 子类型（检查 FQN + 类型实参）。
        let arg_fqn = nominal_fqn_of(ak).or_else(|| scalar_fqn(ak, self.env.interner));
        let bound_fqn = nominal_fqn_of(bk).or_else(|| scalar_fqn(bk, self.env.interner));
        if let (Some(a), Some(b)) = (arg_fqn, bound_fqn) {
            // 直接 FQN 匹配：检查类型实参。
            if a == b {
                let arg_args = nominal_args_of(ak).unwrap_or(&[]);
                let bound_args = nominal_args_of(bk).unwrap_or(&[]);
                if arg_args.len() != bound_args.len() {
                    return false;
                }
                return arg_args
                    .iter()
                    .zip(bound_args)
                    .all(|(aa, ba)| self.arg_satisfies_bound(*aa, *ba));
            }
            // 遍历超类型实例化（FQN + 类型实参）找匹配。
            let bound_args = nominal_args_of(bk).unwrap_or(&[]);
            if let Some(sup_insts) = self.env.supertype_instances(a) {
                for (sup_fqn, sup_args) in sup_insts {
                    if *sup_fqn == b {
                        if sup_args.len() != bound_args.len() {
                            return false;
                        }
                        return sup_args
                            .iter()
                            .zip(bound_args)
                            .all(|(sa, ba)| self.arg_satisfies_bound(*sa, *ba));
                    }
                }
            }
            // 回退：Index supertypes_of（无类型实参信息，仅在 bound 无类型实参时使用）。
            if bound_args.is_empty() {
                return self.env.fqn_is_subtype(a, b);
            }
            return false;
        }
        // 无法判定 → lenient（不报）。
        true
    }

    /// 取类型实参里的**第一个类型**（用于 `Option<T>`）；无类型实参返回 `None`。
    fn lower_type_args_one(&mut self, args: &[TypeArg], span: Span) -> Option<TypeId> {
        for a in args {
            if let TypeArgKind::Type(t) = &a.kind {
                return Some(self.lower(t));
            }
        }
        self.diags.push(diagnostics::arity_mismatch(1, 0, span));
        None
    }

    /// 解析类型路径为 nominal FQN（当前包顶层 / import），需命中类型命名空间。
    fn resolve_type_fqn(&self, path: &TypePath) -> Option<Symbol> {
        let interner = self.env.interner;
        if path.segments.len() == 1 {
            let name = path.segments[0].symbol;
            // 当前包顶层 `<pkg>.<name>`。
            if let Some(f) = self.current_package_type_fqn(name) {
                return Some(f);
            }
            // import。
            if let Some(f) = self
                .imports
                .resolve_name(name, self.env.index, interner)
                .filter(|&fqn| self.env.index.lookup_type(fqn).is_some())
            {
                return Some(f);
            }
            return None;
        }
        // 多段：完整路径作为 FQN。
        let fqn_text = path_text(path, interner);
        let fqn = interner.get(&fqn_text)?;
        if self.env.index.lookup_type(fqn).is_some() {
            Some(fqn)
        } else {
            None
        }
    }

    fn current_package_type_fqn(&self, name: Symbol) -> Option<Symbol> {
        let name_text = self.env.interner.resolve(name);
        let fqn_text = if self.package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{}.{}", self.package_prefix, name_text)
        };
        let fqn = self.env.interner.get(&fqn_text)?;
        if self.env.index.lookup_type(fqn).is_some() {
            Some(fqn)
        } else {
            None
        }
    }
}

fn path_text(path: &TypePath, interner: &Interner) -> String {
    path.segments
        .iter()
        .map(|s| interner.resolve(s.symbol))
        .collect::<Vec<_>>()
        .join(".")
}

fn is_option_name(name: &str) -> bool {
    name == "Option" || name == "scoop.core.Option"
}

/// 若 `kind` 是 nominal（ref 或 value），返回其 FQN。
fn nominal_fqn_of(kind: &TypeKind) -> Option<Symbol> {
    match kind {
        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
        | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(n.fqn),
        _ => None,
    }
}

fn nominal_args_of(kind: &TypeKind) -> Option<&[TypeId]> {
    match kind {
        TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
        | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(&n.args),
        _ => None,
    }
}

/// 标量 → scoop.core 短名 FQN。
fn scalar_fqn(kind: &TypeKind, interner: &Interner) -> Option<Symbol> {
    use crate::ty::{RefTypeKind, ValueTypeKind};
    let name: &'static str = match kind {
        TypeKind::Value(ValueTypeKind::Int) => "scoop.core.Int",
        TypeKind::Value(ValueTypeKind::UInt) => "scoop.core.UInt",
        TypeKind::Value(ValueTypeKind::Bool) => "scoop.core.Bool",
        TypeKind::Value(ValueTypeKind::Char) => "scoop.core.Char",
        TypeKind::Value(ValueTypeKind::Float64) => "scoop.core.Float64",
        TypeKind::Value(ValueTypeKind::Float32) => "scoop.core.Float32",
        TypeKind::Value(ValueTypeKind::IntN(8)) => "scoop.core.Int8",
        TypeKind::Value(ValueTypeKind::IntN(16)) => "scoop.core.Int16",
        TypeKind::Value(ValueTypeKind::IntN(32)) => "scoop.core.Int32",
        TypeKind::Value(ValueTypeKind::IntN(64)) => "scoop.core.Int64",
        TypeKind::Value(ValueTypeKind::UIntN(8)) => "scoop.core.UInt8",
        TypeKind::Value(ValueTypeKind::UIntN(16)) => "scoop.core.UInt16",
        TypeKind::Value(ValueTypeKind::UIntN(32)) => "scoop.core.UInt32",
        TypeKind::Value(ValueTypeKind::UIntN(64)) => "scoop.core.UInt64",
        TypeKind::Ref(RefTypeKind::String) => "scoop.core.String",
        _ => return None,
    };
    interner.get(name)
}

/// where 约束的人类可读描述。
fn bound_desc(bound: &crate::syntax::ast::GenericBound, interner: &Interner) -> String {
    use crate::syntax::ast::GenericBound;
    match bound {
        GenericBound::Ref(_) => "ref".to_string(),
        GenericBound::Value(_) => "value".to_string(),
        GenericBound::Type(t) => type_ref_text(t, interner),
    }
}

fn type_ref_text(t: &TypeRef, interner: &Interner) -> String {
    match &t.kind {
        TypeRefKind::Path { path, .. } => path
            .segments
            .iter()
            .map(|s| interner.resolve(s.symbol))
            .collect::<Vec<_>>()
            .join("."),
        _ => format!("{:?}", t.kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::collect::collect_file;
    use crate::resolve::imports::ImportTable;
    use crate::resolve::index::Index;
    use crate::resolve::symbol::ConeKind;
    use crate::ty::{TypeKind, TypeStore};
    use scoop2_base::{FileId, SourceFile};
    use scoop2_syntax::parser::parse_file;

    /// 解析 + 收集 `src`（含 `fun __t(): <TY> {}`），降低其返回类型，返回
    /// (TypeId, store, interner)。
    fn lower_of(src: &str) -> (TypeId, TypeStore, Interner) {
        let result = parse_file(&SourceFile::new_virtual("<mem>", src));
        let mut interner = result.interner;
        let mut index = Index::new();
        let mut diags = DiagnosticSink::new();
        let cone = index.intern_cone("test", ConeKind::Bin);
        let prefix = crate::resolve::collect::package_prefix_of(&result.file, &interner);
        collect_file(
            &result.file,
            FileId(0),
            cone,
            &mut index,
            &mut interner,
            &mut diags,
        );
        let imports =
            ImportTable::collect(&result.file, FileId(0), &index, &mut interner, &mut diags);
        let fun = result
            .file
            .items
            .iter()
            .find_map(|i| match &i.kind {
                crate::syntax::ast::ItemKind::Fun(d)
                    if interner.resolve(d.name.symbol) == "__t" =>
                {
                    Some(d)
                }
                _ => None,
            })
            .expect("test must declare fun __t(): <TY>");
        let ret_ty = fun.return_ty.clone().expect("__t needs a return type");
        let mut env = TypeEnv::new(&index, &interner);
        let id = {
            let mut lower =
                TypeLowering::new(&mut env, &imports, HashMap::new(), prefix, &mut diags);
            lower.lower(&ret_ty)
        };
        let store = std::mem::replace(&mut env.store, TypeStore::new());
        (id, store, interner)
    }

    #[test]
    fn lowers_builtin_int() {
        let (id, store, _) = lower_of("fun __t(): Int {}");
        assert!(matches!(
            store.kind(id),
            TypeKind::Value(crate::ty::ValueTypeKind::Int)
        ));
    }

    #[test]
    fn lowers_user_struct_as_value_nominal() {
        let (id, store, it) = lower_of("struct Point {}\nfun __t(): Point {}");
        match store.kind(id) {
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
                assert_eq!(it.resolve(n.fqn), "Point");
            }
            other => panic!("expected value nominal, got {other:?}"),
        }
    }

    #[test]
    fn lowers_user_class_as_ref_nominal() {
        let (id, store, it) = lower_of("class C {}\nfun __t(): C {}");
        match store.kind(id) {
            TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => {
                assert_eq!(it.resolve(n.fqn), "C");
            }
            other => panic!("expected ref nominal, got {other:?}"),
        }
    }

    #[test]
    fn lowers_nullable_and_option_consistently() {
        let (q, store, _) = lower_of("fun __t(): Int? {}");
        let (o, store2, _) = lower_of("fun __t(): Option<Int> {}");
        let inner_q = match store.kind(q) {
            TypeKind::Value(crate::ty::ValueTypeKind::Option(i)) => *i,
            _ => panic!("Int? should be Option(Int)"),
        };
        let inner_o = match store2.kind(o) {
            TypeKind::Value(crate::ty::ValueTypeKind::Option(i)) => *i,
            _ => panic!("Option<Int> should be Option(Int)"),
        };
        assert_eq!(
            inner_q, inner_o,
            "Int? and Option<Int> inner types must match"
        );
    }

    #[test]
    fn lowers_tuple_and_function() {
        let (tup, store, _) = lower_of("fun __t(): (Int, Bool) {}");
        assert!(matches!(
            store.kind(tup),
            TypeKind::Value(crate::ty::ValueTypeKind::Tuple(_))
        ));
        let (f, store, _) = lower_of("fun __t(): (Int) -> Bool {}");
        assert!(matches!(
            store.kind(f),
            TypeKind::Ref(crate::ty::RefTypeKind::Function(_))
        ));
    }
}
