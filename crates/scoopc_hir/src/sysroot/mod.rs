//! Sysroot 加载（AST 持有层）。
//!
//! Path/manifest 级 sysroot 工具由 `scoop_project_model::sysroot` 持有。
//! 本模块在那个基础上构建“文件 + AST 索引”的 [`Sysroot`]，供 HIR/typecheck
//! 阶段消费。

use std::path::{Path, PathBuf};

use miette::{Context as _, Result, miette};

pub use scoop_project_model::sysroot::{
    DEFAULT_AUTO_DEPENDENCY_CONES, SYSROOT_OVERLAY_ENV, SysrootSourceConePackage,
    SysrootSourceEntry, canonicalize_sysroot_root, collect_auto_sysroot_source_cone_packages,
    collect_auto_sysroot_source_cone_packages_for_platform, collect_auto_sysroot_source_entries,
    collect_auto_sysroot_source_entries_for_platform, collect_merged_sysroot_entries,
    collect_sysroot_files, collect_sysroot_source_cone_packages,
    collect_sysroot_source_cone_packages_for_platform, default_sysroot_path,
    select_auto_sysroot_source_cone_packages, sysroot_source_cone_names,
};

use crate::source::SourceFile;

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
    /// 默认 sysroot 路径（开发期路径：相对 workspace 根的 `sysroot/`）。
    pub fn default_path() -> PathBuf {
        default_sysroot_path()
    }

    pub fn load_from(root: impl AsRef<Path>) -> Result<Self> {
        Self::load_from_with_overlay(root, None)
    }

    pub fn load_from_with_overlay(
        root: impl AsRef<Path>,
        overlay_root: Option<&Path>,
    ) -> Result<Self> {
        let root = canonicalize_sysroot_root(root.as_ref(), "sysroot")?;
        let entries = collect_merged_sysroot_entries(&root, overlay_root)?;
        Self::load_from_entries(root, entries)
    }

    pub fn load_auto_from_with_overlay_and_dependencies(
        root: impl AsRef<Path>,
        overlay_root: Option<&Path>,
        extra_dependency_names: &[String],
    ) -> Result<Self> {
        let target_platform = crate::target::TargetPlatform::host();
        Self::load_auto_from_with_overlay_dependencies_and_platform(
            root,
            overlay_root,
            extra_dependency_names,
            target_platform.id(),
        )
    }

    pub fn load_auto_from_with_overlay_dependencies_and_platform(
        root: impl AsRef<Path>,
        overlay_root: Option<&Path>,
        extra_dependency_names: &[String],
        target_platform: &str,
    ) -> Result<Self> {
        let root = canonicalize_sysroot_root(root.as_ref(), "sysroot")?;
        let entries = collect_auto_sysroot_source_entries_for_platform(
            &root,
            overlay_root,
            extra_dependency_names,
            target_platform,
        )?;
        Self::load_from_entries(root, entries)
    }

    fn load_from_entries(root: PathBuf, entries: Vec<SysrootSourceEntry>) -> Result<Self> {
        if entries.is_empty() {
            return Err(miette!(
                "sysroot/lib 下没有可加载的 source cone：{}",
                root.display()
            ));
        }

        let mut files = Vec::new();
        for entry in entries {
            let path = entry.path;
            let source = load_sysroot_source(&path, entry.trusted_syslib)?;
            let ast = crate::parser::parse_file(&source)
                .wrap_err_with(|| format!("解析 sysroot 文件失败：{}", path.display()))?;
            files.push(SysrootFile { path, source, ast });
        }

        Ok(Self { root, files })
    }

    pub fn index_files(&self) -> impl Iterator<Item = &SysrootFile> {
        self.files.iter()
    }
}

