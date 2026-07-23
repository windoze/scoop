//! `scoop2c`：Scoop 下一代前端的驱动二进制。
//!
//! CLI 与旧 `scoopc` 的工具子集兼容，使现有 fixture 基础设施可以通过
//! `SCOOPC_BIN=target/debug/scoop2c python3 tools/run_fixtures.py
//! --fixtures tests/fixtures_ng` 零改动驱动本前端。
//!
//! 支持的子命令：
//!
//! - `check-source --phase <parse|resolve|typecheck|infer|lower> --input <path>
//!   [--source <file>] [--target-platform <p>]`
//! - `dump-ast <file.scoop>`
//! - `dump-hir <file.scoop>`
//!
//! 退出码约定（与旧 `scoopc` 一致）：成功 `0`；诊断错误 `1`；CLI 用法错误 `2`。

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod cli {
    use std::path::PathBuf;

    /// 解析后的命令行。
    #[derive(Debug)]
    pub enum Command {
        CheckSource(CheckSourceArgs),
        DumpAst { input: PathBuf },
        DumpHir { input: PathBuf },
    }

    #[derive(Debug)]
    pub struct CheckSourceArgs {
        pub phase: Phase,
        pub input: PathBuf,
        pub source: Option<PathBuf>,
        pub target_platform: Option<String>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Phase {
        Parse,
        Resolve,
        Typecheck,
        Infer,
        Lower,
    }

    impl Phase {
        pub fn parse(text: &str) -> Result<Phase, String> {
            match text {
                "parse" => Ok(Phase::Parse),
                "resolve" => Ok(Phase::Resolve),
                "typecheck" => Ok(Phase::Typecheck),
                "infer" => Ok(Phase::Infer),
                "lower" => Ok(Phase::Lower),
                other => Err(format!(
                    "未知 phase `{other}`（可选值：parse, resolve, typecheck, infer, lower）"
                )),
            }
        }
    }

    /// 解析命令行；用法错误返回 `Err(消息)`（退出码 2）。
    pub fn parse_args(args: &[String]) -> Result<Command, String> {
        let Some(subcommand) = args.first() else {
            return Err(usage());
        };
        let rest = &args[1..];
        match subcommand.as_str() {
            "check-source" => parse_check_source(rest).map(Command::CheckSource),
            "dump-ast" => parse_dump(rest).map(|input| Command::DumpAst { input }),
            "dump-hir" => parse_dump(rest).map(|input| Command::DumpHir { input }),
            "-h" | "--help" => Err(usage()),
            other => Err(format!("未知子命令 `{other}`\n\n{}", usage())),
        }
    }

    fn usage() -> String {
        "用法：\n  scoop2c check-source --phase <parse|resolve|typecheck|infer|lower> --input <path> [--source <file>] [--target-platform <p>]\n  scoop2c dump-ast <file.scoop>\n  scoop2c dump-hir <file.scoop>".to_string()
    }

    fn parse_check_source(args: &[String]) -> Result<CheckSourceArgs, String> {
        let mut phase = None;
        let mut input = None;
        let mut source = None;
        let mut target_platform = None;
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            let take_value = |i: &mut usize| -> Result<String, String> {
                *i += 1;
                args.get(*i)
                    .cloned()
                    .ok_or_else(|| format!("选项 {flag} 缺少参数值"))
            };
            match flag {
                "--phase" => phase = Some(Phase::parse(&take_value(&mut i)?)?),
                "--input" => input = Some(PathBuf::from(take_value(&mut i)?)),
                "--source" => source = Some(PathBuf::from(take_value(&mut i)?)),
                "--target-platform" => target_platform = Some(take_value(&mut i)?),
                other => return Err(format!("check-source: 未知选项 `{other}`")),
            }
            i += 1;
        }
        Ok(CheckSourceArgs {
            phase: phase.ok_or_else(|| "check-source: 缺少 --phase".to_string())?,
            input: input.ok_or_else(|| "check-source: 缺少 --input".to_string())?,
            source,
            target_platform,
        })
    }

    fn parse_dump(args: &[String]) -> Result<PathBuf, String> {
        match args {
            [input] => Ok(PathBuf::from(input)),
            _ => Err("dump-*: 需要恰好一个输入文件路径".to_string()),
        }
    }
}

