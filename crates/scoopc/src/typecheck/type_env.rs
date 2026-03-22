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
use crate::resolve::{ImportTable, Index, Visibility};
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
    /// 是否包含 `eff` effect row 参数（spec §3.4 / §5.8）。
    ///
    /// 说明：
    /// - `eff` 参数不计入 `type_param_count`（arity），因为它不是 type argument；
    /// - use-site 的 `Type<eff Row>` lowering 需要知道声明处的默认值与声明文件上下文。
    pub eff_param: Option<EffParamInfo>,
    /// 类型参数名（按声明顺序）。
    ///
    /// 说明：
    /// - 早期阶段很多逻辑只需要 arity；但 `where` 约束满足性检查（T0458）需要把
    ///   `where T: Bound` 的左侧映射到“第几个 type arg”。
    pub type_param_names: Vec<String>,
    /// 声明处变型（declaration-site variance）：与 `type_param_count` 对齐的按位信息。
    ///
    /// 说明：
    /// - `None` 表示 invariant；
    /// - `Some(In|Out)` 对应 `in`/`out`。
    pub type_param_variances: Vec<Option<ast::TypeParamVariance>>,
    /// `where` 子句的约束信息（T0458）。
    ///
    /// 说明：
    /// - 这里保留 `TypeRef`（而不是提前 lowering 成 `TypeId`），以便在 use-site
    ///   结合具体 type args 做 substitution 后再进行检查。
    pub where_constraints: Vec<WhereConstraintInfo>,
    pub span: Span,
    pub decl_file: PathBuf,
}

/// `eff` effect row 参数在 type env 中的最小表示。
#[derive(Debug, Clone)]
pub struct EffParamInfo {
    pub span: Span,
    pub name: String,
    pub default: Option<ast::EffectRowExpr>,
}

/// `where` 子句的一条约束在 type env 中的最小表示。
#[derive(Debug, Clone)]
pub struct WhereConstraintInfo {
    pub span: Span,
    /// 约束目标在声明 type param 列表中的索引（0-based）。
    pub param_index: usize,
    /// 约束右侧的 bound TypeRef（在声明处文件上下文中解析/lower）。
    pub bound: ast::TypeRef,
}

/// 单个源文件在 typecheck lowering 阶段需要的最小上下文信息。
///
/// 用途：
/// - typealias 展开（T0446）：需要在“声明处文件”的 package/import 规则下解析 RHS 类型引用；
/// - 其它跨文件 type position lowering（例如 sysroot enum variant 字段，T0426）也可复用。
#[derive(Debug, Clone)]
pub struct FileTypeContext {
    pub pkg_prefix: String,
    pub imports: ImportTable,
}

/// `typealias` 的声明信息（用于 typecheck 阶段展开别名）。
#[derive(Debug, Clone)]
pub struct TypeAliasInfo {
    pub decl_file: PathBuf,
    pub name_span: Span,
    pub ty: ast::TypeRef,
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
    supertypes: HashMap<String, Vec<String>>,
    file_ctx: HashMap<PathBuf, FileTypeContext>,
    type_aliases: HashMap<String, TypeAliasInfo>,
}

