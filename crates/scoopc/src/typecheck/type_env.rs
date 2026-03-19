//! 类型环境（type environment）。
//!
//! T0402：基于 sysroot AST 建立 type env（Any/Option/Raise），
//! 为后续 typecheck 提供“类型符号的声明头（kind + arity）”查询能力。

use std::collections::HashMap;
use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::sysroot::Sysroot;

/// 类型符号的种类（type namespace）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSymbolKind {
    /// 名义类型声明：class/interface/struct/enum/effect。
    Nominal(ast::TypeKind),
    /// 类型别名：`typealias Name = ...`。
    TypeAlias,
}

/// typecheck 使用的“类型符号声明头”信息。
///
/// 当前阶段（T0402）只保留最小集合：
/// - `kind`：区分 nominal type 与 typealias
/// - `type_param_count`：泛型参数数量（arity）
/// - `span/decl_file`：用于 env 构建阶段的重复定义诊断与调试
#[derive(Debug, Clone)]
pub struct TypeSymbol {
    pub kind: TypeSymbolKind,
    pub type_param_count: usize,
    pub span: Span,
    pub decl_file: PathBuf,
}

#[derive(Debug, Error, Diagnostic)]
pub enum TypeEnvError {
    #[error("重复的类型符号：{fqn}")]
    #[diagnostic(code(scoop::typecheck::duplicate_type_symbol))]
    DuplicateTypeSymbol {
        fqn: String,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
    },
}

/// 类型环境：通过 FQN 查询类型符号信息。
#[derive(Debug, Default, Clone)]
pub struct TypeEnv {
    by_fqn: HashMap<String, TypeSymbol>,
}

impl TypeEnv {
    /// 从 sysroot AST 构建类型环境。
    ///
    /// 说明：
    /// - sysroot 是编译器“内建 API 的声明源”，因此 typecheck 的起点应由 sysroot 决定；
    /// - 当前阶段仅收集声明头信息，不解析函数体/方法体。
    pub fn from_sysroot(sysroot: &Sysroot) -> Result<Self, TypeEnvError> {
        let mut env = Self::default();
        for f in &sysroot.files {
            env.collect_from_file(&f.source, &f.ast)?;
        }
        Ok(env)
    }

    /// 将一个普通源文件的类型声明头信息合并进当前环境。
    ///
    /// 说明：
    /// - 该方法用于把“当前编译单元的用户代码”纳入 type env，从而支持后续阶段：
    ///   - TypeRef lowering 的泛型 arity 检查（T0403）
    ///   - 顶层签名检查（T0404）
    /// - 目前依旧只收集声明头（kind + arity），不进入函数体/方法体。
    pub fn extend_from_file(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
    ) -> Result<(), TypeEnvError> {
        self.collect_from_file(source, file)
    }

    /// 按 FQN 查询类型符号。
    pub fn type_symbol(&self, fqn: &str) -> Option<&TypeSymbol> {
        self.by_fqn.get(fqn)
    }

    /// 返回给定 FQN 的 type params 数量（arity）。
    pub fn type_param_count(&self, fqn: &str) -> Option<usize> {
        self.type_symbol(fqn).map(|s| s.type_param_count)
    }

    fn collect_from_file(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
    ) -> Result<(), TypeEnvError> {
        let pkg_prefix = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            match item {
                ast::Item::TypeAlias(ta) => {
                    let name = source.slice(ta.name.span).to_string();
                    let fqn = join_prefix(&pkg_prefix, &name);
                    self.insert_symbol(
                        fqn,
                        TypeSymbol {
                            kind: TypeSymbolKind::TypeAlias,
                            type_param_count: 0,
                            span: ta.name.span,
                            decl_file: source.path().to_path_buf(),
                        },
                    )?;
                }
                ast::Item::Type(ty) => {
                    self.collect_type_decl(source, &pkg_prefix, ty)?;
                }
                ast::Item::Fun(_) | ast::Item::Val(_) => {}
            }
        }

        Ok(())
    }

    fn collect_type_decl(
        &mut self,
        source: &SourceFile,
        prefix: &str,
        decl: &ast::TypeDecl,
    ) -> Result<(), TypeEnvError> {
        let name = source.slice(decl.name.span).to_string();
        let fqn = join_prefix(prefix, &name);

        self.insert_symbol(
            fqn.clone(),
            TypeSymbol {
                kind: TypeSymbolKind::Nominal(decl.kind),
                type_param_count: decl.type_params.len(),
                span: decl.name.span,
                decl_file: source.path().to_path_buf(),
            },
        )?;

        let Some(body) = &decl.body else {
            return Ok(());
        };

        for member in &body.members {
            let ast::TypeMember::Type(nested) = member else {
                continue;
            };
            self.collect_type_decl(source, &fqn, nested)?;
        }

        Ok(())
    }

    fn insert_symbol(&mut self, fqn: String, symbol: TypeSymbol) -> Result<(), TypeEnvError> {
        if let Some(prev) = self.by_fqn.get(&fqn) {
            return Err(TypeEnvError::DuplicateTypeSymbol {
                fqn,
                first: prev.span.into(),
                second: symbol.span.into(),
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

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    #[test]
    fn sysroot_type_env_contains_option_arity() {
        let sess = Session::new().unwrap();
        let env = TypeEnv::from_sysroot(sess.sysroot()).unwrap();

        assert_eq!(env.type_param_count("scoop.core.Option"), Some(1));
    }
}