use cli::Command;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::parse_args(&args) {
        Ok(command) => run(command),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn run(command: Command) -> ExitCode {
    match command {
        Command::CheckSource(args) => run_check_source(&args),
        Command::DumpAst { input } => run_dump_ast(&input),
        Command::DumpHir { input } => run_dump_hir(&input),
    }
}

fn load_source(path: &std::path::Path) -> Result<scoop2_base::SourceFile, ExitCode> {
    scoop2_base::SourceFile::load(path).map_err(|err| {
        eprintln!("error: {err}");
        ExitCode::from(1)
    })
}

/// 定位 sysroot 目录：优先环境变量 `SCOOP_SYSROOT`，否则相对当前可执行文件
/// （`target/debug/scoop2c` → `<root>/sysroot`）。找不到返回 `None`（前端仍可对
/// 不依赖内置类型的程序解析）。
fn locate_sysroot() -> Option<PathBuf> {
    if let Ok(s) = std::env::var("SCOOP_SYSROOT") {
        let p = PathBuf::from(s);
        if p.is_dir() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    // exe = <root>/target/<profile>/scoop2c
    let root = exe.parent()?.parent()?.parent()?;
    let p = root.join("sysroot");
    if p.is_dir() { Some(p) } else { None }
}

/// 收集 fixture 的 `.sysroot` overlay 目录中的 `.scoop` 文件。
fn collect_overlay_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(overlay) = std::env::var("SCOOP_SYSROOT_OVERLAY") {
        let p = PathBuf::from(&overlay);
        if p.is_dir() {
            walk_inner(&p, &mut out);
            out.sort();
        }
    }
    out
}

/// 递归收集 `dir` 下的所有 `*.scoop` 文件路径（按路径排序，保证确定性）。
fn walk_scoop_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_inner(dir, &mut out);
    out.sort();
    out
}

