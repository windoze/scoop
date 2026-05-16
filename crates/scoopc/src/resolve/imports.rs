//! import 解析与导入表（ImportTable）。
//!
//! 说明：
//! - Scoop 的名字解析分为 type/value（以及 fun/value 细分）多个命名空间（见 T0301）。
//! - import 本身是“名字引入”机制，但**是否能用于解析**取决于使用场景（type context vs expr/value context）。
//! - 当前阶段（T0303）仅构建 import 表：显式 import 与通配 `*` import；
//!   不在这里执行表达式中的标识符解析（后续任务再接入）。

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{ast, source::SourceFile, span::Span};

use super::{Index, ResolveError, SymbolKind};

const AUTO_PRELUDE_STAR_IMPORTS: [&str; 2] = ["scoop.core", "scoop.lang.string"];

/// 按命名空间拆分后的 import 表。
///
/// - `ty`：用于 type context 的显式 import（只包含确实存在 type symbol 的导入项）
/// - `value`：用于 value context 的显式 import（fun/value 任一存在即计入）
/// - `star`：通配 `import foo.bar.*` 的前缀（两套命名空间共用；解析时按需过滤）
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportTable {
    pub ty: ImportNamespace,
    pub value: ImportNamespace,
    pub star: Vec<String>,
}

/// 单个命名空间下的导入集合。
///
/// 设计说明：
/// - 使用 multimap（`local -> Vec<fqn>`）来允许多个同名显式 import（后续可产生歧义诊断或要求用户用 alias）。
/// - 使用 `BTreeMap` 保证 `Debug` 输出稳定，便于单测断言与 fixtures 回归。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportNamespace {
    pub explicit: BTreeMap<String, Vec<String>>,
}

impl ImportTable {
    pub fn build(
        source: &SourceFile,
        file: &ast::File,
        index: &Index,
    ) -> Result<Self, ResolveError> {
        let mut table = ImportTable::default();

        add_auto_prelude_star_imports(&mut table, source);
        if index.has_sysroot_files() {
            for path in auto_prelude_star_imports(source) {
                validate_star_import(index, path, Span::synthetic_prelude())?;
            }
        }

        // T0315/T1310：import alias 需要参与冲突检查。
        // 规则：
        // - alias 名字在对应命名空间下必须唯一（不能与其它 import 的 local 名冲突）
        // - 允许 alias 与当前文件的顶层声明同名：由“同包/本地声明优先”的 shadowing 规则处理（T1310）
        let mut import_locals_ty: HashMap<String, ImportLocalInfo> = HashMap::new();
        let mut import_locals_value: HashMap<String, ImportLocalInfo> = HashMap::new();

        for import in &file.imports {
            let path = join_import_path(source, &import.path);

            if import.has_star {
                // 通配 import：目标 package 存在即可；空 cone 会在后续阶段逐步补 export。
                validate_star_import(index, &path, import.span)?;

                table.star.push(path);
                continue;
            }

            let Some(syms) = index.by_fqn.get(&path) else {
                return Err(ResolveError::UnresolvedImport {
                    import: path,
                    span: import.span.into(),
                });
            };

            let (local, local_span, is_alias) = local_name_and_span(source, import);

            if syms.get(SymbolKind::Type).is_some() {
                check_import_alias_conflicts(&mut import_locals_ty, local, local_span, is_alias)?;

                table
                    .ty
                    .explicit
                    .entry(local.to_string())
                    .or_default()
                    .push(path.clone());
            }

            // value namespace：fun/value 任一存在即认为该 import 对 value 解析有意义。
            if syms.has_fun() || syms.get(SymbolKind::Value).is_some() {
                check_import_alias_conflicts(
                    &mut import_locals_value,
                    local,
                    local_span,
                    is_alias,
                )?;

                table
                    .value
                    .explicit
                    .entry(local.to_string())
                    .or_default()
                    .push(path);
            }
        }

        dedup_star_imports(&mut table.star);

        Ok(table)
    }
}

pub(crate) fn auto_prelude_star_imports(source: &SourceFile) -> &'static [&'static str] {
    if source.is_sysroot() {
        &[]
    } else {
        &AUTO_PRELUDE_STAR_IMPORTS
    }
}

pub(crate) fn add_auto_prelude_star_imports(table: &mut ImportTable, source: &SourceFile) {
    table.star.extend(
        auto_prelude_star_imports(source)
            .iter()
            .map(|path| (*path).to_string()),
    );
}

