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
/// - `files`：签名文件（仅声明，不编译函数体）。用于 Index 与类型检查。
/// - `compilable_source_paths`：含有函数体的 sysroot 文件（如 `string.scoop`），
///   需作为编译单元的一部分参与完整的 resolve → typecheck → HIR lowering → codegen 管线。
///   这些文件不重复加入 `files`，以避免 Index 中出现双重声明。
#[derive(Debug)]
pub struct Sysroot {
    pub root: PathBuf,
    pub files: Vec<SysrootFile>,
    /// T0143：需要被编译（而非仅作为签名索引）的 sysroot 源文件路径。
    pub compilable_source_paths: Vec<PathBuf>,
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
        for path in paths {
            let source = SourceFile::load_sysroot(&path)?;
            let ast = crate::parser::parse_file(&source)
                .wrap_err_with(|| format!("解析 sysroot 文件失败：{}", path.display()))?;

            // T0143：含有真实实现体的 sysroot 文件需要走完整编译管线，而非仅作为签名索引。
            // 除了既有的 `string/print/task.scoop` 之外，overlay 的 `core.scoop` 若声明了
            // bodied `@Intrinsic struct/class` 也必须进入 compilable support sources。
            if is_compilable_sysroot_file(&path, &source, &ast) {
                compilable_source_paths.push(path);
                continue;
            }

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

        Ok(Self {
            root,
            files,
            compilable_source_paths,
        })
    }
}

/// T0143：判断 sysroot 文件是否需要作为编译单元的一部分（而非仅签名索引）。
/// 当前规则：
/// - `string.scoop`、`print.scoop`、`scalar_string_bridge.scoop` 与 `task.scoop` 一直是 support sources；
/// - overlay 的 `core.scoop` 若声明了 bodied `@Intrinsic struct/class`，也要进入完整编译管线。
fn is_compilable_sysroot_file(path: &Path, source: &SourceFile, ast: &crate::ast::File) -> bool {
    is_always_compilable_sysroot_file(path) || has_bodied_intrinsic_nominal_method(source, ast)
}

fn is_always_compilable_sysroot_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        name == "string.scoop"
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
        assert_eq!(sysroot.files.len(), 1);
        assert_eq!(sysroot.files[0].path, unsafe_file);

        let mut support_sources = Vec::new();
        collect_compilable_sysroot_files(&base_root, Some(&overlay_root), &mut support_sources)
            .unwrap();
        assert_eq!(support_sources, vec![overlay_core]);
    }
}
