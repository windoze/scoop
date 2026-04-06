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

/// 最终链接阶段的可选配置（T1114）。
///
/// 约定：
/// - `linker` 仅表示“要运行的可执行文件路径/名称”，不包含额外参数；额外参数放到 `link_flags`；
/// - `link_flags` 逐项作为独立 argv 追加到最终链接命令中（不做拆分/转义重写），以保持行为可预测。
#[derive(Debug, Clone, Copy)]
pub struct LinkOptions<'a> {
    /// 指定链接器/驱动程序（例如 `clang`/`clang++`）。
    ///
    /// 当为 `None` 时使用默认 `clang`。
    pub linker: Option<&'a str>,
    /// 追加到最终链接命令的额外参数（保持顺序）。
    pub link_flags: &'a [String],
}

impl<'a> Default for LinkOptions<'a> {
    fn default() -> Self {
        Self {
            linker: None,
            link_flags: &[],
        }
    }
}

/// 链接阶段错误（T0806）。
#[derive(Debug, Error, Diagnostic)]
pub enum LinkError {
    #[error(
        "找不到链接器 `{linker}`（需要安装并确保在 PATH 中，或在 Cone.toml 中配置 `native-build.linker`）"
    )]
    #[diagnostic(code(scoop::toolchain::linker_not_found))]
    LinkerNotFound { linker: String },

    #[error("运行链接器 `{linker}` 失败：{source}")]
    #[diagnostic(code(scoop::toolchain::linker_spawn_failed))]
    LinkerSpawnFailed {
        linker: String,
        #[source]
        source: std::io::Error,
    },

    #[error("链接失败（退出码：{status}）\n命令：{command}\nstdout：{stdout}\nstderr：{stderr}")]
    #[diagnostic(code(scoop::toolchain::link_failed))]
    LinkFailed {
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
/// - 直接把 `runtime/c/*.c` 作为输入交给 clang，让其编译并参与链接；
/// - 避免依赖 Cargo build 输出路径（后续若要复用 `scoop_runtime` crate 产物再重构）。
pub fn link_obj_with_runtime(
    obj: &Path,
    output: &Path,
    libs: &[String],
    options: LinkOptions<'_>,
) -> Result<(), LinkError> {
    let mut cmd = link_command_with_runtime(obj, output, libs, options)?;
    let linker_for_error = cmd.get_program().to_string_lossy().to_string();

    let output_res = cmd.output();
    let output_res = match output_res {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(LinkError::LinkerNotFound {
                linker: linker_for_error,
            });
        }
        Err(e) => {
            return Err(LinkError::LinkerSpawnFailed {
                linker: linker_for_error,
                source: e,
            });
        }
    };

    if !output_res.status.success() {
        return Err(LinkError::LinkFailed {
            status: output_res.status,
            command: format_command_for_debug(&cmd),
            stdout: String::from_utf8_lossy(&output_res.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output_res.stderr).to_string(),
        });
    }

    Ok(())
}

fn link_command_with_runtime(
    obj: &Path,
    output: &Path,
    libs: &[String],
    options: LinkOptions<'_>,
) -> Result<Command, LinkError> {
    let runtime_sources = runtime_c_sources()?;

    let linker = options.linker.unwrap_or("clang");
    let mut cmd = Command::new(linker);
    cmd.arg("-DSCOOP_GC_BACKEND=3");
    cmd.arg(obj);
    for src in &runtime_sources {
        cmd.arg(src);
    }
    for lib in libs {
        if lib.trim().is_empty() {
            continue;
        }
        cmd.arg(format!("-l{}", lib.trim()));
    }
    for flag in options.link_flags {
        if flag.trim().is_empty() {
            continue;
        }
        cmd.arg(flag);
    }
    cmd.arg("-o").arg(output);
    Ok(cmd)
}

fn runtime_c_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime/c")
}

