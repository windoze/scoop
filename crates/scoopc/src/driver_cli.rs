//! `scoopc` CLI 参数解析。
//!
//! 该模块只负责把原始参数解析成稳定结构，避免把 session 配置散落在二进制入口里。

use std::path::PathBuf;

use miette::Result;

use crate::session::SessionOptions;

pub const USAGE: &str = "\
用法：
  scoopc [--emit-llvm] <input.scoop> [-o <out.ll>]
  scoopc --obj <input.scoop> [-o <out.o>]
  scoopc build-single-cone --cone-root <dir> --out <dir> \\
      --inputs-fingerprint <hex> [--upstream-artifact <dir> ...] [--cone-id <key>]

说明：
  - 该二进制需要启用 `scoopc` 的 `llvm` feature（需要 LLVM 21.1 + `llvm-config`）。
  - 裸 `<input.scoop>` 按 single-source virtual cone 处理，不会根据相邻 `Cone.toml`
    自动恢复 explicit cone/project context。
  - `--obj` 为 object 输出模式；省略时默认输出 LLVM IR。
  - `build-single-cone` 由 scoop 的 cone DAG scheduler 通过子进程派发，将 cone-being
    -compiled 当作 graph consumer 跑完整 frontend 并写入 per-cone artifact。
";

/// scoopc CLI 顶层模式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerCli {
    /// 默认 single-source virtual cone 模式。
    Legacy(LegacyCompilerCli),
    /// `scoopc build-single-cone ...`：scoop driver 派发的子进程入口。
    BuildSingleCone(BuildSingleConeCli),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyCompilerCli {
    pub emit_mode: EmitMode,
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub session_options: SessionOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitMode {
    LlvmIr,
    Object,
}

/// `scoopc build-single-cone` 子命令的参数。
///
/// 该子命令仅作为 scoop driver -> scoopc 子进程的稳定接口；裸调用应优先经由 scoop
/// driver。CLI 形态与 [`crate::single_cone::run_single_cone_artifact_compile`] 字段一一对应。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSingleConeCli {
    /// cone-being-compiled 的根目录（含 `Cone.toml`）。
    pub cone_root: PathBuf,
    /// per-cone artifact 输出目录（写 `manifest.json` / `*.bin` / `inputs.fingerprint` ...）。
    pub output_dir: PathBuf,
    /// 父进程为本 cone 计算的 inputs fingerprint，hex 编码。
    pub inputs_fingerprint: Vec<u8>,
    /// 已构建好的上游 cone artifact 目录列表（顺序无关；按 manifest 中 StableConeKey 匹配）。
    pub upstream_artifact_dirs: Vec<PathBuf>,
    /// 期望的 cone stable key（"name@version"）。可选，仅作为父子进程一致性校验。
    pub expected_cone_id: Option<String>,
    /// 子进程内的 session 选项（profile / sysroot overlay / extra sysroot deps 等）。
    pub session_options: SessionOptions,
}

pub fn parse_args<I, S>(args: I) -> Result<Option<CompilerCli>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args_iter = args.into_iter().map(Into::into);
    let first = args_iter.next();
    match first.as_deref() {
        Some("-h") | Some("--help") => return Ok(None),
        Some("build-single-cone") => {
            return parse_build_single_cone(args_iter)
                .map(|cli| Some(CompilerCli::BuildSingleCone(cli)));
        }
        _ => {}
    }

    let chained = first.into_iter().chain(args_iter);
    parse_legacy(chained).map(|maybe| maybe.map(CompilerCli::Legacy))
}

