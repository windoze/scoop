//! 类型引用的名字解析（resolve 阶段：`TypeRef` 中的类型名是否可解析）。
//!
//! 在 header 收集 + import 之后，遍历顶层声明的**签名类型位置**（函数参数 /
//! 返回 / 接收者、typealias 目标、顶层 `val` 类型、`where` 子句），把每个
//! `TypeRef` 的路径类型名解析为：
//!
//! - 当前声明的**类型参数**；或
//! - 当前包的顶层**类型符号**（`<pkg>.<name>`）；或
//! - **import** 命中的类型符号。
//!
//! 都不命中 → `scoop::resolve::unresolved_type`（类型名）。
//! `where` 子句左侧不是当前声明的类型参数 → `scoop::resolve::unresolved_type_param`。
//!
//! **不在本阶段**：effect 行里的 effect 类型名（deferred）、类型体成员签名
//! （成员解析增量）、`ref`/`value` bound（非类型）。type-ref 的完整 lowering
//! （`TypeRef → TypeId`）是 typecheck 阶段。

use std::collections::HashSet;

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{Interner, Symbol};

use crate::syntax::ast::{
    self, File, FunDecl, GenericBound, ItemKind, TypeArgKind, TypePath, TypeRef, TypeRefKind,
};

use super::errors;
use super::imports::ImportTable;
use super::index::Index;

/// 解析一个文件顶层声明的签名类型引用。
pub fn resolve_file_type_refs(
    file: &File,
    index: &Index,
    imports: &ImportTable,
    interner: &Interner,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
) {
    let mut r = TypeResolver {
        index,
        imports,
        interner,
        diags,
        package_prefix: package_prefix.to_string(),
    };
    for item in &file.items {
        match &item.kind {
            ItemKind::Fun(d) => {
                r.resolve_annotations(&d.annotations);
                r.resolve_fun(d);
            }
            ItemKind::TypeAlias(d) => {
                r.resolve_annotations(&d.annotations);
                let tps = type_param_names(d.type_params.as_ref());
                r.resolve_type_param_bounds(d.type_params.as_ref(), &tps);
                r.resolve_type_ref(&d.ty, &tps);
            }
            ItemKind::Val(d) => {
                r.resolve_annotations(&d.annotations);
                if let Some(ty) = &d.ty {
                    let tps = HashSet::new();
                    r.resolve_type_ref(ty, &tps);
                }
            }
            ItemKind::ExtensionProperty(d) => {
                r.resolve_annotations(&d.annotations);
            }
            ItemKind::Object(d) => {
                r.resolve_annotations(&d.annotations);
                if let Some(b) = &d.body {
                    r.resolve_member_annotations(&b.members);
                }
            }
            ItemKind::Type(d) => {
                r.resolve_annotations(&d.annotations);
                if let Some(b) = &d.body {
                    r.resolve_member_annotations(&b.members);
                }
            }
        }
    }
}

struct TypeResolver<'a> {
    index: &'a Index,
    imports: &'a ImportTable,
    interner: &'a Interner,
    diags: &'a mut DiagnosticSink,
    package_prefix: String,
}

impl<'a> TypeResolver<'a> {
    fn resolve_fun(&mut self, d: &FunDecl) {
        let tps = type_param_names(d.type_params.as_ref());
        // 类型参数自身的 bound（`<T: Bound>`）。
        self.resolve_type_param_bounds(d.type_params.as_ref(), &tps);
        if let Some(recv) = &d.receiver {
            self.resolve_type_ref(recv, &tps);
        }
        for p in &d.params {
            if let Some(ty) = &p.ty {
                self.resolve_type_ref(ty, &tps);
            }
        }
        if let Some(ret) = &d.return_ty {
            self.resolve_type_ref(ret, &tps);
        }
        if let Some(wc) = &d.where_clause {
            for c in &wc.constraints {
                // 左侧必须是当前声明的类型参数。
                if !tps.contains(&c.name.symbol) {
                    let name = self.interner.resolve(c.name.symbol).to_string();
                    self.diags
                        .push(errors::unresolved_type_param(&name, c.name.span));
                }
                // 右侧 bound：ref/value 非类型；Type 才解析。
                if let GenericBound::Type(ty) = &c.bound {
                    self.resolve_type_ref(ty, &tps);
                }
            }
        }
    }

    fn resolve_type_param_bounds(
        &mut self,
        tpl: Option<&ast::TypeParamList>,
        tps: &HashSet<Symbol>,
    ) {
        let Some(tpl) = tpl else { return };
        for p in &tpl.params {
            if let Some(GenericBound::Type(ty)) = &p.bound {
                self.resolve_type_ref(ty, tps);
            }
        }
    }

