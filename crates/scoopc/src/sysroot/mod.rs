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

/// T0143：sysroot 文件分为两类：
/// - `files`：签名文件（仅声明，不作为 support source 编译）。用于 production Index 起点。
/// - `compilable_source_paths`：含有函数体的 sysroot 文件（如 `string.scoop`），
///   需作为编译单元的一部分参与完整的 resolve → typecheck → HIR lowering → codegen 管线。
/// - `compilable_files`：已解析的 compilable sysroot 文件，供 dump / package API export / 单测
///   这类不经 support-source 输入的路径参与索引。
#[derive(Debug)]
pub struct Sysroot {
    pub root: PathBuf,
    pub files: Vec<SysrootFile>,
    /// T0143：需要被编译（而非仅作为签名索引）的 sysroot 源文件路径。
    pub compilable_source_paths: Vec<PathBuf>,
    /// 与 `compilable_source_paths` 对应的已解析 sysroot 文件。
    ///
    /// 说明：这些文件不放入 `files`，以避免 production frontend 在 support-source
    /// 输入中再次加入同一文件时形成重复声明；需要完整 sysroot 索引的路径应使用
    /// `index_files()`。
    pub compilable_files: Vec<SysrootFile>,
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
        let mut compilable_source_paths = Vec::new();
        let mut compilable_files = Vec::new();
        for path in paths {
            let source = SourceFile::load_sysroot(&path)?;
            let ast = crate::parser::parse_file(&source)
                .wrap_err_with(|| format!("解析 sysroot 文件失败：{}", path.display()))?;

            // T0143：含有真实实现体的 sysroot 文件需要走完整编译管线，而非仅作为签名索引。
            // 除了既有的 `string/print/task.scoop` 之外，overlay 的 `core.scoop` 若声明了
            // bodied `@Intrinsic struct/class` 也必须进入 compilable support sources。
            if is_compilable_sysroot_file(&path, &source, &ast) {
                compilable_source_paths.push(path.clone());
                compilable_files.push(SysrootFile {
                    path: path.clone(),
                    source: source.clone(),
                    ast: ast.clone(),
                });
                if path.file_name().is_some_and(|name| name == "core.scoop") {
                    files.push(SysrootFile {
                        path,
                        source,
                        ast: signature_only_sysroot_ast(ast),
                    });
                }
                continue;
            }

            files.push(SysrootFile { path, source, ast });
        }

        {
            let mut source_clones = files
                .iter()
                .map(|file| file.source.clone())
                .collect::<Vec<_>>();
            source_clones.extend(compilable_files.iter().map(|file| file.source.clone()));
            let indexed_sources = source_clones
                .iter()
                .map(|source| (crate::resolve::ConeId::DEFAULT, source))
                .collect::<Vec<_>>();
            let mut ast_refs = files
                .iter_mut()
                .map(|file| &mut file.ast)
                .collect::<Vec<_>>();
            ast_refs.extend(compilable_files.iter_mut().map(|file| &mut file.ast));
            crate::comptime::trim_package_level_comptime_ifs_in_indexed_compilation_unit(
                &[],
                &indexed_sources,
                &mut ast_refs,
            )
            .wrap_err("裁剪 sysroot compilation-unit 的 package-level comptime if 失败")?;
        }

        Ok(Self {
            root,
            files,
            compilable_source_paths,
            compilable_files,
        })
    }

    pub fn index_files(&self) -> impl Iterator<Item = &SysrootFile> {
        self.files.iter()
    }
}

/// T0143：判断 sysroot 文件是否需要作为编译单元的一部分（而非仅签名索引）。
/// 当前规则：
/// - `core.scoop`、`string.scoop`、`print.scoop`、`scalar_string_bridge.scoop` 与 `task.scoop`
///   一直是 support sources；
/// - overlay 的 `core.scoop` 若声明了 bodied `@Intrinsic struct/class`，也要进入完整编译管线。
fn is_compilable_sysroot_file(path: &Path, source: &SourceFile, ast: &crate::ast::File) -> bool {
    is_always_compilable_sysroot_file(path) || has_bodied_intrinsic_nominal_method(source, ast)
}

