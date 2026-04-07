//! `scoop new` 子命令：创建最小 CONE application 项目骨架（TODO T1118）。
//!
//! 目标（v0）：
//! - 只生成 application（可执行）项目：`src/main.scoop` + `Cone.toml` + `README.md`；
//! - `project_name` 同时作为目录名、`[cone].name` 与 `package` 标识符；
//! - 若目标目录已存在则拒绝（不覆盖、不合并）。

use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _};
use thiserror::Error;

const SRC_DIR_NAME: &str = "src";
const MAIN_FILE_NAME: &str = "main.scoop";

#[derive(Debug, Error, Diagnostic)]
pub enum NewProjectError {
    #[error(
        "项目名不合法：`{name}`（要求：以字母/下划线开头，仅包含字母/数字/下划线；例如 `hello_world`）"
    )]
    #[diagnostic(code(scoop::driver::new_invalid_project_name))]
    InvalidProjectName { name: String },

    #[error("目标目录已存在：{path}")]
    #[diagnostic(code(scoop::driver::new_target_dir_exists))]
    TargetDirExists { path: PathBuf },

    #[error("无法创建目录：{path}")]
    #[diagnostic(code(scoop::driver::new_create_dir_failed))]
    CreateDirFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("无法写入文件：{path}")]
    #[diagnostic(code(scoop::driver::new_write_file_failed))]
    WriteFileFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// 执行 `scoop new <project-name>`。
pub fn run(project_name: String) -> miette::Result<()> {
    let cwd = std::env::current_dir()
        .into_diagnostic()
        .wrap_err("无法获取当前工作目录")?;
    let created = create_cone_app_project(&cwd, &project_name)?;
    println!("创建：{}", created.display());
    Ok(())
}

/// 在 `base_dir` 下创建一个最小的 CONE application 项目。
///
/// 返回创建的项目目录路径。
pub fn create_cone_app_project(
    base_dir: &Path,
    project_name: &str,
) -> Result<PathBuf, NewProjectError> {
    let name = project_name.trim();
    if name.is_empty() || !is_valid_scoop_package_ident(name) {
        return Err(NewProjectError::InvalidProjectName {
            name: project_name.to_owned(),
        });
    }

    let project_dir = base_dir.join(name);
    match std::fs::create_dir(&project_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(NewProjectError::TargetDirExists { path: project_dir });
        }
        Err(e) => {
            return Err(NewProjectError::CreateDirFailed {
                path: project_dir,
                source: e,
            });
        }
    }

    let src_dir = project_dir.join(SRC_DIR_NAME);
    std::fs::create_dir(&src_dir).map_err(|e| NewProjectError::CreateDirFailed {
        path: src_dir.clone(),
        source: e,
    })?;

    write_file(project_dir.join("Cone.toml"), &cone_toml_template(name))?;
    write_file(src_dir.join(MAIN_FILE_NAME), &main_scoop_template(name))?;
    write_file(project_dir.join("README.md"), &readme_template(name))?;

    Ok(project_dir)
}

fn write_file(path: PathBuf, contents: &str) -> Result<(), NewProjectError> {
    std::fs::write(&path, contents)
        .map_err(|e| NewProjectError::WriteFileFailed { path, source: e })
}

fn cone_toml_template(name: &str) -> String {
    // 约定：v0 先固定版本号为 `0.1.0`（与当前仓库版本对齐）。
    format!(
        r#"[cone]
name = "{name}"
version = "0.1.0"

[dependencies]
scoop-core = "0.1.0"
"#
    )
}

fn main_scoop_template(name: &str) -> String {
    format!(
        r#"package {name}

public fun main() / Pure! {{
}}
"#
    )
}

fn readme_template(name: &str) -> String {
    format!(
        r#"# {name}

这是一个由 `scoop new` 生成的最小 CONE application 项目。

## 布局

- `Cone.toml`：cone manifest（`[cone].name`）
- `src/main.scoop`：入口源码（需要定义 `public fun main()`，并保持 `package` 与 `[cone].name` 一致）

## 构建与运行

如果你在 Scoop 仓库根目录下开发：

```bash
# 构建
cargo run -p scoop --features llvm -- build .

# 运行
cargo run -p scoop --features llvm -- run .
```

如果你已安装 `scoop`：

```bash
scoop build .
scoop run .
```

说明：

- `build/run` 需要 LLVM 后端（`--features llvm`），并确保 `clang` 与 `llvm-config`（LLVM 21.1）在 `PATH` 中。
"#
    )
}

fn is_valid_scoop_package_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    for c in chars {
        if !(c == '_' || c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use miette::Diagnostic as _;
    use tempfile::tempdir;

    #[test]
    fn new_creates_minimal_cone_app_project() {
        let dir = tempdir().unwrap();
        let project_dir = super::create_cone_app_project(dir.path(), "hello_world").unwrap();

        assert!(project_dir.is_dir(), "应创建项目目录");
        assert!(project_dir.join("Cone.toml").is_file(), "应生成 Cone.toml");
        assert!(project_dir.join("README.md").is_file(), "应生成 README.md");
        assert!(
            project_dir.join("src").join("main.scoop").is_file(),
            "应生成 src/main.scoop"
        );

        let cone_toml = std::fs::read_to_string(project_dir.join("Cone.toml")).unwrap();
        assert!(
            cone_toml.contains("name = \"hello_world\""),
            "Cone.toml 应包含项目名，实际：{cone_toml}"
        );

        let main_scoop =
            std::fs::read_to_string(project_dir.join("src").join("main.scoop")).unwrap();
        assert!(
            main_scoop.contains("package hello_world"),
            "main.scoop 应包含 package 声明，实际：{main_scoop}"
        );

        let readme = std::fs::read_to_string(project_dir.join("README.md")).unwrap();
        assert!(
            readme.contains("# hello_world"),
            "README.md 应包含标题，实际：{readme}"
        );
    }

    #[test]
    fn new_rejects_invalid_project_name_with_stable_error_code() {
        let dir = tempdir().unwrap();
        let err = super::create_cone_app_project(dir.path(), "hello-world").unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::driver::new_invalid_project_name",
            "应返回稳定错误码"
        );
    }
}
