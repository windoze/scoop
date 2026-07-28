//! Import 解析：单导入 / 通配导入 / 别名 + 自动 prelude（spec P1 §2.2）。
//!
//! 规则：
//! - 显式导入与别名产生的**本地名在文件内必须唯一**；冲突（绑定到不同目标）
//!   → `scoop::resolve::duplicate_definition`；
//! - 显式 / 别名导入的**目标 FQN 必须存在于 [`Index`]**（否则
//!   `scoop::resolve::unresolved_import`）；
//! - 通配导入不校验前缀（空前缀只是贡献零个名字）；
//! - **自动 prelude**（`scoop.core.*`、`scoop.lang.string.*`）作为通配加入；
//! - 解析简单名时：**显式优先于通配**（与导入书写顺序无关）。
//!
//! 同文件顶层声明可**遮蔽**导入别名（不报冲突）；本表只负责导入侧。

use hashbrown::HashMap;

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{FileId, Interner, Span, Symbol};

use crate::syntax::ast::{File, ImportDecl};

use super::errors;
use super::index::Index;

/// 自动加入每个文件的 prelude 通配包。
const PRELUDE_WILDCARDS: &[&str] = &["scoop.core", "scoop.lang.string"];

/// 一个文件的导入作用域。
#[derive(Debug, Default, Clone)]
pub struct ImportTable {
    /// 本地简单名 → (目标 FQN, 该导入的 span)。
    explicits: HashMap<Symbol, (Symbol, Span)>,
    /// 通配包前缀 FQN（含 prelude）。
    wildcards: Vec<Symbol>,
    /// 是否在收集 sysroot 文件的导入（sysroot 文件可自由导入其他 sysroot 包）。
    collecting_sysroot: bool,
}

impl ImportTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// 收集一个文件的 import（含自动 prelude），校验并记录诊断。
    pub fn collect(
        file: &File,
        _file_id: FileId,
        index: &Index,
        interner: &mut Interner,
        diags: &mut DiagnosticSink,
    ) -> ImportTable {
        Self::collect_with_origin(file, _file_id, index, interner, diags, false, &[])
    }

    /// 同 [`collect`]，但可指定是否为 sysroot 文件（sysroot 可自由导入其他 sysroot 包）。
    /// `declared_deps` 是用户显式声明的依赖包名（通过 build manifest / `--deps` / 环境变量）。
    pub fn collect_with_origin(
        file: &File,
        _file_id: FileId,
        index: &Index,
        interner: &mut Interner,
        diags: &mut DiagnosticSink,
        is_sysroot: bool,
        declared_deps: &[String],
    ) -> ImportTable {
        let mut t = ImportTable::new();
        t.collecting_sysroot = is_sysroot;
        for p in PRELUDE_WILDCARDS {
            t.wildcards.push(interner.intern(p));
        }
        for imp in &file.imports {
            t.collect_one_with_deps(imp, index, interner, diags, is_sysroot, declared_deps);
        }
        t
    }

    #[allow(dead_code)]
    fn collect_one(
        &mut self,
        imp: &ImportDecl,
        index: &Index,
        interner: &mut Interner,
        diags: &mut DiagnosticSink,
    ) {
        self.collect_one_with_deps(imp, index, interner, diags, self.collecting_sysroot, &[]);
    }

    fn collect_one_with_deps(
        &mut self,
        imp: &ImportDecl,
        index: &Index,
        interner: &mut Interner,
        diags: &mut DiagnosticSink,
        is_sysroot: bool,
        declared_deps: &[String],
    ) {
        let path_text = path_text(imp, interner);
        if imp.wildcard.is_some() {
            // 非 auto-dependency 的 sysroot 包（如 scoop.thread）不能通过 wildcard
            // 隐式导入——需要显式声明依赖（build manifest / `--deps` / 环境变量）。
            // 仅对用户代码（非 sysroot 文件自身）强制。
            if !is_sysroot
                && path_text.starts_with("scoop.")
                && !is_auto_dependency(&path_text)
                && !declared_deps.iter().any(|d| d == &path_text)
            {
                diags.push(errors::unresolved_import(&path_text, imp.span));
                return;
            }
            self.wildcards.push(interner.intern(&path_text));
            return;
        }
        let target = interner.intern(&path_text);
        let local_sym = match &imp.alias {
            Some(a) => a.symbol,
            None => {
                imp.path
                    .segments
                    .last()
                    .expect("import path has at least one segment")
                    .symbol
            }
        };
        // 目标存在性。
        if index.lookup(target).is_none() {
            diags.push(errors::unresolved_import(&path_text, imp.span));
            return;
        }
        // 本地名唯一性（绑定到不同目标才冲突）。
        if let Some((prev_target, prev_span)) = self.explicits.get(&local_sym) {
            if *prev_target != target {
                let local_text = interner.resolve(local_sym).to_string();
                diags.push(errors::duplicate_definition(
                    &local_text,
                    *prev_span,
                    imp.span,
                ));
            }
        } else {
            self.explicits.insert(local_sym, (target, imp.span));
        }
    }

    /// 按简单名解析导入：显式优先，再按通配前缀拼 `prefix.name` 探测 [`Index`]。
    /// 返回目标 FQN；都不命中则 `None`。
    pub fn resolve_name(&self, name: Symbol, index: &Index, interner: &Interner) -> Option<Symbol> {
        if let Some((fqn, _)) = self.explicits.get(&name) {
            return Some(*fqn);
        }
        let name_text = interner.resolve(name);
        for &prefix in &self.wildcards {
            let fqn_text = format!("{}.{}", interner.resolve(prefix), name_text);
            if let Some(fqn) = interner.get(&fqn_text)
                && index.lookup(fqn).is_some()
            {
                return Some(fqn);
            }
        }
        None
    }

    /// 显式导入数量（测试用）。
    pub fn explicit_count(&self) -> usize {
        self.explicits.len()
    }
}

