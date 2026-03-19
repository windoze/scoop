//! import 解析与导入表（ImportTable）。
//!
//! 说明：
//! - Scoop 的名字解析分为 type/value（以及 fun/value 细分）多个命名空间（见 T0301）。
//! - import 本身是“名字引入”机制，但**是否能用于解析**取决于使用场景（type context vs expr/value context）。
//! - 当前阶段（T0303）仅构建 import 表：显式 import 与通配 `*` import；
//!   不在这里执行表达式中的标识符解析（后续任务再接入）。

use std::collections::BTreeMap;

use crate::{ast, source::SourceFile};

use super::{Index, ResolveError, SymbolKind};

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

        for import in &file.imports {
            let path = join_import_path(source, &import.path);

            if import.has_star {
                // 通配 import：要求至少存在某个符号在该前缀下（不区分命名空间）。
                let prefix = format!("{path}.");
                let ok = index.by_fqn.keys().any(|k| k.starts_with(&prefix));
                if !ok {
                    return Err(ResolveError::UnresolvedImport {
                        import: format!("{path}.*"),
                        span: import.span.into(),
                    });
                }

                table.star.push(path);
                continue;
            }

            let Some(syms) = index.by_fqn.get(&path) else {
                return Err(ResolveError::UnresolvedImport {
                    import: path,
                    span: import.span.into(),
                });
            };

            let Some(local) = last_segment(source, &import.path) else {
                // parser 保证 import 至少有一个 segment；这里作为防御性兜底。
                continue;
            };

            if syms.get(SymbolKind::Type).is_some() {
                table
                    .ty
                    .explicit
                    .entry(local.to_string())
                    .or_default()
                    .push(path.clone());
            }

            // value namespace：fun/value 任一存在即认为该 import 对 value 解析有意义。
            if syms.get(SymbolKind::Fun).is_some() || syms.get(SymbolKind::Value).is_some() {
                table
                    .value
                    .explicit
                    .entry(local.to_string())
                    .or_default()
                    .push(path);
            }
        }

        Ok(table)
    }
}

fn join_import_path(source: &SourceFile, path: &[ast::Ident]) -> String {
    path.iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}

fn last_segment<'a>(source: &'a SourceFile, path: &'a [ast::Ident]) -> Option<&'a str> {
    path.last().map(|id| source.slice(id.span))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use crate::source::SourceFile;

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

        assert_eq!(table.star, vec!["a".to_string()]);

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
}
