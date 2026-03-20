//! 类型环境（type environment）。
//!
//! T0402：基于 sysroot AST 建立 type env（Any/Option/Raise），
//! 为后续 typecheck 提供“类型符号的声明头（kind + arity）”查询能力。
//!
//! T0425：扩展 type env 以收集 enum variants（tag + payload types），为 rich enum 的类型检查打底。

use std::collections::HashMap;
use std::path::Path;
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

/// enum variant 的字段信息（payload field）。
#[derive(Debug, Clone)]
pub struct EnumVariantField {
    pub name: String,
    pub span: Span,
    pub ty: ast::TypeRef,
}

/// enum variant 信息（tag + payload types）。
#[derive(Debug, Clone)]
pub struct EnumVariantInfo {
    pub tag: u32,
    pub name: String,
    pub span: Span,
    pub fields: Vec<EnumVariantField>,
}

/// enum 声明的语义信息（当前阶段仅 variants）。
#[derive(Debug, Clone)]
pub struct EnumDecl {
    /// enum 声明所在的源文件（用于在 typecheck 阶段按 span 取回原始标识符文本）。
    pub decl_file: PathBuf,
    /// enum 的类型参数名（按声明顺序）。
    ///
    /// 说明：
    /// - 目前主要用于 enum variant 构造表达式的最小泛型实例化（T0426）；
    /// - 未来若引入更完整的泛型推断/约束系统，这里仍可作为声明信息来源。
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariantInfo>,
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

    #[error("enum variant 重复定义：{enum_fqn}.{variant}")]
    #[diagnostic(code(scoop::typecheck::duplicate_enum_variant))]
    DuplicateEnumVariant {
        enum_fqn: String,
        variant: String,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
    },

    #[error("enum variant 字段重复定义：{enum_fqn}.{variant}.{field}")]
    #[diagnostic(code(scoop::typecheck::duplicate_enum_variant_field))]
    DuplicateEnumVariantField {
        enum_fqn: String,
        variant: String,
        field: String,
        #[label("重复定义在这里")]
        second: miette::SourceSpan,
        #[label("第一次定义在这里")]
        first: miette::SourceSpan,
    },
}

/// 类型环境：通过 FQN 查询类型符号信息。
#[derive(Debug, Default, Clone)]
pub struct TypeEnv {
    by_fqn: HashMap<String, TypeSymbol>,
    enums: HashMap<String, EnumDecl>,
    sources: HashMap<PathBuf, SourceFile>,
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

    /// 按 FQN 查询 enum 的 variant 信息（若该类型不是 enum 或未收集到则为 None）。
    pub fn enum_decl(&self, fqn: &str) -> Option<&EnumDecl> {
        self.enums.get(fqn)
    }

    /// 通过文件路径获取对应的 `SourceFile`（若该文件在构建 env 时被收集过）。
    pub fn source(&self, path: &Path) -> Option<&SourceFile> {
        self.sources.get(path)
    }

    /// 返回所有名字为 `variant_name` 的 enum variant 候选（跨所有已收集的 enum）。
    ///
    /// 说明（T0426）：
    /// - 早期阶段我们允许通过 `Some(1)` 这种“不带 enum 前缀”的写法构造 variant；
    /// - 为避免引入完整的作用域/导入规则，本方法仅提供候选集合，
    ///   由 typecheck 决定“同名唯一”约束与报错策略。
    pub fn find_enum_variants_named(
        &self,
        variant_name: &str,
    ) -> Vec<(String, EnumVariantInfo)> {
        let mut out = Vec::new();
        for (enum_fqn, decl) in &self.enums {
            for v in &decl.variants {
                if v.name == variant_name {
                    out.push((enum_fqn.clone(), v.clone()));
                }
            }
        }
        out
    }

    fn collect_from_file(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
    ) -> Result<(), TypeEnvError> {
        // 记录源文件内容，供后续 typecheck 在跨文件引用（例如 sysroot enum variants）时
        // 通过 span 反查标识符文本。
        self.sources
            .entry(source.path().to_path_buf())
            .or_insert_with(|| source.clone());

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
        let type_params: Vec<String> = decl
            .type_params
            .iter()
            .map(|p| source.slice(p.name.span).to_string())
            .collect();

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
            if matches!(decl.kind, ast::TypeKind::Enum) {
                self.enums.insert(
                    fqn.clone(),
                    EnumDecl {
                        decl_file: source.path().to_path_buf(),
                        type_params,
                        variants: Vec::new(),
                    },
                );
            }
            return Ok(());
        };

        if matches!(decl.kind, ast::TypeKind::Enum) {
            let mut variants: Vec<EnumVariantInfo> = Vec::new();
            let mut seen_variants: HashMap<String, Span> = HashMap::new();

            for member in &body.members {
                let ast::TypeMember::EnumVariant(v) = member else {
                    continue;
                };

                let variant_name = source.slice(v.name.span).to_string();
                if let Some(prev) = seen_variants.get(&variant_name).copied() {
                    return Err(TypeEnvError::DuplicateEnumVariant {
                        enum_fqn: fqn.clone(),
                        variant: variant_name,
                        first: prev.into(),
                        second: v.name.span.into(),
                    });
                }
                seen_variants.insert(variant_name.clone(), v.name.span);

                let mut fields: Vec<EnumVariantField> = Vec::new();
                let mut seen_fields: HashMap<String, Span> = HashMap::new();
                for p in &v.params {
                    let field_name = source.slice(p.name.span).to_string();
                    if let Some(prev) = seen_fields.get(&field_name).copied() {
                        return Err(TypeEnvError::DuplicateEnumVariantField {
                            enum_fqn: fqn.clone(),
                            variant: variant_name.clone(),
                            field: field_name,
                            first: prev.into(),
                            second: p.name.span.into(),
                        });
                    }
                    seen_fields.insert(field_name.clone(), p.name.span);

                    let Some(ty) = &p.ty else {
                        continue;
                    };
                    fields.push(EnumVariantField {
                        name: field_name,
                        span: p.name.span,
                        ty: ty.clone(),
                    });
                }

                variants.push(EnumVariantInfo {
                    tag: u32::try_from(variants.len()).unwrap_or(u32::MAX),
                    name: variant_name,
                    span: v.span,
                    fields,
                });
            }

            self.enums.insert(
                fqn.clone(),
                EnumDecl {
                    decl_file: source.path().to_path_buf(),
                    type_params,
                    variants,
                },
            );
        }

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
