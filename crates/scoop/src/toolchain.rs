//! 工具链封装（早期阶段仅覆盖最小链接）。
//!
//! 设计目标：
//! - driver 侧避免把“调用 clang/ld 的细节”散落在各个子命令里；
//! - 错误要结构化（miette 诊断码），便于 fixtures/CI 定位问题；
//! - 仅支持 host 平台的最小 happy path（T0806）。

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use miette::Diagnostic;
use thiserror::Error;

/// 链接阶段错误（T0806）。
#[derive(Debug, Error, Diagnostic)]
pub enum LinkError {
    #[error("找不到 clang（需要安装 clang 并确保在 PATH 中）")]
    #[diagnostic(code(scoop::toolchain::clang_not_found))]
    ClangNotFound,

    #[error("运行 clang 失败：{source}")]
    #[diagnostic(code(scoop::toolchain::clang_spawn_failed))]
    ClangSpawnFailed {
        #[source]
        source: std::io::Error,
    },

    #[error("clang 链接失败（退出码：{status}）\n命令：{command}\nstdout：{stdout}\nstderr：{stderr}")]
    #[diagnostic(code(scoop::toolchain::clang_link_failed))]
    ClangLinkFailed {
        status: ExitStatus,
        command: String,
        stdout: String,
        stderr: String,
    },

    #[error("找不到 runtime C 源文件：{path}")]
    #[diagnostic(code(scoop::toolchain::runtime_source_missing))]
    RuntimeSourceMissing { path: PathBuf },
}

/// 通过 clang 将单个 object 文件与 Scoop runtime 链接为可执行文件。
///
/// 当前阶段实现策略：
/// - 直接把 `runtime/c/scoop_runtime.c` 作为输入交给 clang，让其编译并参与链接；
/// - 避免依赖 Cargo build 输出路径（后续若要复用 `scoop_runtime` crate 产物再重构）。
pub fn link_obj_with_runtime(obj: &Path, output: &Path) -> Result<(), LinkError> {
    let runtime_c = runtime_c_path();
    if !runtime_c.is_file() {
        return Err(LinkError::RuntimeSourceMissing { path: runtime_c });
    }

    let mut cmd = Command::new("clang");
    cmd.arg(obj).arg(&runtime_c).arg("-o").arg(output);

    let output_res = cmd.output();
    let output_res = match output_res {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(LinkError::ClangNotFound),
        Err(e) => return Err(LinkError::ClangSpawnFailed { source: e }),
    };

    if !output_res.status.success() {
        return Err(LinkError::ClangLinkFailed {
            status: output_res.status,
            command: format_command_for_debug(&cmd),
            stdout: String::from_utf8_lossy(&output_res.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output_res.stderr).to_string(),
        });
    }

    Ok(())
}

fn runtime_c_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime/c/scoop_runtime.c")
}

fn format_command_for_debug(cmd: &Command) -> String {
    let program = cmd.get_program().to_string_lossy();
    let args = cmd
        .get_args()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>();
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn clang_can_link_object_with_runtime_and_run() {
        let dir = tempfile::tempdir().unwrap();
        let main_c = dir.path().join("main.c");
        let main_o = dir.path().join("main.o");

        std::fs::write(&main_c, "int main(void) { return 0; }\n").unwrap();

        let status = Command::new("clang")
            .arg("-c")
            .arg(&main_c)
            .arg("-o")
            .arg(&main_o)
            .status()
            .unwrap();
        assert!(status.success(), "clang -c 应成功");

        let out = dir.path().join(format!("a{}", std::env::consts::EXE_EXTENSION));
        link_obj_with_runtime(&main_o, &out).unwrap();
        assert!(out.is_file(), "应生成可执行文件");

        let status = Command::new(&out).status().unwrap();
        assert!(status.success(), "可执行文件应返回 0");
    }

    #[test]
    fn clang_can_link_object_with_runtime_and_println() {
        let dir = tempfile::tempdir().unwrap();
        let main_c = dir.path().join("main.c");
        let main_o = dir.path().join("main.o");

        // 直接声明 runtime ABI（避免依赖未来才会引入的头文件安装/导出流程）。
        //
        // 约定：String 为一个指向 runtime 对象的指针；当前 early stage 先把它实现为
        // `{ len: u64, data: *const u8 }` 结构体的地址（见 `runtime/c/scoop_runtime.c`）。
        std::fs::write(
            &main_c,
            r#"
#include <stdint.h>

typedef struct ScoopString {
  uint64_t len;
  const uint8_t *data;
} ScoopString;

void scoop_println(const ScoopString *value);

int main(void) {
  const char *msg = "hi";
  ScoopString s = {2, (const uint8_t *)msg};
  scoop_println(&s);
  return 0;
}
"#,
        )
        .unwrap();

        let status = Command::new("clang")
            .arg("-c")
            .arg(&main_c)
            .arg("-o")
            .arg(&main_o)
            .status()
            .unwrap();
        assert!(status.success(), "clang -c 应成功");

        let out = dir.path().join(format!("a{}", std::env::consts::EXE_EXTENSION));
        link_obj_with_runtime(&main_o, &out).unwrap();
        assert!(out.is_file(), "应生成可执行文件");

        let output = Command::new(&out).output().unwrap();
        assert!(output.status.success(), "可执行文件应返回 0");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "hi\n",
            "stdout 应匹配"
        );
    }
}
