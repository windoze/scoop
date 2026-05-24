//! LLVM toolchain baseline check for the standalone LLVM backend crate.
//!
//! When the `llvm` feature is enabled, this build script checks
//! `llvm-config --version` before compiling the inkwell/llvm-sys backend.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

const EXPECTED_LLVM_MAJOR: u32 = 21;
const EXPECTED_LLVM_MINOR: u32 = 1;

fn main() {
    println!("cargo:rerun-if-env-changed=LLVM_CONFIG_PATH");
    println!("cargo:rerun-if-env-changed=LLVM_SYS_211_PREFIX");
    println!("cargo:rerun-if-env-changed=PATH");

    if std::env::var("CARGO_FEATURE_LLVM").is_err() {
        return;
    }

    let llvm_config = resolve_llvm_config();
    let output = Command::new(&llvm_config)
        .arg("--version")
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "启用 LLVM 后端需要 `llvm-config`（LLVM {EXPECTED_LLVM_MAJOR}.{EXPECTED_LLVM_MINOR}）。\n\
找不到/无法执行 llvm-config：{err}\n\
\n\
修复方式：\n\
- macOS（Homebrew）：`brew install llvm@21`，并执行：\n\
  `export PATH=\"/opt/homebrew/opt/llvm@21/bin:$PATH\"`\n\
- 或显式指定：\n\
  - `LLVM_CONFIG_PATH=/path/to/llvm-config`\n\
  - `LLVM_SYS_211_PREFIX=/path/to/llvm@21/prefix`（会使用 `$LLVM_SYS_211_PREFIX/bin/llvm-config`）\n\
"
            );
        });

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "`{}` 执行失败（用于检测 LLVM 版本）。\n\
stdout:\n\
{stdout}\n\
stderr:\n\
{stderr}\n\
",
            display_cmd(&llvm_config)
        );
    }

    let version = String::from_utf8_lossy(&output.stdout);
    let Some((major, minor)) = parse_major_minor(&version) else {
        panic!(
            "无法解析 LLVM 版本：`{}` 输出为：`{}`",
            display_cmd(&llvm_config),
            version.trim()
        );
    };

    if major != EXPECTED_LLVM_MAJOR || minor != EXPECTED_LLVM_MINOR {
        panic!(
            "LLVM 版本不匹配：需要 LLVM {EXPECTED_LLVM_MAJOR}.{EXPECTED_LLVM_MINOR}（inkwell `llvm21-1`），但 `{}` 返回 `{}`。\n\
\n\
修复方式：\n\
- macOS（Homebrew）：\n\
  `brew install llvm@21`\n\
  `export PATH=\"/opt/homebrew/opt/llvm@21/bin:$PATH\"`\n\
- 或改用正确的 llvm-config：\n\
  - `LLVM_CONFIG_PATH=/path/to/llvm-config`\n\
  - `LLVM_SYS_211_PREFIX=/path/to/llvm@21/prefix`\n\
",
            display_cmd(&llvm_config),
            version.trim()
        );
    }
}

fn resolve_llvm_config() -> OsString {
    if let Ok(path) = std::env::var("LLVM_CONFIG_PATH") {
        return path.into();
    }

    if let Ok(prefix) = std::env::var("LLVM_SYS_211_PREFIX") {
        let candidate = Path::new(&prefix).join("bin").join("llvm-config");
        if candidate.exists() {
            return candidate.into_os_string();
        }
    }

    "llvm-config".into()
}

fn parse_major_minor(version: &str) -> Option<(u32, u32)> {
    let version = version.trim();
    let start = version.find(|c: char| c.is_ascii_digit())?;
    let rest = &version[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    let numeric = &rest[..end];

    let mut parts = numeric.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    Some((major, minor))
}

fn display_cmd(cmd: &OsString) -> String {
    cmd.to_string_lossy().to_string()
}