fn is_always_compilable_sysroot_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        name == "string.scoop"
            || name == "core.scoop"
            || name == "print.scoop"
            || name == "scalar_string_bridge.scoop"
            || name == "task.scoop"
    })
}

fn has_bodied_intrinsic_nominal_method(source: &SourceFile, ast: &crate::ast::File) -> bool {
    ast.items.iter().any(|item| {
        let crate::ast::Item::Type(decl) = item else {
            return false;
        };
        matches!(
            decl.kind,
            crate::ast::TypeKind::Struct | crate::ast::TypeKind::Class
        ) && has_intrinsic_annotation(source, &decl.annotations)
            && decl.body.as_ref().is_some_and(|body| {
                body.members.iter().any(|member| {
                    let crate::ast::TypeMember::Fun(fun) = member else {
                        return false;
                    };
                    matches!(fun.body, crate::ast::FunBody::Block(_))
                })
            })
    })
}

fn has_intrinsic_annotation(
    source: &SourceFile,
    annotations: &[crate::ast::AnnotationUse],
) -> bool {
    annotations.iter().any(|annotation| {
        let segments = annotation
            .path
            .iter()
            .map(|ident| ident.text(source))
            .collect::<Vec<_>>();
        matches!(
            segments.as_slice(),
            ["Intrinsic"] | ["scoop", "core", "Intrinsic"]
        )
    })
}

pub(crate) fn signature_only_sysroot_ast(mut ast: crate::ast::File) -> crate::ast::File {
    for item in &mut ast.items {
        strip_item_bodies(item);
    }
    ast
}

fn strip_item_bodies(item: &mut crate::ast::Item) {
    match item {
        crate::ast::Item::Fun(fun) => {
            fun.body = crate::ast::FunBody::Missing;
        }
        crate::ast::Item::Type(decl) => strip_type_decl_bodies(decl),
        crate::ast::Item::Object(object) => strip_object_decl_bodies(object),
        crate::ast::Item::ComptimeIf(item) => {
            for nested in &mut item.then_branch.items {
                strip_item_bodies(nested);
            }
            if let Some(else_branch) = item.else_branch.as_deref_mut() {
                strip_comptime_else_bodies(else_branch);
            }
        }
        crate::ast::Item::TypeAlias(_)
        | crate::ast::Item::ExtensionProperty(_)
        | crate::ast::Item::Val(_) => {}
    }
}

fn strip_comptime_else_bodies(else_branch: &mut crate::ast::ComptimeIfItemElse) {
    match else_branch {
        crate::ast::ComptimeIfItemElse::Block(block) => {
            for nested in &mut block.items {
                strip_item_bodies(nested);
            }
        }
        crate::ast::ComptimeIfItemElse::If(item) => {
            for nested in &mut item.then_branch.items {
                strip_item_bodies(nested);
            }
            if let Some(next) = item.else_branch.as_deref_mut() {
                strip_comptime_else_bodies(next);
            }
        }
    }
}

fn strip_type_decl_bodies(decl: &mut crate::ast::TypeDecl) {
    if decl.kind == crate::ast::TypeKind::Interface {
        return;
    }
    let Some(body) = &mut decl.body else {
        return;
    };
    for member in &mut body.members {
        strip_type_member_bodies(member);
    }
}

fn strip_object_decl_bodies(object: &mut crate::ast::ObjectDecl) {
    let Some(body) = &mut object.body else {
        return;
    };
    for member in &mut body.members {
        strip_type_member_bodies(member);
    }
}

fn strip_type_member_bodies(member: &mut crate::ast::TypeMember) {
    match member {
        crate::ast::TypeMember::Fun(fun) => {
            fun.body = crate::ast::FunBody::Missing;
        }
        crate::ast::TypeMember::Type(decl) => strip_type_decl_bodies(decl),
        crate::ast::TypeMember::Object(object) => strip_object_decl_bodies(object),
        crate::ast::TypeMember::EnumVariant(_)
        | crate::ast::TypeMember::Property(_)
        | crate::ast::TypeMember::InitBlock(_)
        | crate::ast::TypeMember::SecondaryCtor(_) => {}
    }
}

