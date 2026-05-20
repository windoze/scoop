//! Sysroot 加载。
//!
//! Sysroot 是一组 `.scoop` 源文件，描述编译器内建的 API 表面：
//! - 对编译器：提供内建类型/函数/效果的签名来源
//! - 对工具链：LSP/文档可从 sysroot 读取类型信息
//!
//! 当前阶段：只实现“定位目录 + 读取 `.scoop` + 调用 parser”。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

use crate::source::SourceFile;

/// 外部 driver 可通过该环境变量为单次构建注入 sysroot overlay。
pub const SYSROOT_OVERLAY_ENV: &str = "SCOOP_SYSROOT_OVERLAY";

/// sysroot 中的所有文件都以完整 AST 参与声明索引与 support-source 编译。
#[derive(Debug)]
pub struct Sysroot {
    pub root: PathBuf,
    pub files: Vec<SysrootFile>,
}

#[derive(Debug)]
pub struct SysrootFile {
    pub path: PathBuf,
    pub source: SourceFile,
    pub ast: crate::ast::File,
}

impl Sysroot {
    /// 默认 sysroot 路径。
    ///
    /// 当前实现是“开发期路径”：相对于 `crates/scoopc` 的 `../../sysroot`。
    /// 当编译器支持安装/分发后，这里应改为：
    /// - 优先读取 `SCOOP_SYSROOT` 环境变量
    /// - 或使用可执行文件旁的资源目录
    pub fn default_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sysroot")
    }

    pub fn load_from(root: impl AsRef<Path>) -> Result<Self> {
        Self::load_from_with_overlay(root, None)
    }

    pub fn load_from_with_overlay(
        root: impl AsRef<Path>,
        overlay_root: Option<&Path>,
    ) -> Result<Self> {
        let root = canonicalize_sysroot_root(root.as_ref(), "sysroot")?;
        let paths = collect_merged_sysroot_paths(&root, overlay_root)?;
        if paths.is_empty() {
            return Err(miette!(
                "sysroot 目录下没有 .scoop 文件：{}",
                root.display()
            ));
        }

        let mut files = Vec::new();
        for path in paths {
            let source = SourceFile::load_trusted_syslib(&path)?;
            let ast = crate::parser::parse_file(&source)
                .wrap_err_with(|| format!("解析 sysroot 文件失败：{}", path.display()))?;
            files.push(SysrootFile { path, source, ast });
        }

        {
            let source_clones = files
                .iter()
                .map(|file| file.source.clone())
                .collect::<Vec<_>>();
            let indexed_sources = source_clones
                .iter()
                .map(|source| (crate::resolve::ConeId::DEFAULT, source))
                .collect::<Vec<_>>();
            let mut ast_refs = files
                .iter_mut()
                .map(|file| &mut file.ast)
                .collect::<Vec<_>>();
            crate::comptime::trim_package_level_comptime_ifs_in_indexed_compilation_unit(
                &[],
                &indexed_sources,
                &mut ast_refs,
            )
            .wrap_err("裁剪 sysroot compilation-unit 的 package-level comptime if 失败")?;
        }

        Ok(Self { root, files })
    }

    pub fn index_files(&self) -> impl Iterator<Item = &SysrootFile> {
        self.files.iter()
    }
}

/// 收集所有 sysroot 源文件路径，供 build pipeline 作为 support sources 加入 `input.sources`。
pub fn collect_sysroot_files(
    root: &Path,
    overlay_root: Option<&Path>,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    let root = canonicalize_sysroot_root(root, "sysroot")?;
    out.extend(collect_merged_sysroot_paths(&root, overlay_root)?);
    Ok(())
}