    fn resolve_type_ref(&mut self, ty: &TypeRef, tps: &HashSet<Symbol>) {
        match &ty.kind {
            TypeRefKind::Path { path, args } => {
                if !self.resolve_path_type(path, tps) {
                    let name = path_text(path, self.interner);
                    self.diags.push(errors::unresolved_type(&name, path.span));
                }
                for arg in args {
                    if let TypeArgKind::Type(t) = &arg.kind {
                        self.resolve_type_ref(t, tps);
                    }
                    // Star / Effect 实参：本阶段不解析。
                }
            }
            TypeRefKind::Unit => {}
            TypeRefKind::Tuple(elems) => {
                for e in elems {
                    self.resolve_type_ref(e, tps);
                }
            }
            TypeRefKind::Function {
                params,
                ret,
                effect: _,
            } => {
                for p in params {
                    self.resolve_type_ref(p, tps);
                }
                self.resolve_type_ref(ret, tps);
            }
            TypeRefKind::ReceiverFunction {
                receiver,
                params,
                ret,
                effect: _,
            } => {
                self.resolve_type_ref(receiver, tps);
                for p in params {
                    self.resolve_type_ref(p, tps);
                }
                self.resolve_type_ref(ret, tps);
            }
            TypeRefKind::Nullable(inner) => self.resolve_type_ref(inner, tps),
        }
    }

    /// 解析路径类型；返回是否可解析。
    fn resolve_path_type(&self, path: &TypePath, tps: &HashSet<Symbol>) -> bool {
        let segs = &path.segments;
        if segs.len() == 1 {
            let name = segs[0].symbol;
            if tps.contains(&name) {
                return true;
            }
            if self.current_package_type_exists(name) {
                return true;
            }
            if self.imported_type_exists(name) {
                return true;
            }
            false
        } else {
            // 多段：按完整 FQN 查类型符号。
            let fqn_text = path_text(path, self.interner);
            if let Some(fqn) = self.interner.get(&fqn_text) {
                return self
                    .index
                    .lookup(fqn)
                    .and_then(|ns| ns.ty.as_ref())
                    .is_some();
            }
            false
        }
    }

    fn current_package_type_exists(&self, name: Symbol) -> bool {
        let name_text = self.interner.resolve(name);
        let fqn_text = if self.package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{}.{}", self.package_prefix, name_text)
        };
        let Some(fqn) = self.interner.get(&fqn_text) else {
            return false;
        };
        self.index
            .lookup(fqn)
            .and_then(|ns| ns.ty.as_ref())
            .is_some()
    }

    fn imported_type_exists(&self, name: Symbol) -> bool {
        let Some(fqn) = self.imports.resolve_name(name, self.index, self.interner) else {
            return false;
        };
        self.index
            .lookup(fqn)
            .and_then(|ns| ns.ty.as_ref())
            .is_some()
    }

    /// 解析注解使用路径（`@Path.To.Ann`）为类型；不命中 → unresolved_type。
    fn resolve_annotations(&mut self, anns: &[ast::AnnotationUse]) {
        for ann in anns {
            if !self.annotation_path_resolves(&ann.path) {
                let text = path_text(&ann.path, self.interner);
                // 指向注解路径的首段（跳过 `@`），与 fixture 的 EXPECT-ERROR-AT 对齐。
                let span = ann
                    .path
                    .segments
                    .first()
                    .map(|s| s.span)
                    .unwrap_or(ann.span);
                self.diags.push(errors::unresolved_type(&text, span));
            }
        }
    }

    /// 注解路径是否解析为类型符号。单段 → 当前包/import 类型；多段 → `<pkg>.<path>` 类型查找。
    fn annotation_path_resolves(&self, path: &TypePath) -> bool {
        if path.segments.len() == 1 {
            let name = path.segments[0].symbol;
            // 内建注解名（spec P5 §12）始终可解析——它们不需要在 sysroot 中声明为 class。
            let name_text = self.interner.resolve(name);
            if is_builtin_annotation(name_text) {
                return true;
            }
            return self.current_package_type_exists(name) || self.imported_type_exists(name);
        }
        let path_text = path_text(path, self.interner);
        let fqn_text = if self.package_prefix.is_empty() {
            path_text
        } else {
            format!("{}.{}", self.package_prefix, path_text)
        };
        let Some(fqn) = self.interner.get(&fqn_text) else {
            return false;
        };
        self.index
            .lookup(fqn)
            .and_then(|ns| ns.ty.as_ref())
            .is_some()
    }

    /// 解析类型体成员的注解（属性 / 成员函数 / variant / 嵌套类型 / object）。
    fn resolve_member_annotations(&mut self, members: &[ast::TypeMember]) {
        for m in members {
            match &m.kind {
                ast::TypeMemberKind::Property(d) => self.resolve_annotations(&d.annotations),
                ast::TypeMemberKind::Fun(d) => self.resolve_annotations(&d.annotations),
                ast::TypeMemberKind::EnumVariant(d) => self.resolve_annotations(&d.annotations),
                ast::TypeMemberKind::Object(d) => {
                    self.resolve_annotations(&d.annotations);
                    if let Some(b) = &d.body {
                        self.resolve_member_annotations(&b.members);
                    }
                }
                ast::TypeMemberKind::Type(d) => {
                    self.resolve_annotations(&d.annotations);
                    if let Some(b) = &d.body {
                        self.resolve_member_annotations(&b.members);
                    }
                }
                ast::TypeMemberKind::InitBlock(_) | ast::TypeMemberKind::SecondaryCtor(_) => {}
            }
        }
    }
}