fn runtime_c_sources() -> Result<Vec<PathBuf>, LinkError> {
    let dir = runtime_c_dir();
    let runtime_main = dir.join("scoop_runtime.c");
    if !runtime_main.is_file() {
        return Err(LinkError::RuntimeSourceMissing { path: runtime_main });
    }

    let mut extra = Vec::<PathBuf>::new();
    let entries = std::fs::read_dir(&dir)
        .map_err(|_| LinkError::RuntimeSourceMissing { path: dir.clone() })?;

    for entry in entries {
        let entry = entry.map_err(|_| LinkError::RuntimeSourceMissing { path: dir.clone() })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path == runtime_main {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("c") {
            continue;
        }
        extra.push(path);
    }

    // 稳定顺序，避免 debug command 字符串抖动。
    extra.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));

    let mut all = Vec::with_capacity(1 + extra.len());
    all.push(runtime_main);
    all.extend(extra);
    Ok(all)
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

        let out = dir
            .path()
            .join(format!("a{}", std::env::consts::EXE_EXTENSION));
        link_obj_with_runtime(&main_o, &out, &[], LinkOptions::default()).unwrap();
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
        // 约定：String 为一个指向 runtime 对象的指针；当前 `ScoopString` 为 GC-managed 对象：
        // `{ hdr: ScoopGcObjectHeader, len: u64, data: *const u8 }`（见 `runtime/c/scoop_runtime.c`）。
        std::fs::write(
            &main_c,
            r#"
#include <stdint.h>

typedef struct ScoopGcObjectHeader {
  void *next;
  void *type_desc;
  uint64_t size_bytes;
  uint32_t flags;
  uint32_t mark;
} ScoopGcObjectHeader;

typedef struct ScoopString {
  ScoopGcObjectHeader hdr;
  uint64_t len;
  const uint8_t *data;
} ScoopString;

void *scoop_alloc(uint64_t size);
void scoop_println(const ScoopString *value);

int main(void) {
  const char *msg = "hi";
  ScoopString *s = (ScoopString *)scoop_alloc((uint64_t)sizeof(ScoopString));
  s->len = 2;
  s->data = (const uint8_t *)msg;
  scoop_println(s);
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

        let out = dir
            .path()
            .join(format!("a{}", std::env::consts::EXE_EXTENSION));
        link_obj_with_runtime(&main_o, &out, &[], LinkOptions::default()).unwrap();
        assert!(out.is_file(), "应生成可执行文件");

        let output = Command::new(&out).output().unwrap();
        assert!(output.status.success(), "可执行文件应返回 0");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "hi\n",
            "stdout 应匹配"
        );
    }

    #[test]
    fn clang_link_command_includes_extern_libs() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("main.scoop");
        std::fs::write(
            &src,
            r#"
package fixtures.t1020
import scoop.core.*

@Extern(lib = "m")
fun cos(x: Int): Int
"#,
        )
        .unwrap();

        let source = scoopc::source::SourceFile::load(&src).unwrap();
        let session = scoopc::session::Session::new().unwrap();
        let lowered = scoopc::hir::lower_for_dump(&session, &source).unwrap();

        assert_eq!(lowered.extern_libs, vec!["m".to_string()]);

        let obj = dir.path().join("main.o");
        let out = dir.path().join("a.out");
        let cmd =
            link_command_with_runtime(&obj, &out, &lowered.extern_libs, LinkOptions::default())
                .unwrap();
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(
            args.iter().any(|a| a == "-lm"),
            "clang args 应包含 -lm，实际：{args:?}"
        );
    }

    #[test]
    fn link_command_includes_link_flags_in_stable_order() {
        let dir = tempfile::tempdir().unwrap();
        let obj = dir.path().join("main.o");
        let out = dir.path().join("a.out");

        let libs = vec!["m".to_string()];
        let link_flags = vec![
            "-Wl,--gc-sections".to_string(),
            "-Wl,-dead_strip".to_string(),
        ];

        let options = LinkOptions {
            linker: Some("my-linker"),
            link_flags: &link_flags,
        };
        let cmd1 = link_command_with_runtime(&obj, &out, &libs, options).unwrap();
        let args1 = cmd1
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            cmd1.get_program().to_string_lossy(),
            "my-linker",
            "应使用自定义 linker 程序"
        );

        let idx_lib = args1
            .iter()
            .position(|a| a == "-lm")
            .expect("应包含 extern libs -lm");
        let idx_flag1 = args1
            .iter()
            .position(|a| a == "-Wl,--gc-sections")
            .expect("应包含 link flag 1");
        let idx_flag2 = args1
            .iter()
            .position(|a| a == "-Wl,-dead_strip")
            .expect("应包含 link flag 2");
        let idx_o = args1.iter().position(|a| a == "-o").expect("应包含 -o");

        assert!(
            idx_lib < idx_flag1 && idx_flag1 < idx_flag2 && idx_flag2 < idx_o,
            "args 顺序应为：extern libs -> link-flags -> -o，实际：{args1:?}"
        );

        // 同一输入下命令构造应稳定（避免 debug command 抖动）。
        let cmd2 = link_command_with_runtime(&obj, &out, &libs, options).unwrap();
        let args2 = cmd2
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(args1, args2, "args 列表应稳定");
    }
}
