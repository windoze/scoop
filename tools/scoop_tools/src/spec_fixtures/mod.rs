//! 从规范抽取 doctest fixtures。
//!
//! 当前约定：
//! - 仅抽取 fenced code block（```）内部包含 `// FIXTURE: <path>` 的代码块
//! - `<path>` 必须位于 `spec_doctest/` 下，且以 `.scoop` 结尾
//! - 生成目标为 `<fixtures_root>/<path>`

mod parse;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use miette::{miette, Context as _, IntoDiagnostic as _, Result};

pub const GENERATED_DIR: &str = "spec_doctest";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Sync,
    Check,
}

#[derive(Debug, Clone)]
pub struct GeneratedFixture {
    pub rel_path: PathBuf,
    pub content: String,
}

pub fn run(mode: Mode, spec_path: &Path, fixtures_root: &Path) -> Result<Vec<PathBuf>> {
    let spec_text = std::fs::read_to_string(spec_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取规范文件：{}", spec_path.display()))?;

    let fixtures = parse::extract(&spec_text)?;

    // 路径合法性与输出范围检查
    let mut by_path: BTreeMap<PathBuf, String> = BTreeMap::new();
    for f in fixtures {
        validate_fixture_path(&f.rel_path)?;
        if by_path.insert(f.rel_path.clone(), f.content).is_some() {
            return Err(miette!(
                "规范里存在重复的 `// FIXTURE:` 路径：{}",
                f.rel_path.display()
            ));
        }
    }

    let out_root = fixtures_root.join(GENERATED_DIR);
    match mode {
        Mode::Sync => sync(&out_root, &by_path)?,
        Mode::Check => check(&out_root, &by_path)?,
    }

    Ok(by_path.keys().cloned().collect())
}

fn validate_fixture_path(rel_path: &Path) -> Result<()> {
    if rel_path.as_os_str().is_empty() {
        return Err(miette!("`// FIXTURE:` 路径不能为空"));
    }
    if rel_path.is_absolute() {
        return Err(miette!(
            "`// FIXTURE:` 路径必须是相对路径：{}",
            rel_path.display()
        ));
    }
    for c in rel_path.components() {
        match c {
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(miette!(
                    "`// FIXTURE:` 路径不允许包含前缀/根目录/..：{}",
                    rel_path.display()
                ));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    if rel_path.extension().is_none_or(|ext| ext != "scoop") {
        return Err(miette!(
            "`// FIXTURE:` 路径必须以 .scoop 结尾：{}",
            rel_path.display()
        ));
    }
    if !rel_path.starts_with(GENERATED_DIR) {
        return Err(miette!(
            "`// FIXTURE:` 路径必须位于 `{GENERATED_DIR}/` 下：{}",
            rel_path.display()
        ));
    }
    Ok(())
}

fn sync(out_root: &Path, desired: &BTreeMap<PathBuf, String>) -> Result<()> {
    std::fs::create_dir_all(out_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法创建输出目录：{}", out_root.display()))?;

    // 写入/更新所有目标文件
    for (rel, content) in desired {
        let abs = out_root.parent().unwrap_or(out_root).join(rel);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)
                .into_diagnostic()
                .wrap_err_with(|| format!("无法创建目录：{}", parent.display()))?;
        }
        std::fs::write(&abs, content)
            .into_diagnostic()
            .wrap_err_with(|| format!("无法写入：{}", abs.display()))?;
    }

    // 删除多余文件（只清理 spec_doctest 目录树）
    let mut desired_abs: BTreeSet<PathBuf> = BTreeSet::new();
    for rel in desired.keys() {
        desired_abs.insert(out_root.parent().unwrap_or(out_root).join(rel));
    }

    remove_stale_scoop_files(out_root, &desired_abs)?;
    Ok(())
}

fn check(out_root: &Path, desired: &BTreeMap<PathBuf, String>) -> Result<()> {
    // 1) 检查每个目标文件内容一致
    for (rel, content) in desired {
        let abs = out_root.parent().unwrap_or(out_root).join(rel);
        let existing = std::fs::read_to_string(&abs)
            .into_diagnostic()
            .wrap_err_with(|| format!("缺少生成文件：{}（请运行 spec-fixtures sync）", abs.display()))?;
        if existing != *content {
            return Err(miette!(
                "生成文件与规范不一致：{}（请运行 spec-fixtures sync）",
                abs.display()
            ));
        }
    }

    // 2) 检查没有多余文件
    let mut desired_abs: BTreeSet<PathBuf> = BTreeSet::new();
    for rel in desired.keys() {
        desired_abs.insert(out_root.parent().unwrap_or(out_root).join(rel));
    }
    let extras = find_extra_scoop_files(out_root, &desired_abs)?;
    if !extras.is_empty() {
        let mut msg = String::from("spec_doctest 目录存在多余文件（请运行 spec-fixtures sync 清理）：\n");
        for p in extras {
            msg.push_str("  - ");
            msg.push_str(&p.display().to_string());
            msg.push('\n');
        }
        return Err(miette!(msg));
    }

    Ok(())
}

fn remove_stale_scoop_files(dir: &Path, keep: &BTreeSet<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;

        if ty.is_dir() {
            remove_stale_scoop_files(&path, keep)?;
            // 尝试清理空目录
            let _ = std::fs::remove_dir(&path);
            continue;
        }

        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") && !keep.contains(&path)
        {
            std::fs::remove_file(&path)
                .into_diagnostic()
                .wrap_err_with(|| format!("无法删除：{}", path.display()))?;
        }
    }
    Ok(())
}

fn find_extra_scoop_files(dir: &Path, keep: &BTreeSet<PathBuf>) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut extras = Vec::new();
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;

        if ty.is_dir() {
            extras.extend(find_extra_scoop_files(&path, keep)?);
            continue;
        }

        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") && !keep.contains(&path)
        {
            extras.push(path);
        }
    }
    extras.sort();
    Ok(extras)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn validate_fixture_path_rules() {
        assert!(validate_fixture_path(Path::new("spec_doctest/a.scoop")).is_ok());
        assert!(validate_fixture_path(Path::new("a.scoop")).is_err());
        assert!(validate_fixture_path(Path::new("../spec_doctest/a.scoop")).is_err());
        assert!(validate_fixture_path(Path::new("spec_doctest/a.txt")).is_err());
    }

    #[test]
    fn extract_blocks_from_markdown() {
        let md = r#"
Text

```scoop
// FIXTURE: spec_doctest/ok.scoop
// EXPECT: pass
fun main() {}
```

```kotlin
// not a fixture
fun main() {}
```
"#;
        let fixtures = parse::extract(md).unwrap();
        assert_eq!(fixtures.len(), 1);
        assert_eq!(fixtures[0].rel_path, PathBuf::from("spec_doctest/ok.scoop"));
        assert!(fixtures[0].content.contains("fun main() {}"));
    }
}