/// 取类型参数列表的名字集合。
fn type_param_names(tpl: Option<&ast::TypeParamList>) -> HashSet<Symbol> {
    let mut s = HashSet::new();
    if let Some(tpl) = tpl {
        for p in &tpl.params {
            s.insert(p.name.symbol);
        }
    }
    s
}

/// 内建注解名（spec P5 §12）：始终可解析，无需在 sysroot 中声明为 class。
fn is_builtin_annotation(name: &str) -> bool {
    matches!(
        name,
        "Extern"
            | "Intrinsic"
            | "AllowIntrinsic"
            | "Unsafe"
            | "Safe"
            | "NoGC"
            | "Deprecated"
            | "ReplaceWith"
            | "CLayout"
            | "TailRec"
            | "Global"
            | "ThreadLocal"
            | "InteriorMutable"
            | "Target"
            | "Retention"
            | "Suppress"
            | "Experimental"
            | "CallingConvention"
    )
}

fn path_text(path: &TypePath, interner: &Interner) -> String {
    path.segments
        .iter()
        .map(|seg| interner.resolve(seg.symbol))
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::collect::collect_file;
    use crate::resolve::imports::ImportTable;
    use crate::resolve::symbol::ConeKind;
    use scoop2_base::{FileId, SourceFile};
    use scoop2_syntax::parser::parse_file;

    fn resolve_types(src: &str) -> Vec<String> {
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
        resolve_file_type_refs(
            &result.file,
            &index,
            &imports,
            &interner,
            &mut diags,
            &prefix,
        );
        diags.iter().map(|d| d.code.to_string()).collect()
    }

    #[test]
    fn unresolved_type_in_param() {
        let codes = resolve_types("fun f(x: Missing) {}\n");
        assert!(
            codes.iter().any(|c| c == "scoop::resolve::unresolved_type"),
            "{codes:?}"
        );
    }

    #[test]
    fn undeclared_type_param_in_sig() {
        // `T` 未声明为类型参数 → unresolved_type（per fixture T0309）。
        let codes = resolve_types("fun f(x: T) {}\n");
        assert!(
            codes.iter().any(|c| c == "scoop::resolve::unresolved_type"),
            "{codes:?}"
        );
    }

    #[test]
    fn declared_type_param_resolves() {
        let codes = resolve_types("fun <T> f(x: T): T {}\n");
        assert!(codes.is_empty(), "T is declared: {codes:?}");
    }

    #[test]
    fn where_clause_bound_type_unresolved() {
        let codes = resolve_types("fun <T> f(x: T): T where T: Show {}\n");
        assert!(
            codes.iter().any(|c| c == "scoop::resolve::unresolved_type"),
            "{codes:?}"
        );
    }

    #[test]
    fn where_clause_left_not_type_param() {
        let codes = resolve_types("interface Show {}\nfun <T> f(x: T): T where U: Show {}\n");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::resolve::unresolved_type_param"),
            "{codes:?}"
        );
    }

    #[test]
    fn nullable_tuple_function_types_recurse() {
        // (A, B) -> C? 全部未声明 → unresolved_type（至少一条）。
        let codes = resolve_types("fun f(x: (A, B) -> C?) {}\n");
        assert!(
            codes.iter().any(|c| c == "scoop::resolve::unresolved_type"),
            "{codes:?}"
        );
    }
}