fn validate_star_import(index: &Index, path: &str, span: Span) -> Result<(), ResolveError> {
    if index.has_importable_prefix(path) {
        return Ok(());
    }

    Err(ResolveError::UnresolvedImport {
        import: format!("{path}.*"),
        span: span.into(),
    })
}

fn dedup_star_imports(star: &mut Vec<String>) {
    let mut seen = HashSet::new();
    star.retain(|path| seen.insert(path.clone()));
}

#[derive(Debug, Clone, Copy)]
struct ImportLocalInfo {
    first_span: Span,
    has_alias: bool,
}

fn local_name_and_span<'a>(
    source: &'a SourceFile,
    import: &'a ast::ImportDecl,
) -> (&'a str, Span, bool) {
    if let Some(alias) = &import.alias {
        return (source.slice(alias.span), alias.span, true);
    }

    let Some(local) = import.path.last() else {
        // parser 保证 import 至少有一个 segment；这里作为防御性兜底。
        return ("", import.span, false);
    };
    (source.slice(local.span), local.span, false)
}

/// 检查“alias 与其它 import 的 local 名字冲突”。
///
/// 允许多个 **非 alias** 的显式 import 同名（后续在使用点产生歧义诊断或要求用户显式 alias）。
/// 但只要其中任意一个使用了 alias，则认为用户“显式指定了 local 名字”，必须保证唯一性。
fn check_import_alias_conflicts(
    locals: &mut HashMap<String, ImportLocalInfo>,
    local: &str,
    local_span: Span,
    is_alias: bool,
) -> Result<(), ResolveError> {
    let Some(prev) = locals.get(local).copied() else {
        locals.insert(
            local.to_string(),
            ImportLocalInfo {
                first_span: local_span,
                has_alias: is_alias,
            },
        );
        return Ok(());
    };

    // 同名显式 import：只有当 alias 参与时才报错（T0315）。
    if prev.has_alias || is_alias {
        return Err(ResolveError::DuplicateDefinition {
            name: local.to_string(),
            first: prev.first_span.into(),
            second: local_span.into(),
        });
    }

    Ok(())
}