fn parse_legacy<I>(args: I) -> Result<Option<LegacyCompilerCli>>
where
    I: IntoIterator<Item = String>,
{
    let mut emit_llvm = false;
    let mut emit_obj = false;
    let mut output: Option<PathBuf> = None;
    let mut input: Option<PathBuf> = None;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--emit-llvm" => emit_llvm = true,
            "--emit-obj" | "--obj" => emit_obj = true,
            "-o" | "--output" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("参数 `{arg}` 需要一个输出路径\n\n{USAGE}"));
                };
                output = Some(PathBuf::from(value));
            }
            _ if arg.starts_with('-') => {
                return Err(miette::miette!("未知参数：{arg}\n\n{USAGE}"));
            }
            _ => {
                if input.is_some() {
                    return Err(miette::miette!("一次只支持一个输入文件\n\n{USAGE}"));
                }
                input = Some(PathBuf::from(arg));
            }
        }
    }

    let emit_mode = match (emit_llvm, emit_obj) {
        (true, false) => EmitMode::LlvmIr,
        (false, true) => EmitMode::Object,
        (false, false) => EmitMode::LlvmIr,
        (true, true) => {
            return Err(miette::miette!(
                "`--emit-llvm` 与 `--obj`/`--emit-obj` 不能同时使用\n\n{USAGE}"
            ));
        }
    };

    let input = input.ok_or_else(|| miette::miette!("缺少输入文件\n\n{USAGE}"))?;

    Ok(Some(LegacyCompilerCli {
        emit_mode,
        input,
        output,
        session_options: SessionOptions::new(),
    }))
}

fn parse_build_single_cone<I>(args: I) -> Result<BuildSingleConeCli>
where
    I: IntoIterator<Item = String>,
{
    let mut cone_root: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut inputs_fingerprint_hex: Option<String> = None;
    let mut upstream_artifact_dirs: Vec<PathBuf> = Vec::new();
    let mut expected_cone_id: Option<String> = None;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                return Err(miette::miette!("{USAGE}"));
            }
            "--cone-root" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("`--cone-root` 需要一个目录路径\n\n{USAGE}"));
                };
                cone_root = Some(PathBuf::from(value));
            }
            "--out" | "--output-dir" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!("`{arg}` 需要一个目录路径\n\n{USAGE}"));
                };
                output_dir = Some(PathBuf::from(value));
            }
            "--inputs-fingerprint" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--inputs-fingerprint` 需要一个 hex 字符串\n\n{USAGE}"
                    ));
                };
                inputs_fingerprint_hex = Some(value);
            }
            "--upstream-artifact" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--upstream-artifact` 需要一个目录路径\n\n{USAGE}"
                    ));
                };
                upstream_artifact_dirs.push(PathBuf::from(value));
            }
            "--cone-id" => {
                let Some(value) = args.next() else {
                    return Err(miette::miette!(
                        "`--cone-id` 需要一个 cone stable-key 字符串\n\n{USAGE}"
                    ));
                };
                expected_cone_id = Some(value);
            }
            _ => {
                return Err(miette::miette!(
                    "build-single-cone 不接受参数 `{arg}`\n\n{USAGE}"
                ));
            }
        }
    }

    let cone_root = cone_root
        .ok_or_else(|| miette::miette!("`build-single-cone` 缺少 `--cone-root`\n\n{USAGE}"))?;
    let output_dir =
        output_dir.ok_or_else(|| miette::miette!("`build-single-cone` 缺少 `--out`\n\n{USAGE}"))?;
    let hex_value = inputs_fingerprint_hex.ok_or_else(|| {
        miette::miette!("`build-single-cone` 缺少 `--inputs-fingerprint`\n\n{USAGE}")
    })?;
    let inputs_fingerprint = decode_hex(&hex_value).ok_or_else(|| {
        miette::miette!("`--inputs-fingerprint` 必须是偶数长度的 hex 字符串：{hex_value}")
    })?;

    Ok(BuildSingleConeCli {
        cone_root,
        output_dir,
        inputs_fingerprint,
        upstream_artifact_dirs,
        expected_cone_id,
        session_options: SessionOptions::new(),
    })
}

