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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Fun,
    Type,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;

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
}

