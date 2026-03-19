//! 名字解析（name resolution）。
//!
//! Scoop 的完整名字解析会涉及：
//! - package/import
//! - 多命名空间（type/value）
//! - 作用域（块级、类型体、泛型参数、扩展 receiver 等）
//! - 可见性（public/internal/private）
//!
//! 当前阶段先落地最小子集：**顶层符号索引**。
//! - 把每个文件的 `package` + 顶层声明名组合成 FQN（Fully Qualified Name）
//! - 构建索引并检测重复定义

use std::collections::HashMap;

use miette::Diagnostic;
use thiserror::Error;

use crate::{ast, source::SourceFile, span::Span};

#[derive(Debug, Error, Diagnostic)]
pub enum ResolveError {
    #[error("重复定义：{name}")]
    #[diagnostic(code(scoop::resolve::duplicate_definition))]
    DuplicateDefinition {
        name: String,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
    },

    #[error("未解析的 import：{import}")]
    #[diagnostic(code(scoop::resolve::unresolved_import))]
    UnresolvedImport {
        import: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未解析的类型：{name}")]
    #[diagnostic(code(scoop::resolve::unresolved_type))]
    UnresolvedType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Fun,
    Type,
    Value,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    pub span: Span,
}

/// 同一个 FQN 下按命名空间（type/value/fun）分组的符号集合。
///
/// 说明：
/// - 语言层面允许 **同名 type 与 fun/value 并存**（它们属于不同命名空间）。
/// - 同一命名空间内仍保持“当前阶段不支持重载”的约束：重复定义直接报错。
#[derive(Debug, Default, Clone)]
pub struct NamespacedSymbols {
    pub ty: Option<Symbol>,
    pub fun: Option<Symbol>,
    pub value: Option<Symbol>,
}

impl NamespacedSymbols {
    fn slot_mut(&mut self, kind: SymbolKind) -> &mut Option<Symbol> {
        match kind {
            SymbolKind::Type => &mut self.ty,
            SymbolKind::Fun => &mut self.fun,
            SymbolKind::Value => &mut self.value,
        }
    }

    fn get(&self, kind: SymbolKind) -> Option<&Symbol> {
        match kind {
            SymbolKind::Type => self.ty.as_ref(),
            SymbolKind::Fun => self.fun.as_ref(),
            SymbolKind::Value => self.value.as_ref(),
        }
    }
}

/// 一个编译单元（多个文件）的顶层符号索引。
#[derive(Debug, Default)]
pub struct Index {
    /// FQN（例如 `scoop.core.Option`）→ 按命名空间分组的符号集合。
    pub by_fqn: HashMap<String, NamespacedSymbols>,
}

impl Index {
    pub fn build(files: &[(&SourceFile, &ast::File)]) -> Result<Self, ResolveError> {
        let mut index = Index::default();
        for (source, file) in files {
            index.add_file(source, file)?;
        }
        Ok(index)
    }

