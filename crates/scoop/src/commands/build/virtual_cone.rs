//! Facade-owned single-file wrapping into a standard source cone.
//!
//! `scoopc build-single-cone` only accepts a cone root. For `scoop build a.scoop`
//! the facade materializes a synthetic bin cone under `build/<profile>/virtual/`
//! and then routes it through the same artifact pipeline as explicit cone builds.

use std::path::{Path, PathBuf};

use miette::{Context as _, IntoDiagnostic as _, Result};

use super::BuildProfile;

#[allow(dead_code)]
pub(crate) fn materialize_single_file(input: &Path, profile: BuildProfile) -> Result<PathBuf> {
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let name = virtual_cone_name(input);
    let root = parent
        .join("build")
        .join(profile.as_str())
        .join("virtual")
        .join(format!("{name}@0.0.0"));
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法创建 virtual cone src 目录：{}", src_dir.display()))?;
    std::fs::copy(input, src_dir.join("main.scoop"))
        .into_diagnostic()
        .wrap_err_with(|| format!("无法复制单文件输入到 virtual cone：{}", input.display()))?;
    std::fs::write(
        root.join(scoop_project_model::CONE_TOML_FILE_NAME),
        format!("[cone]\nname = \"{name}\"\nversion = \"0.0.0\"\nkind = \"bin\"\n"),
    )
    .into_diagnostic()
    .wrap_err_with(|| format!("无法写入 virtual Cone.toml：{}", root.display()))?;
    std::fs::write(
        root.join(".scoop-virtual-cone"),
        format!("{}\n", input.display()),
    )
    .into_diagnostic()
    .wrap_err_with(|| format!("无法写入 virtual cone marker：{}", root.display()))?;
    Ok(root)
}

fn virtual_cone_name(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("virtual-cone");
    let mut out = String::with_capacity(stem.len());
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "virtual-cone".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_cone_name_is_filesystem_safe() {
        assert_eq!(
            virtual_cone_name(Path::new("hello world!.scoop")),
            "hello_world_"
        );
    }
}
