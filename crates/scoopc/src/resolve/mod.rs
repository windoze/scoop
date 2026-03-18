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

/// 一个编译单元（多个文件）的顶层符号索引。
#[derive(Debug, Default)]
pub struct Index {
    /// FQN（例如 `scoop.core.Option`）→ Symbol
    pub by_fqn: HashMap<String, Symbol>,
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
                ast::Item::Fun(fun) => {
                    self.insert_symbol(source, &pkg, SymbolKind::Fun, fun.name.span)?;
                }
                ast::Item::Type(ty) => {
                    self.insert_symbol(source, &pkg, SymbolKind::Type, ty.name.span)?;
                }
                ast::Item::Val(v) => {
                    self.insert_symbol(source, &pkg, SymbolKind::Value, v.name.span)?;
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

        if let Some(prev) = self.by_fqn.get(&fqn) {
            return Err(ResolveError::DuplicateDefinition {
                name: fqn,
                first: prev.span.into(),
                second: name_span.into(),
            });
        }

        self.by_fqn.insert(fqn, symbol);
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
            ast::Item::Fun(fun) => {
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

fn check_imports_exist(source: &SourceFile, file: &ast::File, index: &Index) -> Result<(), ResolveError> {
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
        if let Some(sym) = index.by_fqn.get(&fqn)
            && sym.kind == SymbolKind::Type {
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