impl TypeEnv {
    /// 从 sysroot AST 构建类型环境。
    ///
    /// 说明：
    /// - sysroot 是编译器“内建 API 的声明源”，因此 typecheck 的起点应由 sysroot 决定；
    /// - 当前阶段仅收集声明头信息，不解析函数体/方法体。
    pub fn from_sysroot(sysroot: &Sysroot, index: &Index) -> Result<Self, TypeEnvError> {
        let mut env = Self::default();
        for f in &sysroot.files {
            env.collect_from_file(&f.source, &f.ast, index)?;
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
        index: &Index,
    ) -> Result<(), TypeEnvError> {
        self.collect_from_file(source, file, index)
    }

    /// 按 FQN 查询类型符号。
    pub fn type_symbol(&self, fqn: &str) -> Option<&TypeSymbol> {
        self.by_fqn.get(fqn)
    }

    /// 返回给定 FQN 的 type params 数量（arity）。
    pub fn type_param_count(&self, fqn: &str) -> Option<usize> {
        self.type_symbol(fqn).map(|s| s.type_param_count)
    }

    /// 返回给定 FQN 的声明处 type param variances（若不是 nominal/typealias 或未收集则为 None）。
    pub fn type_param_variances(&self, fqn: &str) -> Option<&[Option<ast::TypeParamVariance>]> {
        self.type_symbol(fqn)
            .map(|s| s.type_param_variances.as_slice())
    }

    /// 按 FQN 查询 enum 的 variant 信息（若该类型不是 enum 或未收集到则为 None）。
    pub fn enum_decl(&self, fqn: &str) -> Option<&EnumDecl> {
        self.enums.get(fqn)
    }

    /// 通过文件路径获取对应的 `SourceFile`（若该文件在构建 env 时被收集过）。
    pub fn source(&self, path: &Path) -> Option<&SourceFile> {
        self.sources.get(path)
    }

    /// 返回给定源文件的 type lowering 上下文（package/import）。
    pub fn file_type_context(&self, path: &Path) -> Option<&FileTypeContext> {
        self.file_ctx.get(path)
    }

    /// 按 FQN 查询 typealias 的声明信息（用于别名展开与循环检测）。
    pub fn type_alias(&self, fqn: &str) -> Option<&TypeAliasInfo> {
        self.type_aliases.get(fqn)
    }

    /// 返回给定 nominal type 的 direct supertypes（以 FQN 形式；不包含隐式 `Any`）。
    pub fn direct_supertypes(&self, fqn: &str) -> Option<&[String]> {
        self.supertypes.get(fqn).map(|v| v.as_slice())
    }

    /// 返回所有名字为 `variant_name` 的 enum variant 候选（跨所有已收集的 enum）。
    ///
    /// 说明（T0426）：
    /// - 早期阶段我们允许通过 `Some(1)` 这种“不带 enum 前缀”的写法构造 variant；
    /// - 为避免引入完整的作用域/导入规则，本方法仅提供候选集合，
    ///   由 typecheck 决定“同名唯一”约束与报错策略。
    pub fn find_enum_variants_named(&self, variant_name: &str) -> Vec<(String, EnumVariantInfo)> {
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
        index: &Index,
    ) -> Result<(), TypeEnvError> {
        // 记录源文件内容，供后续 typecheck 在跨文件引用（例如 sysroot enum variants）时
        // 通过 span 反查标识符文本。
        self.sources
            .entry(source.path().to_path_buf())
            .or_insert_with(|| source.clone());

        let pkg_prefix = package_prefix(source, file.package.as_ref());
        self.file_ctx
            .entry(source.path().to_path_buf())
            .or_insert_with(|| FileTypeContext {
                pkg_prefix: pkg_prefix.clone(),
                imports: build_import_table_best_effort(source, file, index),
            });

        for item in &file.items {
            match item {
                ast::Item::TypeAlias(ta) => {
                    let name = source.slice(ta.name.span).to_string();
                    let fqn = join_prefix(&pkg_prefix, &name);
                    self.insert_symbol(
                        fqn.clone(),
                        TypeSymbol {
                            kind: TypeSymbolKind::TypeAlias,
                            type_param_count: 0,
                            eff_param: None,
                            type_param_names: Vec::new(),
                            type_param_variances: Vec::new(),
                            where_constraints: Vec::new(),
                            span: ta.name.span,
                            decl_file: source.path().to_path_buf(),
                        },
                    )?;

                    // 记录别名声明：用于 TypeRef lowering 阶段展开 alias（T0446）。
                    self.type_aliases.insert(
                        fqn,
                        TypeAliasInfo {
                            decl_file: source.path().to_path_buf(),
                            name_span: ta.name.span,
                            ty: ta.ty.clone(),
                        },
                    );
                }
                ast::Item::Type(ty) => {
                    self.collect_type_decl(source, file, &pkg_prefix, ty, index)?;
                }
                ast::Item::Object(obj) => {
                    self.collect_object_decl(source, file, &pkg_prefix, obj, index)?;
                }
                ast::Item::Fun(_) | ast::Item::Val(_) | ast::Item::ExtensionProperty(_) => {}
            }
        }

        Ok(())
    }

    fn collect_type_decl(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
        prefix: &str,
        decl: &ast::TypeDecl,
        index: &Index,
    ) -> Result<(), TypeEnvError> {
        let name = source.slice(decl.name.span).to_string();
        let fqn = join_prefix(prefix, &name);
        let type_params: Vec<String> = decl
            .type_params
            .iter()
            .map(|p| source.slice(p.name.span).to_string())
            .collect();
        let type_param_variances: Vec<Option<ast::TypeParamVariance>> =
            decl.type_params.iter().map(|p| p.variance).collect();

        let mut where_constraints: Vec<WhereConstraintInfo> = Vec::new();
        if let Some(w) = &decl.where_clause {
            // type param name -> index
            let mut idx_of: HashMap<&str, usize> = HashMap::new();
            for (idx, name) in type_params.iter().enumerate() {
                idx_of.insert(name.as_str(), idx);
            }

            for c in &w.constraints {
                let name = source.slice(c.ty_param.span);
                let Some(&param_index) = idx_of.get(name) else {
                    // resolver/typecheck 会给出更精确的诊断；这里保持健壮性。
                    continue;
                };
                where_constraints.push(WhereConstraintInfo {
                    span: c.span,
                    param_index,
                    bound: c.bound.clone(),
                });
            }
        }

        self.insert_symbol(
            fqn.clone(),
            TypeSymbol {
                kind: TypeSymbolKind::Nominal(decl.kind),
                type_param_count: decl.type_params.len(),
                eff_param: decl.eff_param.as_ref().map(|p| EffParamInfo {
                    span: p.span,
                    name: source.slice(p.name.span).to_string(),
                    default: p.default.clone(),
                }),
                type_param_names: type_params.clone(),
                type_param_variances,
                where_constraints,
                span: decl.name.span,
                decl_file: source.path().to_path_buf(),
            },
        )?;

        // 记录 direct supertypes（用于后续最小子类型/boxing 判断）。
        //
        // 注意：
        // - 当前只存储 “解析后的 FQN”，不存储 type args（更完整的泛型超类型实例化留给后续任务）。
        // - 不包含隐式 `Any`；`Any` 的顶类型语义由 typecheck 单独处理。
        let mut supers: Vec<String> = Vec::new();
        for st in &decl.supertypes {
            if let Some(st_fqn) = index.type_ref_to_fqn_in_file(source, file, &st.ty) {
                supers.push(st_fqn);
            }
        }
        supers.sort();
        supers.dedup();
        if !supers.is_empty() {
            self.supertypes.insert(fqn.clone(), supers);
        }

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
            match member {
                ast::TypeMember::Type(nested) => {
                    self.collect_type_decl(source, file, &fqn, nested, index)?;
                }
                ast::TypeMember::Object(obj) => {
                    self.collect_object_decl(source, file, &fqn, obj, index)?;
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }

        Ok(())
    }

    fn collect_object_decl(
        &mut self,
        source: &SourceFile,
        file: &ast::File,
        prefix: &str,
        obj: &ast::ObjectDecl,
        index: &Index,
    ) -> Result<(), TypeEnvError> {
        let (name, name_span) = match &obj.name {
            Some(name) => (source.slice(name.span).to_string(), name.span),
            None => match obj.kind {
                ast::ObjectKind::Companion => ("Companion".to_string(), obj.span),
                ast::ObjectKind::Object => {
                    // parser 会拒绝 `object { ... }` 这类非法语法；这里作为防御性兜底忽略。
                    return Ok(());
                }
            },
        };

        // Kotlin-like：object 在 type 层面表现为一个“无构造器的 class-like nominal type”。
        // 说明：真实的初始化时机、存储与 codegen/runtime 语义留给后续阶段处理。
        let fqn = join_prefix(prefix, &name);
        self.insert_symbol(
            fqn.clone(),
            TypeSymbol {
                kind: TypeSymbolKind::Nominal(ast::TypeKind::Class),
                type_param_count: 0,
                eff_param: None,
                type_param_names: Vec::new(),
                type_param_variances: Vec::new(),
                where_constraints: Vec::new(),
                span: name_span,
                decl_file: source.path().to_path_buf(),
            },
        )?;

        // 记录 direct supertypes（与 nominal type 一致；不包含隐式 `Any`）。
        let mut supers: Vec<String> = Vec::new();
        for st in &obj.supertypes {
            if let Some(st_fqn) = index.type_ref_to_fqn_in_file(source, file, &st.ty) {
                supers.push(st_fqn);
            }
        }
        supers.sort();
        supers.dedup();
        if !supers.is_empty() {
            self.supertypes.insert(fqn.clone(), supers);
        }

        let Some(body) = &obj.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    self.collect_type_decl(source, file, &fqn, nested, index)?;
                }
                ast::TypeMember::Object(nested) => {
                    self.collect_object_decl(source, file, &fqn, nested, index)?;
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
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

fn build_import_table_best_effort(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
) -> ImportTable {
    let mut table = ImportTable::default();

    for import in &file.imports {
        let path = import
            .path
            .iter()
            .map(|id| source.slice(id.span))
            .collect::<Vec<_>>()
            .join(".");

        if import.has_star {
            table.star.push(path);
            continue;
        }

        let local = import
            .alias
            .as_ref()
            .map(|id| source.slice(id.span))
            .or_else(|| import.path.last().map(|id| source.slice(id.span)))
            .unwrap_or("")
            .to_string();

        // 只把确实存在且在当前文件可见的 type symbol 写入 type 命名空间的显式 import 表。
        if let Some(syms) = index.by_fqn.get(&path) {
            if let Some(sym) = syms.ty.as_ref() {
                if is_symbol_visible_from(source, sym) {
                    table.ty.explicit.entry(local).or_default().push(path);
                }
            }
        }
    }

    // 稳定化（便于 Debug/测试 & 未来可能的缓存命中）。
    table.star.sort();
    table.star.dedup();
    for v in table.ty.explicit.values_mut() {
        v.sort();
        v.dedup();
    }

    table
}

fn is_symbol_visible_from(source: &SourceFile, symbol: &crate::resolve::Symbol) -> bool {
    match symbol.visibility {
        Visibility::Public | Visibility::Internal => true,
        Visibility::Private => symbol.decl_file == source.path(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::session::Session;

    #[test]
    fn sysroot_type_env_contains_option_arity() {
        let sess = Session::new().unwrap();
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        let index = Index::build(&pairs).unwrap();
        let env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();

        assert_eq!(env.type_param_count("scoop.core.Option"), Some(1));
    }

    #[test]
    fn sysroot_type_env_collects_option_variants() {
        let sess = Session::new().unwrap();
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        let index = Index::build(&pairs).unwrap();
        let env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();

        let decl = env
            .enum_decl("scoop.core.Option")
            .expect("Option 应当是 enum");
        let source = env
            .source(&decl.decl_file)
            .expect("sysroot source 应当已被收集进 TypeEnv");

        assert_eq!(decl.type_params, vec!["T".to_string()]);
        assert_eq!(decl.variants.len(), 2);

        let some = &decl.variants[0];
        assert_eq!(some.name, "Some");
        assert_eq!(some.tag, 0);
        assert_eq!(some.fields.len(), 1);
        assert_eq!(some.fields[0].name, "value");
        match &some.fields[0].ty {
            ast::TypeRef::Path(p) => {
                assert_eq!(p.segments.len(), 1);
                assert_eq!(source.slice(p.segments[0].span), "T");
            }
            other => panic!("Option.Some.value: 期望 TypeRef::Path，但得到 {other:?}"),
        }

        let none = &decl.variants[1];
        assert_eq!(none.name, "None");
        assert_eq!(none.tag, 1);
        assert!(none.fields.is_empty());
    }

    #[test]
    fn type_env_collects_where_constraints_for_type_decl() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            r#"
package fixtures.typecheck

interface Show {}

struct Bad {}

class Box<T> where T: Show {}
"#,
        );
        let ast = sess.parse(&src).unwrap();

        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&src, &ast));

        let index = Index::build(&pairs).unwrap();

        let mut env = TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(&src, &ast, &index).unwrap();

        let sym = env
            .type_symbol("fixtures.typecheck.Box")
            .expect("Box 应当被收集进 TypeEnv");
        assert_eq!(sym.type_param_names, vec!["T".to_string()]);
        assert_eq!(sym.where_constraints.len(), 1);
        assert_eq!(sym.where_constraints[0].param_index, 0);

        match &sym.where_constraints[0].bound {
            ast::TypeRef::Path(p) => {
                assert_eq!(p.segments.len(), 1);
                assert_eq!(src.slice(p.segments[0].span), "Show");
            }
            other => panic!("where bound: 期望 TypeRef::Path，但得到 {other:?}"),
        }
    }
}