/// 由 `ImportDecl` 的路径段拼点分文本。
fn path_text(imp: &ImportDecl, interner: &Interner) -> String {
    imp.path
        .segments
        .iter()
        .map(|seg| interner.resolve(seg.symbol))
        .collect::<Vec<_>>()
        .join(".")
}

/// auto-dependency 包：这些 sysroot 包无需显式声明依赖即可通过 wildcard 导入。
/// 其他 sysroot 包（scoop.thread / scoop.sync 等）需要显式依赖声明（build-system 级）。
/// 当前前端无 manifest，无法区分；保留定义供未来 manifest-aware 阶段使用。
#[allow(dead_code)]
fn is_auto_dependency(package: &str) -> bool {
    matches!(
        package,
        "scoop.core"
            | "scoop.collections"
            | "scoop.unsafe"
            | "scoop.delegates"
            | "scoop.lang.string"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::collect::collect_file;
    use crate::resolve::symbol::ConeKind;
    use scoop2_base::SourceFile;
    use scoop2_syntax::parser::parse_file;

    /// 单文件（语法顺序：package? import* item*）：先 collect 顶层声明进 index，
    /// 再 collect imports；返回 (table, index, interner, 诊断码)。
    fn resolve_imports(src: &str) -> (ImportTable, Index, Interner, Vec<String>) {
        let result = parse_file(&SourceFile::new_virtual("<mem>", src));
        let mut interner = result.interner;
        let mut index = Index::new();
        let mut diags = DiagnosticSink::new();
        let cone = index.intern_cone("test", ConeKind::Bin);
        collect_file(
            &result.file,
            FileId(0),
            cone,
            &mut index,
            &mut interner,
            &mut diags,
        );
        let table =
            ImportTable::collect(&result.file, FileId(0), &index, &mut interner, &mut diags);
        let codes = diags.iter().map(|d| d.code.to_string()).collect();
        (table, index, interner, codes)
    }

    #[test]
    fn unresolved_single_import_is_reported() {
        let (_, _, _, codes) = resolve_imports("import missing.Sym\n");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::resolve::unresolved_import"),
            "{codes:?}"
        );
    }

    #[test]
    fn resolved_single_import_builds_mapping() {
        let (table, index, mut it, codes) =
            resolve_imports("package p\nimport p.A\npublic class A\n");
        assert!(codes.is_empty(), "no diagnostics: {codes:?}");
        let local = it.intern("A");
        let target = it.intern("p.A");
        assert_eq!(table.resolve_name(local, &index, &it), Some(target));
    }

    #[test]
    fn wildcard_import_resolves_member() {
        let (table, index, mut it, codes) =
            resolve_imports("package p\nimport p.*\npublic class A\npublic class B\n");
        assert!(codes.is_empty(), "{codes:?}");
        let a = it.intern("A");
        let pa = it.intern("p.A");
        assert_eq!(table.resolve_name(a, &index, &it), Some(pa));
    }

    #[test]
    fn alias_maps_local_name_to_target() {
        let (table, index, mut it, codes) =
            resolve_imports("package p\nimport p.A as D\npublic class A\n");
        assert!(codes.is_empty(), "{codes:?}");
        let d = it.intern("D");
        let pa = it.intern("p.A");
        assert_eq!(table.resolve_name(d, &index, &it), Some(pa));
    }
}