fn walk_inner(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_inner(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
}

fn run_check_source(args: &cli::CheckSourceArgs) -> ExitCode {
    let _ = (&args.source, &args.target_platform);
    let source = match load_source(&args.input) {
        Ok(source) => source,
        Err(code) => return code,
    };
    match args.phase {
        cli::Phase::Parse => {
            let result = scoop2_syntax::parser::parse_file(&source);
            report_diagnostics(&source, result.diagnostics)
        }
        cli::Phase::Resolve => {
            let mut interner = scoop2_base::Interner::new();
            let mut diags = scoop2_base::diag::DiagnosticSink::new();
            // 解析用户文件（FileId 0）+ sysroot 全量（作为 prelude/依赖符号）。
            let mut parsed: Vec<scoop2_syntax::parser::ParsedFile> = Vec::with_capacity(1 + 32);
            parsed.push(scoop2_syntax::parser::parse_file_with(
                &source,
                &mut interner,
            ));
            if let Some(sysroot) = locate_sysroot() {
                for path in walk_scoop_files(&sysroot.join("lib")) {
                    if let Ok(src) = scoop2_base::SourceFile::load_sysroot(&path) {
                        parsed.push(scoop2_syntax::parser::parse_file_with(&src, &mut interner));
                    }
                }
            }
            // Fixture sysroot overlay（`.sysroot` 目录中的 `.scoop` 文件）。
            for path in collect_overlay_files() {
                if let Ok(src) = scoop2_base::SourceFile::load_sysroot(&path) {
                    parsed.push(scoop2_syntax::parser::parse_file_with(&src, &mut interner));
                }
            }
            let user_parse_ok = !parsed[0].diagnostics.has_errors();
            for pf in &parsed {
                diags.extend(pf.diagnostics.iter().cloned());
            }
            if user_parse_ok {
                let inputs: Vec<scoop2_hir::resolve::InputFile> = parsed
                    .iter()
                    .enumerate()
                    .map(|(i, pf)| scoop2_hir::resolve::InputFile {
                        file: &pf.file,
                        file_id: scoop2_base::FileId(i as u32),
                        origin: if i == 0 {
                            scoop2_hir::resolve::InputOrigin::User
                        } else {
                            scoop2_hir::resolve::InputOrigin::Sysroot
                        },
                        // 主文件（i==0）非受信任；sysroot 文件受信任。
                        trusted: i != 0,
                    })
                    .collect();
                scoop2_hir::resolve::run_program(&inputs, &mut interner, &mut diags);
            }
            report_diagnostics(&source, diags)
        }
        cli::Phase::Typecheck => {
            let mut interner = scoop2_base::Interner::new();
            let mut diags = scoop2_base::diag::DiagnosticSink::new();
            let mut parsed: Vec<scoop2_syntax::parser::ParsedFile> = Vec::with_capacity(1 + 32);
            let mut user_indices: Vec<usize> = vec![0]; // 主文件始终是 user
            parsed.push(scoop2_syntax::parser::parse_file_with(
                &source,
                &mut interner,
            ));
            if let Some(sysroot) = locate_sysroot() {
                for path in walk_scoop_files(&sysroot.join("lib")) {
                    if let Ok(src) = scoop2_base::SourceFile::load_sysroot(&path) {
                        parsed.push(scoop2_syntax::parser::parse_file_with(&src, &mut interner));
                    }
                }
            }
            // Fixture sysroot overlay（`.sysroot` 目录中的 `.scoop` 文件）→ 当作用户代码检查。
            for path in collect_overlay_files() {
                if let Ok(src) = scoop2_base::SourceFile::load_sysroot(&path) {
                    user_indices.push(parsed.len());
                    parsed.push(scoop2_syntax::parser::parse_file_with(&src, &mut interner));
                }
            }
            let user_parse_ok = !parsed[0].diagnostics.has_errors();
            for pf in &parsed {
                diags.extend(pf.diagnostics.iter().cloned());
            }
            if user_parse_ok {
                let inputs: Vec<scoop2_hir::resolve::InputFile> = parsed
                    .iter()
                    .enumerate()
                    .map(|(i, pf)| scoop2_hir::resolve::InputFile {
                        file: &pf.file,
                        file_id: scoop2_base::FileId(i as u32),
                        origin: if user_indices.contains(&i) {
                            scoop2_hir::resolve::InputOrigin::User
                        } else {
                            scoop2_hir::resolve::InputOrigin::Sysroot
                        },
                        // 主文件（i==0）非受信任；sysroot + `.sysroot` overlay 文件受信任。
                        trusted: i != 0,
                    })
                    .collect();
                scoop2_hir::typecheck::run_typecheck(&inputs, &mut interner, &mut diags);
            }
            report_diagnostics(&source, diags)
        }
        cli::Phase::Infer | cli::Phase::Lower => not_wired("check-source"),
    }
}

fn run_dump_ast(input: &std::path::Path) -> ExitCode {
    let source = match load_source(input) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let result = scoop2_syntax::parser::parse_file(&source);
    if result.diagnostics.has_errors() {
        return report_diagnostics(&source, result.diagnostics);
    }
    print!(
        "{}",
        scoop2_syntax::dump::dump_file(&result.file, &result.interner)
    );
    ExitCode::SUCCESS
}

/// 渲染诊断；有错误返回退出码 1，否则 0。
fn report_diagnostics(
    source: &scoop2_base::SourceFile,
    mut diagnostics: scoop2_base::diag::DiagnosticSink,
) -> ExitCode {
    if !diagnostics.has_errors() {
        return ExitCode::SUCCESS;
    }
    diagnostics.sort_by_offset();
    for diag in diagnostics.iter() {
        eprint!("{}", scoop2_base::diag::render_diagnostic(source, diag));
    }
    ExitCode::from(1)
}

fn not_wired(command: &str) -> ExitCode {
    eprintln!("error: scoop2c: `{command}` 的前端管线尚未接通（前端重写进行中）");
    ExitCode::from(1)
}

fn run_dump_hir(input: &std::path::Path) -> ExitCode {
    let _ = input;
    eprintln!("error: scoop2c: dump-hir 的语义阶段尚未接通（前端重写进行中）");
    ExitCode::from(1)
}