fn collect_merged_sysroot_paths(root: &Path, overlay_root: Option<&Path>) -> Result<Vec<PathBuf>> {
    let mut merged = BTreeMap::new();

    let mut base_paths = Vec::new();
    collect_scoop_files(root, &mut base_paths)?;
    for path in base_paths {
        let rel = path
            .strip_prefix(root)
            .expect("sysroot file should be under canonical root")
            .to_path_buf();
        merged.insert(rel, path);
    }

    if let Some(overlay_root) = overlay_root {
        let overlay_root = canonicalize_sysroot_root(overlay_root, "sysroot overlay")?;
        let mut overlay_paths = Vec::new();
        collect_scoop_files(&overlay_root, &mut overlay_paths)?;
        for path in overlay_paths {
            let rel = path
                .strip_prefix(&overlay_root)
                .expect("overlay file should be under canonical overlay root")
                .to_path_buf();
            merged.insert(rel, path);
        }
    }

    Ok(merged.into_values().collect())
}

fn canonicalize_sysroot_root(root: &Path, label: &str) -> Result<PathBuf> {
    let root = root.to_path_buf();
    let root = root.canonicalize().into_diagnostic().wrap_err_with(|| {
        format!(
            "无法定位 {label} 目录：{}（当前实现默认相对工作目录）",
            root.display()
        )
    })?;
    if !root.is_dir() {
        return Err(miette!("{label} 不是目录：{}", root.display()));
    }
    Ok(root)
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;
        if ty.is_dir() {
            collect_scoop_files(&path, out)?;
            continue;
        }
        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn make_temp_dir(prefix: &str) -> TempDirGuard {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "scoopc_sysroot_{prefix}_{}_{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDirGuard(dir)
    }

    fn write_file(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn load_default_sysroot() {
        // 单元测试运行时工作目录通常是 workspace 根目录。
        // 若未来变动，可改为通过 env/config 指定。
        let sysroot = Sysroot::load_from(Sysroot::default_path()).unwrap();
        assert!(!sysroot.files.is_empty());
    }

    #[test]
    fn lang_string_cone_visible_in_sysroot() {
        let sysroot = Sysroot::load_from(Sysroot::default_path()).unwrap();
        let entries = sysroot
            .index_files()
            .filter(|file| {
                has_package(&file.source, file.ast.package.as_ref(), "scoop.lang.string")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            entries.len(),
            1,
            "default sysroot must index exactly one scoop.lang.string file"
        );
        let entry = entries[0];
        assert!(
            entry.ast.items.iter().any(|item| matches!(
                item,
                ast::Item::Type(decl) if entry.source.slice(decl.name.span) == "StringBuilder"
            )),
            "scoop.lang.string must index the StringBuilder surface"
        );
        assert!(
            file_has_any_block_fun(&entry.ast),
            "scoop.lang.string must keep full method bodies in the sysroot index"
        );
    }

    #[test]
    fn sysroot_regular_funs_have_body_or_explicit_annotation() {
        let root = canonicalize_sysroot_root(&Sysroot::default_path(), "sysroot").unwrap();
        let mut parsed_files = Vec::new();
        let mut violations = Vec::new();

        for path in collect_merged_sysroot_paths(&root, None).unwrap() {
            let source = SourceFile::load_trusted_syslib(&path).unwrap();
            let file = crate::parser::parse_file(&source).unwrap();
            parsed_files.push((path, source, file));
        }

        for (path, source, file) in &parsed_files {
            audit_file_for_declaration_only_regular_funs(path, source, file, &mut violations);
        }

        assert!(
            violations.is_empty(),
            "sysroot ordinary functions/methods without local bodies must be `@Intrinsic`, `@Extern`, or abstract interface methods:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn overlay_paths_replace_base_and_all_files_become_support_sources() {
        let root = make_temp_dir("overlay_all_support_sources");
        let base_root = root.0.join("base");
        let overlay_root = root.0.join("overlay");
        let base_core = base_root.join("scoop.core").join("core.scoop");
        let overlay_core = overlay_root.join("scoop.core").join("core.scoop");
        let unsafe_file = base_root.join("scoop.unsafe").join("unsafe.scoop");

        write_file(
            &base_core,
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(&unsafe_file, "package scoop.unsafe\ninterface PtrMarker\n");
        write_file(
            &overlay_core,
            "package scoop.core\n@Intrinsic class Array<T> { fun overlayMarker(): Int { return 7 } }\ninterface Any\n",
        );

        let sysroot = Sysroot::load_from_with_overlay(&base_root, Some(&overlay_root)).unwrap();
        let overlay_core = overlay_core.canonicalize().unwrap();
        let unsafe_file = unsafe_file.canonicalize().unwrap();

        assert_eq!(sysroot.files.len(), 2);
        assert!(sysroot.files.iter().any(|file| file.path == unsafe_file));
        assert!(sysroot.files.iter().any(|file| file.path == overlay_core));

        let mut support_sources = Vec::new();
        collect_sysroot_files(&base_root, Some(&overlay_root), &mut support_sources).unwrap();
        assert_eq!(support_sources, vec![overlay_core, unsafe_file]);
    }

    #[test]
    fn default_core_is_full_ast_index_file_and_support_source() {
        let sysroot = Sysroot::load_from(Sysroot::default_path()).unwrap();
        let mut support_sources = Vec::new();
        collect_sysroot_files(&Sysroot::default_path(), None, &mut support_sources).unwrap();

        let core = sysroot
            .index_files()
            .find(|file| {
                file.path
                    .file_name()
                    .is_some_and(|name| name == "core.scoop")
            })
            .expect("default core.scoop must remain visible through Sysroot::index_files()");
        assert!(
            support_sources.contains(&core.path),
            "default core.scoop must be compiled as a support source"
        );
        assert!(
            file_has_any_block_fun(&core.ast),
            "default core.scoop must keep full bodies in Sysroot::index_files()"
        );
    }

    fn audit_file_for_declaration_only_regular_funs(
        path: &Path,
        source: &SourceFile,
        file: &ast::File,
        violations: &mut Vec<String>,
    ) {
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    audit_fun_for_declaration_only_regular_fun(
                        path,
                        source,
                        fun,
                        FunAuditContainer::TopLevel,
                        violations,
                    );
                }
                ast::Item::Type(decl) => {
                    audit_type_for_declaration_only_regular_funs(
                        path, source, file, decl, violations,
                    );
                }
                ast::Item::Object(obj) => {
                    audit_object_for_declaration_only_regular_funs(
                        path, source, file, obj, violations,
                    );
                }
                ast::Item::TypeAlias(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Val(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }

    fn audit_type_for_declaration_only_regular_funs(
        path: &Path,
        source: &SourceFile,
        file: &ast::File,
        decl: &ast::TypeDecl,
        violations: &mut Vec<String>,
    ) {
        let Some(body) = &decl.body else {
            return;
        };
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    audit_fun_for_declaration_only_regular_fun(
                        path,
                        source,
                        fun,
                        FunAuditContainer::Type(decl.kind),
                        violations,
                    );
                }
                ast::TypeMember::Type(nested) => {
                    audit_type_for_declaration_only_regular_funs(
                        path, source, file, nested, violations,
                    );
                }
                ast::TypeMember::Object(obj) => {
                    audit_object_for_declaration_only_regular_funs(
                        path, source, file, obj, violations,
                    );
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::InitBlock(_) => {}
            }
        }
    }

    fn audit_object_for_declaration_only_regular_funs(
        path: &Path,
        source: &SourceFile,
        file: &ast::File,
        obj: &ast::ObjectDecl,
        violations: &mut Vec<String>,
    ) {
        let Some(body) = &obj.body else {
            return;
        };
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    audit_fun_for_declaration_only_regular_fun(
                        path,
                        source,
                        fun,
                        FunAuditContainer::Object,
                        violations,
                    );
                }
                ast::TypeMember::Type(nested) => {
                    audit_type_for_declaration_only_regular_funs(
                        path, source, file, nested, violations,
                    );
                }
                ast::TypeMember::Object(nested) => {
                    audit_object_for_declaration_only_regular_funs(
                        path, source, file, nested, violations,
                    );
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::InitBlock(_) => {}
            }
        }
    }

    fn audit_fun_for_declaration_only_regular_fun(
        path: &Path,
        source: &SourceFile,
        fun: &ast::FunDecl,
        container: FunAuditContainer,
        violations: &mut Vec<String>,
    ) {
        if !matches!(fun.body, ast::FunBody::Missing) || fun.kind != ast::FunDeclKind::Regular {
            return;
        }
        if matches!(container, FunAuditContainer::Type(ast::TypeKind::Interface)) {
            return;
        }
        if has_builtin_annotation(source, &fun.annotations, "Intrinsic")
            || has_builtin_annotation(source, &fun.annotations, "Extern")
        {
            return;
        }
        violations.push(format!(
            "{}: `{}`",
            path.display(),
            source.slice(fun.name.span)
        ));
    }

    #[derive(Clone, Copy)]
    enum FunAuditContainer {
        TopLevel,
        Type(ast::TypeKind),
        Object,
    }

    fn has_builtin_annotation(
        source: &SourceFile,
        annotations: &[ast::AnnotationUse],
        name: &str,
    ) -> bool {
        annotations.iter().any(|annotation| {
            annotation
                .path
                .last()
                .is_some_and(|segment| segment.text(source) == name)
        })
    }

    fn file_has_any_block_fun(file: &ast::File) -> bool {
        file.items.iter().any(item_has_any_block_fun)
    }

    fn item_has_any_block_fun(item: &ast::Item) -> bool {
        match item {
            ast::Item::Fun(fun) => matches!(fun.body, ast::FunBody::Block(_)),
            ast::Item::Type(decl) => decl
                .body
                .as_ref()
                .is_some_and(|body| body.members.iter().any(type_member_has_any_block_fun)),
            ast::Item::Object(obj) => obj
                .body
                .as_ref()
                .is_some_and(|body| body.members.iter().any(type_member_has_any_block_fun)),
            ast::Item::ComptimeIf(item) => {
                item.then_branch.items.iter().any(item_has_any_block_fun)
                    || item
                        .else_branch
                        .as_deref()
                        .is_some_and(comptime_else_has_any_block_fun)
            }
            ast::Item::TypeAlias(_) | ast::Item::ExtensionProperty(_) | ast::Item::Val(_) => false,
        }
    }

    fn type_member_has_any_block_fun(member: &ast::TypeMember) -> bool {
        match member {
            ast::TypeMember::Fun(fun) => matches!(fun.body, ast::FunBody::Block(_)),
            ast::TypeMember::Type(decl) => decl
                .body
                .as_ref()
                .is_some_and(|body| body.members.iter().any(type_member_has_any_block_fun)),
            ast::TypeMember::Object(obj) => obj
                .body
                .as_ref()
                .is_some_and(|body| body.members.iter().any(type_member_has_any_block_fun)),
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::InitBlock(_) => false,
        }
    }

    fn comptime_else_has_any_block_fun(else_branch: &ast::ComptimeIfItemElse) -> bool {
        match else_branch {
            ast::ComptimeIfItemElse::Block(block) => block.items.iter().any(item_has_any_block_fun),
            ast::ComptimeIfItemElse::If(item) => {
                item.then_branch.items.iter().any(item_has_any_block_fun)
                    || item
                        .else_branch
                        .as_deref()
                        .is_some_and(comptime_else_has_any_block_fun)
            }
        }
    }

    fn has_package(
        source: &SourceFile,
        package: Option<&ast::PackageDecl>,
        expected: &str,
    ) -> bool {
        let Some(package) = package else {
            return expected.is_empty();
        };
        let actual = package
            .path
            .iter()
            .map(|segment| segment.text(source))
            .collect::<Vec<_>>()
            .join(".");
        actual == expected
    }
}