fn join_import_path(source: &SourceFile, path: &[ast::Ident]) -> String {
    path.iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use crate::source::{SourceFile, SourceOrigin};

    fn prelude_sysroot_sources() -> (SourceFile, ast::File, SourceFile, ast::File) {
        let core = SourceFile::new_virtual_with_origin(
            "<core>",
            "package scoop.core\nstruct PreludeType {}\nfun println() {}",
            SourceOrigin::Sysroot,
        );
        let lang_string = SourceFile::new_virtual_with_origin(
            "<lang_string>",
            "package scoop.lang.string",
            SourceOrigin::Sysroot,
        );
        let core_ast = parse_file(&core).unwrap();
        let lang_string_ast = parse_file(&lang_string).unwrap();
        (core, core_ast, lang_string, lang_string_ast)
    }

    #[test]
    fn auto_prelude_injects_core_for_user_file() {
        let (core, core_ast, lang_string, lang_string_ast) = prelude_sysroot_sources();
        let user = SourceFile::new_virtual("<user>", "package app\nfun use() {}");
        let user_ast = parse_file(&user).unwrap();

        let index = Index::build(&[
            (&core, &core_ast),
            (&lang_string, &lang_string_ast),
            (&user, &user_ast),
        ])
        .unwrap();
        let table = ImportTable::build(&user, &user_ast, &index).unwrap();

        assert_eq!(
            table.star,
            vec!["scoop.core".to_string(), "scoop.lang.string".to_string()]
        );
    }

    #[test]
    fn auto_prelude_skips_sysroot_file() {
        let sysroot = SourceFile::new_virtual_with_origin(
            "<sysroot>",
            "package scoop.lang.string\nfun marker() {}",
            SourceOrigin::Sysroot,
        );
        let ast = parse_file(&sysroot).unwrap();
        let index = Index::build(&[(&sysroot, &ast)]).unwrap();
        let table = ImportTable::build(&sysroot, &ast, &index).unwrap();

        assert!(table.star.is_empty());
    }

    #[test]
    fn auto_prelude_dedup_with_explicit_user_import() {
        let (core, core_ast, lang_string, lang_string_ast) = prelude_sysroot_sources();
        let user = SourceFile::new_virtual(
            "<user>",
            "package app\nimport scoop.core.*\nimport scoop.lang.string.*\nfun use() {}",
        );
        let user_ast = parse_file(&user).unwrap();

        let index = Index::build(&[
            (&core, &core_ast),
            (&lang_string, &lang_string_ast),
            (&user, &user_ast),
        ])
        .unwrap();
        let table = ImportTable::build(&user, &user_ast, &index).unwrap();

        assert_eq!(
            table.star,
            vec!["scoop.core".to_string(), "scoop.lang.string".to_string()]
        );
    }

    #[test]
    fn import_table_separates_type_and_value_explicit_imports() {
        let s1 = SourceFile::new_virtual(
            "<a>",
            "package a\nstruct Foo {}\nstruct Both {}\nfun bar() {}\nfun Both() {}",
        );
        let s2 = SourceFile::new_virtual(
            "<b>",
            "package b\nimport a.Foo\nimport a.bar\nimport a.Both\nimport a.*\nfun use() {}",
        );
        let a1 = parse_file(&s1).unwrap();
        let a2 = parse_file(&s2).unwrap();

        let index = Index::build(&[(&s1, &a1), (&s2, &a2)]).unwrap();
        let table = ImportTable::build(&s2, &a2, &index).unwrap();

        assert_eq!(
            table.star,
            vec![
                "scoop.core".to_string(),
                "scoop.lang.string".to_string(),
                "a".to_string()
            ]
        );

        assert_eq!(
            table.ty.explicit.get("Foo").unwrap(),
            &vec!["a.Foo".to_string()]
        );
        assert_eq!(
            table.value.explicit.get("bar").unwrap(),
            &vec!["a.bar".to_string()]
        );

        // 同名 type 与 fun 并存时，import 同时进入两套命名空间。
        assert_eq!(
            table.ty.explicit.get("Both").unwrap(),
            &vec!["a.Both".to_string()]
        );
        assert_eq!(
            table.value.explicit.get("Both").unwrap(),
            &vec!["a.Both".to_string()]
        );

        let dbg = format!("{table:#?}");
        assert!(dbg.contains("ImportTable"));
        assert!(dbg.contains("ty"));
        assert!(dbg.contains("value"));
        assert!(dbg.contains("Foo"));
        assert!(dbg.contains("bar"));
        assert!(dbg.contains("Both"));
    }

    #[test]
    fn import_table_uses_alias_as_local_name() {
        let s1 = SourceFile::new_virtual("<a>", "package a\nstruct Foo {}\nfun bar() {}");
        let s2 = SourceFile::new_virtual(
            "<b>",
            "package b\nimport a.Foo as Qux\nimport a.bar as baz\nfun use() {}",
        );
        let a1 = parse_file(&s1).unwrap();
        let a2 = parse_file(&s2).unwrap();

        let index = Index::build(&[(&s1, &a1), (&s2, &a2)]).unwrap();
        let table = ImportTable::build(&s2, &a2, &index).unwrap();

        assert_eq!(
            table.ty.explicit.get("Qux").unwrap(),
            &vec!["a.Foo".to_string()]
        );
        assert!(!table.ty.explicit.contains_key("Foo"));

        assert_eq!(
            table.value.explicit.get("baz").unwrap(),
            &vec!["a.bar".to_string()]
        );
        assert!(!table.value.explicit.contains_key("bar"));
    }

    #[test]
    fn import_alias_conflicts_with_another_import_is_error() {
        let s1 = SourceFile::new_virtual("<a>", "package a\nstruct Foo {}\nstruct Bar {}");
        let s2 = SourceFile::new_virtual(
            "<b>",
            "package b\nimport a.Foo as X\nimport a.Bar as X\nfun use(x: X) {}",
        );
        let a1 = parse_file(&s1).unwrap();
        let a2 = parse_file(&s2).unwrap();

        let index = Index::build(&[(&s1, &a1), (&s2, &a2)]).unwrap();
        let err = ImportTable::build(&s2, &a2, &index).unwrap_err();

        assert!(matches!(err, ResolveError::DuplicateDefinition { .. }));
    }

    #[test]
    fn import_alias_can_be_shadowed_by_local_top_level() {
        let s1 = SourceFile::new_virtual("<a>", "package a\nstruct Foo {}");
        let s2 = SourceFile::new_virtual(
            "<b>",
            "package b\nimport a.Foo as Foo\nstruct Foo {}\nfun use(x: Foo) {}",
        );
        let a1 = parse_file(&s1).unwrap();
        let a2 = parse_file(&s2).unwrap();

        let index = Index::build(&[(&s1, &a1), (&s2, &a2)]).unwrap();
        let table = ImportTable::build(&s2, &a2, &index).unwrap();

        // alias 与本地声明同名在此阶段允许：由后续解析规则决定 shadowing（T1310）。
        assert_eq!(
            table.ty.explicit.get("Foo").unwrap(),
            &vec!["a.Foo".to_string()]
        );
    }
}
