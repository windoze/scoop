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
use crate::ty::{EffectRow, FunctionType, NominalType, TypeId, TypeParamType};

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
                effect: _,
            } => {
                let params: Vec<TypeId> = params.iter().map(|p| self.lower(p)).collect();
                let ret = self.lower(ret);
                self.env.store.function(FunctionType {
                    receiver: None,
                    params,
                    return_ty: ret,
                    effects: EffectRow::pure(),
                    closed: false,
                })
            }
            TypeRefKind::ReceiverFunction {
                receiver,
                params,
                ret,
                effect: _,
            } => {
                let receiver = self.lower(receiver);
                let params: Vec<TypeId> = params.iter().map(|p| self.lower(p)).collect();
                let ret = self.lower(ret);
                self.env.store.function(FunctionType {
                    receiver: Some(receiver),
                    params,
                    return_ty: ret,
                    effects: EffectRow::pure(),
                    closed: false,
                })
            }
            TypeRefKind::Nullable(inner) => {
                let inner = self.lower(inner);
                self.env.store.option(inner)
            }
        }
    }

    fn lower_path(&mut self, path: &TypePath, args: &[TypeArg], span: Span) -> TypeId {
        let name_text = path_text(path, self.env.interner);

        // 1. 内建标量 / String / Unit / Nothing（无类型实参）。
        if args.is_empty()
            && let Some(b) = self.env.builtin(&name_text)
        {
            return b;
        }

        // 2. Option<T>（与 T? 一致）。
        if is_option_name(&name_text) {
            let inner = self
                .lower_type_args_one(args, span)
                .unwrap_or_else(|| self.env.store.nothing());
            return self.env.store.option(inner);
        }

        // 3. 类型参数。
        if path.segments.len() == 1
            && let Some(tp) = self.type_params.get(&path.segments[0].symbol)
        {
            return self.env.store.param(*tp);
        }

        // 4. nominal：解析 FQN，按 category 决定 ref/value。
        let Some(fqn) = self.resolve_type_fqn(path) else {
            self.diags
                .push(diagnostics::unresolved_type_ref(&name_text, span));
            return self.env.store.nothing();
        };
        let lowered_args: Vec<TypeId> = args
            .iter()
            .filter_map(|a| match &a.kind {
                TypeArgKind::Type(t) => Some(self.lower(t)),
                _ => None, // Star / Effect 实参：M0 不降级。
            })
            .collect();
        let nominal = NominalType {
            fqn,
            args: lowered_args,
            eff: None,
        };
        if self.env.is_reference_nominal(fqn) {
            self.env.store.ref_nominal(nominal)
        } else {
            self.env.store.value_nominal(nominal)
        }
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