    fn add_file(&mut self, source: &SourceFile, file: &ast::File) -> Result<(), ResolveError> {
        let pkg = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            match item {
                ast::Item::TypeAlias(ta) => {
                    // typealias 是类型命名空间的顶层符号（T0251）。
                    self.insert_symbol(source, &pkg, SymbolKind::Type, ta.name.span)?;
                }
                ast::Item::Fun(fun) => {
                    self.insert_symbol(source, &pkg, SymbolKind::Fun, fun.name.span)?;
                }
                ast::Item::Type(ty) => {
                    self.insert_symbol(source, &pkg, SymbolKind::Type, ty.name.span)?;
                }
                ast::Item::Val(v) => {
                    // 顶层 `val/var` 必须有名字；解构绑定仅在 block 内作为语句出现（T0244）。
                    if let Some(name) = v.name() {
                        self.insert_symbol(source, &pkg, SymbolKind::Value, name.span)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn insert_symbol(
        &mut self,
        source: &SourceFile,
        pkg_prefix: &str,
        kind: SymbolKind,
        name_span: Span,
    ) -> Result<(), ResolveError> {
        let local = source.slice(name_span).to_string();
        let fqn = if pkg_prefix.is_empty() {
            local.clone()
        } else {
            format!("{pkg_prefix}.{local}")
        };

        let symbol = Symbol {
            kind,
            name: local,
            span: name_span,
        };

        let entry = self.by_fqn.entry(fqn.clone()).or_default();
        if let Some(prev) = entry.get(kind) {
            return Err(ResolveError::DuplicateDefinition {
                name: fqn,
                first: prev.span.into(),
                second: name_span.into(),
            });
        }

        *entry.slot_mut(kind) = Some(symbol);
        Ok(())
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

/// 在 `Index` 的基础上，做最小的文件级名字绑定检查：
/// - import 的目标是否存在
/// - 函数签名/顶层 val/var 的类型引用是否可解析（仅 TypeRef::Path）
///
/// 当前阶段的简化：
/// - 只解析类型名（type namespace）；不解析值/函数名
/// - 只检查“存在性”，不做重载/可见性/作用域
pub fn check_file_bindings(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
) -> Result<(), ResolveError> {
    check_imports_exist(source, file, index)?;

    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                resolve_type_ref(source, file, index, &ta.ty)?;
            }
            ast::Item::Fun(fun) => {
                if let Some(receiver) = &fun.receiver {
                    resolve_type_ref(source, file, index, receiver)?;
                }
                for p in &fun.params {
                    if let Some(ty) = &p.ty {
                        resolve_type_ref(source, file, index, ty)?;
                    }
                }
                if let Some(ret) = &fun.return_ty {
                    resolve_type_ref(source, file, index, ret)?;
                }
            }
            ast::Item::Val(v) => {
                if let Some(ty) = &v.ty {
                    resolve_type_ref(source, file, index, ty)?;
                }
            }
            ast::Item::Type(_) => {}
        }
    }

    Ok(())
}

fn check_imports_exist(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
) -> Result<(), ResolveError> {
    for import in &file.imports {
        let path = import
            .path
            .iter()
            .map(|id| source.slice(id.span))
            .collect::<Vec<_>>()
            .join(".");

        if import.has_star {
            let prefix = format!("{path}.");
            let ok = index.by_fqn.keys().any(|k| k.starts_with(&prefix));
            if !ok {
                return Err(ResolveError::UnresolvedImport {
                    import: format!("{path}.*"),
                    span: import.span.into(),
                });
            }
            continue;
        }

        if !index.by_fqn.contains_key(&path) {
            return Err(ResolveError::UnresolvedImport {
                import: path,
                span: import.span.into(),
            });
        }
    }
    Ok(())
}

fn resolve_type_ref(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    ty: &ast::TypeRef,
) -> Result<(), ResolveError> {
    match ty {
        ast::TypeRef::Path(p) => resolve_type_path(source, file, index, p),
        ast::TypeRef::Tuple(t) => {
            for e in &t.elements {
                resolve_type_ref(source, file, index, e)?;
            }
            Ok(())
        }
        // 星投影不引入可解析的符号引用：`List<*>` 中的 `*` 由 typecheck 决定具体含义。
        ast::TypeRef::Star { .. } => Ok(()),
        ast::TypeRef::Function(f) => {
            if let Some(receiver) = &f.receiver {
                resolve_type_ref(source, file, index, receiver)?;
            }
            for p in &f.params {
                resolve_type_ref(source, file, index, p)?;
            }
            resolve_type_ref(source, file, index, &f.return_ty)?;

            if let Some(effects) = &f.effects {
                for term in &effects.terms {
                    resolve_type_path(source, file, index, term)?;
                }
            }

            Ok(())
        }
        ast::TypeRef::Nullable { inner, .. } => resolve_type_ref(source, file, index, inner),
    }
}

fn resolve_type_path(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    path: &ast::TypePath,
) -> Result<(), ResolveError> {
    let segments = path
        .segments
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>();
    let local = segments.join(".");

    let pkg = package_prefix(source, file.package.as_ref());
    let mut candidates = Vec::new();

    // 1) 同包优先：pkg + local
    if !pkg.is_empty() {
        candidates.push(format!("{pkg}.{local}"));
    }

    // 2) 直接使用 local（允许显式写 FQN：`scoop.core.Any`）
    candidates.push(local.clone());

    // 3) 对单段名字，应用 import 规则（显式 import / star import）
    if segments.len() == 1 {
        let name = segments[0];
        for import in &file.imports {
            let import_path = import
                .path
                .iter()
                .map(|id| source.slice(id.span))
                .collect::<Vec<_>>()
                .join(".");

            if import.has_star {
                candidates.push(format!("{import_path}.{name}"));
            } else {
                let last = import
                    .path
                    .last()
                    .map(|id| source.slice(id.span))
                    .unwrap_or("");
                if last == name {
                    candidates.push(import_path);
                }
            }
        }
    }

    // 去重并尝试匹配 type namespace
    candidates.sort();
    candidates.dedup();

    for fqn in candidates {
        if index
            .by_fqn
            .get(&fqn)
            .is_some_and(|syms| syms.get(SymbolKind::Type).is_some())
        {
            // TODO: 在后续阶段把解析结果写回 AST/HIR
            return Ok(());
        }
    }

    Err(ResolveError::UnresolvedType {
        name: local,
        span: path.span.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use crate::session::Session;

    #[test]
    fn duplicate_top_level_is_error() {
        let s1 = SourceFile::new_virtual("<mem1>", "package a\nfun f() {}");
        let s2 = SourceFile::new_virtual("<mem2>", "package a\nfun f() {}");
        let a1 = parse_file(&s1).unwrap();
        let a2 = parse_file(&s2).unwrap();

        let err = Index::build(&[(&s1, &a1), (&s2, &a2)]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("重复定义"));
    }

    #[test]
    fn resolve_types_with_import_star() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nimport scoop.core.*\nfun f(x: Option<Any>): Any {}",
        );
        let ast = parse_file(&src).unwrap();

        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&src, &ast));

        let index = Index::build(&pairs).unwrap();
        check_file_bindings(&src, &ast, &index).unwrap();
    }

    #[test]
    fn unresolved_type_is_error() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual("<mem>", "package a\nfun f(x: Missing) {}");
        let ast = parse_file(&src).unwrap();

        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&src, &ast));

        let index = Index::build(&pairs).unwrap();
        let err = check_file_bindings(&src, &ast, &index).unwrap_err();
        assert!(matches!(err, ResolveError::UnresolvedType { .. }));
    }
}