fn decode_hex(input: &str) -> Option<Vec<u8>> {
    if !input.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let hi = decode_nibble(chunk[0])?;
        let lo = decode_nibble(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn decode_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(10 + (byte - b'a')),
        b'A'..=b'F' => Some(10 + (byte - b'A')),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn legacy(cli: CompilerCli) -> LegacyCompilerCli {
        match cli {
            CompilerCli::Legacy(c) => c,
            other => panic!("expected Legacy, got {other:?}"),
        }
    }

    fn build_single_cone(cli: CompilerCli) -> BuildSingleConeCli {
        match cli {
            CompilerCli::BuildSingleCone(c) => c,
            other => panic!("expected BuildSingleCone, got {other:?}"),
        }
    }

    #[test]
    fn bare_file_defaults_to_virtual_cone_llvm_ir_cli() {
        let cli = legacy(parse_args(["input.scoop"]).unwrap().unwrap());

        assert_eq!(cli.emit_mode, EmitMode::LlvmIr);
        assert_eq!(cli.input, PathBuf::from("input.scoop"));
        assert_eq!(cli.session_options, crate::session::SessionOptions::new());
    }

    #[test]
    fn obj_flag_selects_object_output_mode() {
        let cli = legacy(parse_args(["--obj", "input.scoop"]).unwrap().unwrap());

        assert_eq!(cli.emit_mode, EmitMode::Object);
        assert_eq!(cli.input, PathBuf::from("input.scoop"));
    }

    #[test]
    fn effect_pipeline_selector_removed_for_scoopc_cli() {
        let err = parse_args(["--effect-pipeline", "legacy", "--obj", "input.scoop"]).unwrap_err();

        assert!(err.to_string().contains("未知参数：--effect-pipeline"));
    }

    #[test]
    fn parse_args_rejects_removed_effect_pipeline_selector_with_any_value() {
        let err =
            parse_args(["--effect-pipeline", "future", "--emit-llvm", "input.scoop"]).unwrap_err();

        let message = err.to_string();
        assert!(message.contains("未知参数：--effect-pipeline"));
    }

    #[test]
    fn llvm_and_object_modes_conflict() {
        let err = parse_args(["--emit-llvm", "--obj", "input.scoop"]).unwrap_err();

        assert!(err.to_string().contains("不能同时使用"));
    }

    #[test]
    fn build_single_cone_minimal_cli_parses() {
        let cli = build_single_cone(
            parse_args([
                "build-single-cone",
                "--cone-root",
                "/tmp/foo",
                "--out",
                "/tmp/bar",
                "--inputs-fingerprint",
                "abcd",
            ])
            .unwrap()
            .unwrap(),
        );

        assert_eq!(cli.cone_root, PathBuf::from("/tmp/foo"));
        assert_eq!(cli.output_dir, PathBuf::from("/tmp/bar"));
        assert_eq!(cli.inputs_fingerprint, vec![0xab, 0xcd]);
        assert!(cli.upstream_artifact_dirs.is_empty());
        assert_eq!(cli.expected_cone_id, None);
    }

    #[test]
    fn build_single_cone_collects_repeated_upstream_artifacts() {
        let cli = build_single_cone(
            parse_args([
                "build-single-cone",
                "--cone-root",
                "/tmp/foo",
                "--out",
                "/tmp/bar",
                "--inputs-fingerprint",
                "00",
                "--upstream-artifact",
                "/tmp/up1",
                "--upstream-artifact",
                "/tmp/up2",
                "--cone-id",
                "demo@0.0.0",
            ])
            .unwrap()
            .unwrap(),
        );

        assert_eq!(
            cli.upstream_artifact_dirs,
            vec![PathBuf::from("/tmp/up1"), PathBuf::from("/tmp/up2")]
        );
        assert_eq!(cli.expected_cone_id.as_deref(), Some("demo@0.0.0"));
    }

    #[test]
    fn build_single_cone_rejects_odd_length_hex() {
        let err = parse_args([
            "build-single-cone",
            "--cone-root",
            "/tmp/foo",
            "--out",
            "/tmp/bar",
            "--inputs-fingerprint",
            "abc",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("hex"));
    }

    #[test]
    fn build_single_cone_rejects_missing_required_arg() {
        let err = parse_args([
            "build-single-cone",
            "--cone-root",
            "/tmp/foo",
            "--inputs-fingerprint",
            "00",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("--out"));
    }
}
