//! `scoop package` 子命令（TODO T1104）。
//!
//! 当前阶段：
//! - 只实现“把 cone 源码包打成 `.cone` 归档”（写包）；
//! - 归档格式使用 tar，便于快速落地与可回归测试；
//! - `.cone` 的读取与下游 typecheck 注入留给后续任务（TODO T1105+）。

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::SessionOptions;

/// 执行 `scoop package <cone-root> [-o <out.cone>]`。
pub fn run(input: PathBuf, output: Option<PathBuf>, session_options: SessionOptions) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入路径")?;

    let pkg = scoopc::cone::load_cone_source_package(&input)?;

    let output = output.unwrap_or_else(|| default_output_path(&pkg, &input));
    ensure_output_parent_dir(&output)?;

    let session = scoopc::session::Session::with_options(session_options)?;
    scoopc::cone::write_cone_archive_v0(&session, &pkg, &output)?;

    // v0：打包完成后列出归档内容，方便人工 sanity check。
    println!("写出：{}", output.display());
    let mut entries = scoopc::cone::list_cone_archive_entries(&output)?;
    entries.sort();
    for e in entries {
        println!("- {e}");
    }

    Ok(())
}

fn default_output_path(pkg: &scoopc::cone::ConeSourcePackage, fallback_dir: &Path) -> PathBuf {
    // 默认将归档写到输入目录旁边，文件名使用 `<name>-<version>.cone`（避免同目录多包冲突）。
    let file_name = format!(
        "{}-{}.cone",
        pkg.manifest.cone.name, pkg.manifest.cone.version
    );
    fallback_dir.join(file_name)
}

fn ensure_output_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)
        .into_diagnostic()
        .wrap_err("无法创建输出目录")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use tempfile::tempdir;

    #[test]
    fn package_writes_cone_archive_and_it_contains_required_entries() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("pkg");
        let src = pkg.join("src");
        std::fs::create_dir_all(&src).unwrap();

        std::fs::write(
            pkg.join("Cone.toml"),
            r#"
[cone]
name = "fixture-pkg"
version = "0.0.0"
"#,
        )
        .unwrap();

        std::fs::write(
            src.join("main.scoop"),
            r#"
package fixture
public fun main() { }
"#,
        )
        .unwrap();

        std::fs::write(
            src.join("lib.scoop"),
            r#"
package fixture
public fun util() { }
"#,
        )
        .unwrap();

        let out = dir.path().join("out").join("fixture.cone");
        super::run(pkg, Some(out.clone()), super::SessionOptions::default()).unwrap();

        assert!(out.is_file(), "应写出 .cone 文件");

        let entries = scoopc::cone::list_cone_archive_entries(&out).unwrap();
        let entries: HashSet<String> = entries.into_iter().collect();
        assert!(entries.contains("Cone.toml"));
        assert!(entries.contains(scoopc::cone::CONE_API_SCOOPIR_FILE_NAME));
        assert!(entries.contains(scoopc::cone::CONE_SOURCES_SHA256_FILE_NAME));
    }

    #[test]
    fn package_writes_api_scoopir_and_sources_hash_are_readable() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("pkg");
        let src = pkg.join("src");
        std::fs::create_dir_all(&src).unwrap();

        std::fs::write(
            pkg.join("Cone.toml"),
            r#"
[cone]
name = "fixture-pkg"
version = "0.0.0"
"#,
        )
        .unwrap();

        std::fs::write(
            src.join("main.scoop"),
            r#"
package fixture
public fun main() { }
"#,
        )
        .unwrap();

        std::fs::write(
            src.join("lib.scoop"),
            r#"
package fixture
public fun util() { }
"#,
        )
        .unwrap();

        let out = dir.path().join("fixture.cone");
        super::run(pkg, Some(out.clone()), super::SessionOptions::default()).unwrap();

        let api =
            scoopc::cone::read_cone_archive_entry(&out, scoopc::cone::CONE_API_SCOOPIR_FILE_NAME)
                .unwrap();
        let api: scoopc::cone::scoopir::ScoopIrFile = serde_json::from_slice(&api).unwrap();
        assert_eq!(api.schema.name, scoopc::cone::scoopir::SCOOPIR_SCHEMA_NAME);
        assert_eq!(
            api.schema.version,
            scoopc::cone::scoopir::SCOOPIR_SCHEMA_VERSION
        );

        let fun_fqns = api
            .funs
            .iter()
            .map(|f| f.fqn.as_str())
            .collect::<HashSet<_>>();
        assert!(fun_fqns.contains("fixture.main"));
        assert!(fun_fqns.contains("fixture.util"));

        let sources = scoopc::cone::read_cone_archive_entry(
            &out,
            scoopc::cone::CONE_SOURCES_SHA256_FILE_NAME,
        )
        .unwrap();
        let text = String::from_utf8(sources).unwrap();
        assert!(text.contains("src/main.scoop"));
        assert!(text.contains("src/lib.scoop"));
    }
}