/// T0143：收集 sysroot 中需要走完整编译管线的源文件路径。
/// 供 build pipeline 的 `load_stdlib_sources()` 调用，将这些文件与 stdlib 一同加入 `input.sources`。
pub fn collect_compilable_sysroot_files(
    root: &Path,
    overlay_root: Option<&Path>,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    for path in collect_merged_sysroot_paths(root, overlay_root)? {
        let source = SourceFile::load_sysroot(&path)?;
        let ast = crate::parser::parse_file(&source)
            .wrap_err_with(|| format!("解析 sysroot 文件失败：{}", path.display()))?;
        if is_compilable_sysroot_file(&path, &source, &ast) {
            out.push(path);
        }
    }
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
    use std::collections::BTreeSet;
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
    fn sysroot_declaration_only_regular_funs_are_explicitly_exempted() {
        let root = canonicalize_sysroot_root(&Sysroot::default_path(), "sysroot").unwrap();
        let mut parsed_files = Vec::new();
        let mut implemented_top_level_fqns = BTreeSet::new();
        let mut violations = Vec::new();

        for path in collect_merged_sysroot_paths(&root, None).unwrap() {
            let source = SourceFile::load_sysroot(&path).unwrap();
            let file = crate::parser::parse_file(&source).unwrap();
            collect_bodyful_top_level_regular_fqns(&source, &file, &mut implemented_top_level_fqns);
            parsed_files.push((path, source, file));
        }

        for (path, source, file) in &parsed_files {
            audit_file_for_declaration_only_regular_funs(
                path,
                source,
                file,
                &implemented_top_level_fqns,
                &mut violations,
            );
        }

        assert!(
            violations.is_empty(),
            "sysroot ordinary functions/methods without local bodies must be `@Intrinsic`, `@Extern`, abstract interface methods, or backed by a bodyful sysroot support source:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn overlay_core_with_bodied_intrinsic_nominal_method_becomes_compilable_support_source() {
        let root = make_temp_dir("overlay_core_compilable");
        let base_root = root.0.join("base");
        let overlay_root = root.0.join("overlay");
        let base_core = base_root.join("core.scoop");
        let overlay_core = overlay_root.join("core.scoop");
        let unsafe_file = base_root.join("unsafe.scoop");

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

        assert_eq!(sysroot.compilable_source_paths, vec![overlay_core.clone()]);
        assert_eq!(sysroot.files.len(), 2);
        assert!(sysroot.files.iter().any(|file| file.path == unsafe_file));
        assert!(sysroot.files.iter().any(|file| file.path == overlay_core));

        let mut support_sources = Vec::new();
        collect_compilable_sysroot_files(&base_root, Some(&overlay_root), &mut support_sources)
            .unwrap();
        assert_eq!(support_sources, vec![overlay_core]);
    }

    #[test]
    fn default_core_is_compilable_support_source_and_index_file() {
        let sysroot = Sysroot::load_from(Sysroot::default_path()).unwrap();

        assert!(
            sysroot
                .compilable_source_paths
                .iter()
                .any(|path| path.file_name().is_some_and(|name| name == "core.scoop")),
            "default core.scoop must be compiled as a support source"
        );
        assert!(
            sysroot.compilable_files.iter().any(|file| file
                .path
                .file_name()
                .is_some_and(|name| name == "core.scoop")),
            "default core.scoop must still be available for direct dump/package indexing"
        );
        assert!(
            sysroot.files.iter().any(|file| file
                .path
                .file_name()
                .is_some_and(|name| name == "core.scoop")),
            "default core.scoop signature must remain visible through sysroot.files"
        );
        assert!(
            sysroot.index_files().any(|file| file
                .path
                .file_name()
                .is_some_and(|name| name == "core.scoop")),
            "default core.scoop must remain visible through Sysroot::index_files()"
        );
    }

    fn audit_file_for_declaration_only_regular_funs(
        path: &Path,
        source: &SourceFile,
        file: &ast::File,
        implemented_top_level_fqns: &BTreeSet<String>,
        violations: &mut Vec<String>,
    ) {
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    audit_fun_for_declaration_only_regular_fun(
                        path,
                        source,
                        file,
                        fun,
                        FunAuditContainer::TopLevel,
                        implemented_top_level_fqns,
                        violations,
                    );
                }
                ast::Item::Type(decl) => {
                    audit_type_for_declaration_only_regular_funs(
                        path,
                        source,
                        file,
                        decl,
                        implemented_top_level_fqns,
                        violations,
                    );
                }
                ast::Item::Object(obj) => {
                    audit_object_for_declaration_only_regular_funs(
                        path,
                        source,
                        file,
                        obj,
                        implemented_top_level_fqns,
                        violations,
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
        implemented_top_level_fqns: &BTreeSet<String>,
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
                        file,
                        fun,
                        FunAuditContainer::Type(decl.kind),
                        implemented_top_level_fqns,
                        violations,
                    );
                }
                ast::TypeMember::Type(nested) => {
                    audit_type_for_declaration_only_regular_funs(
                        path,
                        source,
                        file,
                        nested,
                        implemented_top_level_fqns,
                        violations,
                    );
                }
                ast::TypeMember::Object(obj) => {
                    audit_object_for_declaration_only_regular_funs(
                        path,
                        source,
                        file,
                        obj,
                        implemented_top_level_fqns,
                        violations,
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
        implemented_top_level_fqns: &BTreeSet<String>,
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
                        file,
                        fun,
                        FunAuditContainer::Object,
                        implemented_top_level_fqns,
                        violations,
                    );
                }
                ast::TypeMember::Type(nested) => {
                    audit_type_for_declaration_only_regular_funs(
                        path,
                        source,
                        file,
                        nested,
                        implemented_top_level_fqns,
                        violations,
                    );
                }
                ast::TypeMember::Object(nested) => {
                    audit_object_for_declaration_only_regular_funs(
                        path,
                        source,
                        file,
                        nested,
                        implemented_top_level_fqns,
                        violations,
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
        file: &ast::File,
        fun: &ast::FunDecl,
        container: FunAuditContainer,
        implemented_top_level_fqns: &BTreeSet<String>,
        violations: &mut Vec<String>,
    ) {
        if !matches!(fun.body, ast::FunBody::Missing) || fun.kind != ast::FunDeclKind::Regular {
            return;
        }
        if matches!(container, FunAuditContainer::Type(ast::TypeKind::Interface)) {
            return;
        }
        if has_intrinsic_annotation(source, &fun.annotations)
            || has_builtin_annotation(source, &fun.annotations, "Extern")
        {
            return;
        }
        if matches!(container, FunAuditContainer::TopLevel)
            && fun.receiver.is_none()
            && implemented_top_level_fqns.contains(&top_level_fun_fqn(source, file, fun))
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

    fn collect_bodyful_top_level_regular_fqns(
        source: &SourceFile,
        file: &ast::File,
        out: &mut BTreeSet<String>,
    ) {
        for item in &file.items {
            let ast::Item::Fun(fun) = item else {
                continue;
            };
            if fun.kind == ast::FunDeclKind::Regular
                && fun.receiver.is_none()
                && matches!(fun.body, ast::FunBody::Block(_))
            {
                out.insert(top_level_fun_fqn(source, file, fun));
            }
        }
    }

    fn top_level_fun_fqn(source: &SourceFile, file: &ast::File, fun: &ast::FunDecl) -> String {
        let name = source.slice(fun.name.span);
        let Some(package) = &file.package else {
            return name.to_string();
        };
        let prefix = package
            .path
            .iter()
            .map(|segment| segment.text(source))
            .collect::<Vec<_>>()
            .join(".");
        if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}.{name}")
        }
    }
}