fn load_sysroot_source(path: &Path, trusted_syslib: bool) -> Result<SourceFile> {
    if trusted_syslib {
        SourceFile::load_trusted_syslib(path)
    } else {
        SourceFile::load_sysroot(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::cone::{CONE_TOML_FILE_NAME, ConeKind};
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

    fn write_sysroot_cone_manifest(root: &Path, name: &str, kind: ConeKind) {
        write_file(
            &root.join("lib").join(name).join(CONE_TOML_FILE_NAME),
            &format!(
                "[cone]\nname = \"{name}\"\nversion = \"0.0.0\"\nkind = \"{}\"\n",
                kind.as_str()
            ),
        );
    }

    #[test]
    fn load_default_sysroot() {
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

        for entry in collect_merged_sysroot_entries(&root, None).unwrap() {
            let path = entry.path;
            let source = load_sysroot_source(&path, entry.trusted_syslib).unwrap();
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
        let base_core = base_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let overlay_core = overlay_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let unsafe_file = base_root
            .join("lib")
            .join("scoop.unsafe")
            .join("src")
            .join("unsafe.scoop");

        write_sysroot_cone_manifest(&base_root, "scoop.core", ConeKind::Syslib);
        write_sysroot_cone_manifest(&base_root, "scoop.unsafe", ConeKind::Syslib);

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
    fn legacy_overlay_paths_outside_lib_cones_are_ignored() {
        let root = make_temp_dir("legacy_overlay_ignored");
        let base_root = root.0.join("base");
        let overlay_root = root.0.join("overlay");
        let base_core = base_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let legacy_overlay = overlay_root
            .join("fixtures")
            .join("typecheck")
            .join("intrinsic_surface.scoop");
        let overlay_docs = overlay_root.join("docs").join("ignored.scoop");

        write_sysroot_cone_manifest(&base_root, "scoop.core", ConeKind::Syslib);
        write_file(
            &base_core,
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(
            &legacy_overlay,
            "package fixtures.typecheck\n@Intrinsic fun legacy(): Int\n",
        );
        write_file(
            &overlay_docs,
            "package docs\nfun shouldNotLoad(): Int = 1\n",
        );

        let sysroot = Sysroot::load_from_with_overlay(&base_root, Some(&overlay_root)).unwrap();
        let base_core = base_core.canonicalize().unwrap();
        let legacy_overlay = legacy_overlay.canonicalize().unwrap();
        let overlay_docs = overlay_docs.canonicalize().unwrap();

        assert_eq!(sysroot.files.len(), 1);
        assert!(sysroot.files.iter().any(|file| file.path == base_core));
        assert!(!sysroot.files.iter().any(|file| file.path == legacy_overlay));
        assert!(!sysroot.files.iter().any(|file| file.path == overlay_docs));
    }

    #[test]
    fn sysroot_docs_scoop_files_are_ignored() {
        let root = make_temp_dir("docs_ignored");
        let base_root = root.0.join("base");
        let core_file = base_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let docs_file = base_root.join("docs").join("foo.scoop");

        write_sysroot_cone_manifest(&base_root, "scoop.core", ConeKind::Syslib);
        write_file(
            &core_file,
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(&docs_file, "package docs\nfun shouldNotLoad(): Int = 1\n");

        let sysroot = Sysroot::load_from(&base_root).unwrap();
        let core_file = core_file.canonicalize().unwrap();
        let docs_file = docs_file.canonicalize().unwrap();

        assert_eq!(sysroot.files.len(), 1);
        assert!(sysroot.files.iter().any(|file| file.path == core_file));
        assert!(!sysroot.files.iter().any(|file| file.path == docs_file));

        let mut support_sources = Vec::new();
        collect_sysroot_files(&base_root, None, &mut support_sources).unwrap();
        assert_eq!(support_sources, vec![core_file]);
    }

    #[test]
    fn sysroot_manifest_kind_controls_source_trust() {
        let root = make_temp_dir("manifest_kind_trust");
        let base_root = root.0.join("base");
        let core_file = base_root
            .join("lib")
            .join("scoop.core")
            .join("src")
            .join("core.scoop");
        let string_file = base_root
            .join("lib")
            .join("scoop.lang.string")
            .join("src")
            .join("lang_string.scoop");

        write_sysroot_cone_manifest(&base_root, "scoop.core", ConeKind::Syslib);
        write_sysroot_cone_manifest(&base_root, "scoop.lang.string", ConeKind::Lib);
        write_file(
            &core_file,
            "package scoop.core\n@Intrinsic class Array<T>\ninterface Any\n",
        );
        write_file(
            &string_file,
            "package scoop.lang.string\npublic class StringBuilder\n",
        );

        let sysroot = Sysroot::load_from(&base_root).unwrap();
        let core_file = core_file.canonicalize().unwrap();
        let string_file = string_file.canonicalize().unwrap();

        let core = sysroot
            .files
            .iter()
            .find(|file| file.path == core_file)
            .unwrap();
        let string = sysroot
            .files
            .iter()
            .find(|file| file.path == string_file)
            .unwrap();

        assert!(core.source.is_trusted_syslib());
        assert!(string.source.is_sysroot());
        assert!(!string.source.is_trusted_syslib());

        let overlay_root = root.0.join("overlay");
        let string_overlay_extra = overlay_root
            .join("lib")
            .join("scoop.lang.string")
            .join("src")
            .join("extra.scoop");
        write_file(
            &string_overlay_extra,
            "package scoop.lang.string\npublic fun overlayStringToken(): Int = 1\n",
        );

        let overlaid = Sysroot::load_from_with_overlay(&base_root, Some(&overlay_root)).unwrap();
        let string_overlay_extra = string_overlay_extra.canonicalize().unwrap();
        let extra = overlaid
            .files
            .iter()
            .find(|file| file.path == string_overlay_extra)
            .unwrap();

        assert!(extra.source.is_sysroot());
        assert!(!extra.source.is_trusted_syslib());
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
                ast::Item::TypeAlias(_) | ast::Item::ExtensionProperty(_) | ast::Item::Val(_) => {}
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
