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

use std::path::PathBuf;
use std::process::ExitCode;

mod cli {
    use std::path::PathBuf;

    /// 解析后的命令行。
    #[derive(Debug)]
    pub enum Command {
        CheckSource(CheckSourceArgs),
        DumpAst { input: PathBuf },
        DumpHir { input: PathBuf },
        DumpMir { input: PathBuf },
        DumpLir { input: PathBuf },
        Build(BuildArgs),
        Run(RunArgs),
        HirBuild { input: PathBuf, out: PathBuf },
        MirBuild { dir: PathBuf },
    DumpMirArch { dir: PathBuf },
    }

    #[derive(Debug)]
    pub struct BuildArgs {
        pub input: PathBuf,
        pub output: PathBuf,
        pub emit_ir: Option<PathBuf>,
    }

    #[derive(Debug)]
    pub struct RunArgs {
        pub input: PathBuf,
        pub args: Vec<String>,
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
            "dump-mir" => parse_dump(rest).map(|input| Command::DumpMir { input }),
            "dump-lir" => parse_dump(rest).map(|input| Command::DumpLir { input }),
            "build" => parse_build(rest).map(Command::Build),
            "run" => parse_run(rest).map(Command::Run),
            "hir-build" => parse_hir_build(rest),
            "mir-build" => parse_mir_build(rest),
            "dump-mir-arch" => parse_mir_build(rest).map(|c| match c {
                Command::MirBuild { dir } => Command::DumpMirArch { dir },
                other => other,
            }),
            "-h" | "--help" => Err(usage()),
            other => Err(format!("未知子命令 `{other}`\n\n{}", usage())),
        }
    }

    fn parse_build(args: &[String]) -> Result<BuildArgs, String> {
        let mut input = None;
        let mut output = None;
        let mut emit_ir = None;
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            let take = |i: &mut usize| -> Result<String, String> {
                *i += 1;
                args.get(*i)
                    .cloned()
                    .ok_or_else(|| format!("选项 {flag} 缺少参数值"))
            };
            match flag {
                "-o" | "--output" => output = Some(PathBuf::from(take(&mut i)?)),
                "--emit-ir" => emit_ir = Some(PathBuf::from(take(&mut i)?)),
                other if other.starts_with('-') => {
                    return Err(format!("build: 未知选项 `{other}`"));
                }
                other => {
                    if input.is_none() {
                        input = Some(PathBuf::from(other));
                    } else {
                        return Err(format!("build: 多余的位置参数 `{other}`"));
                    }
                }
            }
            i += 1;
        }
        Ok(BuildArgs {
            input: input.ok_or_else(|| "build: 缺少输入文件".to_string())?,
            output: output.ok_or_else(|| "build: 缺少 -o 输出路径".to_string())?,
            emit_ir,
        })
    }

    fn parse_run(args: &[String]) -> Result<RunArgs, String> {
        if args.is_empty() {
            return Err("run: 缺少输入文件".to_string());
        }
        Ok(RunArgs {
            input: PathBuf::from(&args[0]),
            args: args[1..].to_vec(),
        })
    }

    fn usage() -> String {
        "用法：\n  scoop2c check-source --phase <parse|resolve|typecheck|infer|lower> --input <path> [--source <file>] [--target-platform <p>]\n  scoop2c dump-ast <file.scoop>\n  scoop2c dump-hir <file.scoop>\n  scoop2c dump-mir <file.scoop>\n  scoop2c dump-lir <file.scoop>\n  scoop2c hir-build <file.scoop> -o <dir>\n  scoop2c mir-build <dir>".to_string()
    }

    fn parse_hir_build(args: &[String]) -> Result<Command, String> {
        let mut input = None;
        let mut out = None;
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            match flag {
                "-o" | "--output" => {
                    i += 1;
                    out = Some(PathBuf::from(
                        args.get(i)
                            .ok_or_else(|| "hir-build: -o 缺少参数值".to_string())?,
                    ));
                }
                other if other.starts_with('-') => {
                    return Err(format!("hir-build: 未知选项 `{other}`"));
                }
                other => {
                    if input.is_none() {
                        input = Some(PathBuf::from(other));
                    } else {
                        return Err(format!("hir-build: 多余的位置参数 `{other}`"));
                    }
                }
            }
            i += 1;
        }
        Ok(Command::HirBuild {
            input: input.ok_or_else(|| "hir-build: 缺少输入文件".to_string())?,
            out: out.ok_or_else(|| "hir-build: 缺少 -o 输出目录".to_string())?,
        })
    }

    fn parse_mir_build(args: &[String]) -> Result<Command, String> {
        match args {
            [dir] => Ok(Command::MirBuild {
                dir: PathBuf::from(dir),
            }),
            _ => Err("mir-build: 需要恰好一个 archive 目录路径（含 collection.hirv0）".to_string()),
        }
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

use scoop2_archive::pipeline::{
    BuiltProgram, build_program, collect_overlay_files, locate_sysroot, make_inputs,
    read_declared_deps, walk_scoop_files,
};

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
        Command::DumpMir { input } => run_dump_mir(&input),
        Command::DumpLir { input } => run_dump_lir(&input),
        Command::Build(args) => run_build(&args),
        Command::Run(args) => run_run(&args),
        Command::HirBuild { input, out } => run_hir_build(&input, &out),
        Command::MirBuild { dir } => run_mir_build(&dir),
        Command::DumpMirArch { dir } => run_dump_mir_arch(&dir),
    }
}

/// `hir-build`：parse → typecheck → 写出 v0 HIR archive collection（per-cone
/// `.hirarch` + `collection.hirv0`）。之后 MIR 只需该目录即可工作（源文件可删）。
fn run_hir_build(input: &std::path::Path, out: &std::path::Path) -> ExitCode {
    let source = match load_source(input) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let mut program = build_program(&source);
    let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = program
        .sources
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
        .collect();
    match scoop2_archive::pipeline::typecheck_program(&mut program, None) {
        Ok(hir) => match scoop2_archive::v0::write_hir_collection(out, &program, &hir, &[]) {
            Ok(files) => {
                for f in files {
                    println!("{}", f.display());
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        },
        Err(diags) => report_diagnostics(&source, &extra_sources, diags),
    }
}

/// `mir-build`：从 HIR archive collection 目录装配并走 MIR，输出 MIR dump。
/// 只读目录内容（PLAN.md C1/C8——不读源文件 / 不重新 parse）。
fn run_mir_build(dir: &std::path::Path) -> ExitCode {
    let outcome = scoop2_archive::v0::load_hir_collection(dir)
        .map_err(scoop2_archive::v0::StageError::from)
        .and_then(|loaded| {
            let (dump, mat) = scoop2_archive::v0::run_mir_and_dump(&loaded.hir)?;
            // M3-6：MIR archive 落地（指纹 = 成员 cone keys + 全局参数——C7）。
            let members = loaded.members.clone();
            let _archive_path =
                scoop2_archive::v0::write_mir_archive(dir, &loaded.hir, &mat, &members, &[])?;
            Ok(dump)
        });
    match outcome {
        Ok(dump) => {
            print!("{dump}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

/// dump-mir-arch：从 MIR archive 渲染 dump（纯读——M3-6 往返验证）。
fn run_dump_mir_arch(dir: &std::path::Path) -> ExitCode {
    match scoop2_archive::v0::load_mir_archive(dir) {
        Ok(archive) => {
            print!("{}", scoop2_archive::v0::dump_from_mir_archive(&archive));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn load_source(path: &std::path::Path) -> Result<scoop2_base::SourceFile, ExitCode> {
    scoop2_base::SourceFile::load(path).map_err(|err| {
        eprintln!("error: {err}");
        ExitCode::from(1)
    })
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
            report_diagnostics(&source, &[], result.diagnostics)
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
                    .iter_mut()
                    .enumerate()
                    .map(|(i, pf)| scoop2_hir::resolve::InputFile {
                        file: &mut pf.file,
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
            report_diagnostics(&source, &[], diags)
        }
        cli::Phase::Typecheck | cli::Phase::Infer | cli::Phase::Lower => {
            // infer / lower 在本前端中等价于 typecheck：HIR 是前端终点（spec 明确
            // 「只关心 parser/AST/HIR 阶段涵盖的错误」），不再有独立 infer/lower 阶段。
            // 三个 phase 都运行完整的 typecheck 管线并汇报相同诊断。
            let BuiltProgram {
                mut parsed,
                sources,
                user_indices,
                mut interner,
                mut diags,
            } = build_program(&source);
            let user_parse_ok = !parsed[0].diagnostics.has_errors();
            for pf in &parsed {
                diags.extend(pf.diagnostics.iter().cloned());
            }
            if user_parse_ok {
                let mut inputs = make_inputs(&mut parsed, &user_indices);
                let declared_deps: Vec<String> = read_declared_deps().into_iter().collect();
                scoop2_hir::typecheck::run_typecheck(
                    &mut inputs,
                    &mut interner,
                    &mut diags,
                    args.target_platform.as_deref(),
                    &declared_deps,
                );
            }
            // 构建跨文件源映射（FileId → SourceFile）供多文件诊断渲染。
            let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
                .iter()
                .enumerate()
                .skip(1)
                .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
                .collect();
            report_diagnostics(&source, &extra_sources, diags)
        }
    }
}

fn run_dump_ast(input: &std::path::Path) -> ExitCode {
    let source = match load_source(input) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let result = scoop2_syntax::parser::parse_file(&source);
    if result.diagnostics.has_errors() {
        return report_diagnostics(&source, &[], result.diagnostics);
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
    extra_sources: &[(scoop2_base::FileId, scoop2_base::SourceFile)],
    mut diagnostics: scoop2_base::diag::DiagnosticSink,
) -> ExitCode {
    if !diagnostics.has_errors() {
        return ExitCode::SUCCESS;
    }
    // 去重：resolve 已报 unresolved_type 的位置，移除 typecheck 的 unresolved_type_ref。
    diagnostics.dedup_redundant(
        "scoop::typecheck::unresolved_type_ref",
        "scoop::resolve::unresolved_type",
    );
    diagnostics.sort_by_offset();
    for diag in diagnostics.iter() {
        eprint!(
            "{}",
            scoop2_base::diag::render_diagnostic_multi(source, extra_sources, diag)
        );
    }
    ExitCode::from(1)
}

fn run_dump_hir(input: &std::path::Path) -> ExitCode {
    let source = match load_source(input) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let BuiltProgram {
        mut parsed,
        sources,
        user_indices,
        mut interner,
        mut diags,
    } = build_program(&source);
    let user_parse_ok = !parsed[0].diagnostics.has_errors();
    for pf in &parsed {
        diags.extend(pf.diagnostics.iter().cloned());
    }
    if !user_parse_ok {
        return report_diagnostics(&source, &[], diags);
    }
    let mut inputs = make_inputs(&mut parsed, &user_indices);
    let declared_deps: Vec<String> = read_declared_deps().into_iter().collect();
    let mut hir = scoop2_hir::typecheck::run_typecheck(
        &mut inputs,
        &mut interner,
        &mut diags,
        None,
        &declared_deps,
    );
    if diags.has_errors() {
        // 构建跨文件源映射（FileId → SourceFile）供多文件诊断渲染。
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1) // 跳过主文件（source 本身）。
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, diags);
    }
    // dump-hir 是尽力而为的调试视图：未类型化的节点（typecheck 未覆盖的边角）
    // 不追加 `ty=`，而非阻塞输出。完整性闸门（`completeness::verify`）供
    // 需要严格完整性的编译管线调用，不在此强制。
    let files: Vec<(scoop2_base::FileId, &scoop2_syntax::ast::File)> = parsed
        .iter()
        .enumerate()
        .filter(|(i, _)| user_indices.contains(i))
        .map(|(i, pf)| (scoop2_base::FileId(i as u32), &pf.file))
        .collect();
    let rendered = hir.render(files.iter().map(|(id, f)| (*id, *f)));
    print!("{rendered}");
    ExitCode::SUCCESS
}

/// `dump-mir`：typecheck → MIR lowering → 文本 dump。
///
/// 流程：parse + resolve + typecheck（与 dump-hir 同）→ 完整性闸门
/// (`completeness::verify`，作为 MIR 消费前的门禁) → `scoop2_mir::lower` →
/// `scoop2_mir::dump`。有 typecheck / 完整性 / lowering 错误则报诊断并退出 1。
fn run_dump_mir(input: &std::path::Path) -> ExitCode {
    let source = match load_source(input) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let BuiltProgram {
        mut parsed,
        sources,
        user_indices,
        mut interner,
        mut diags,
    } = build_program(&source);
    let user_parse_ok = !parsed[0].diagnostics.has_errors();
    for pf in &parsed {
        diags.extend(pf.diagnostics.iter().cloned());
    }
    if !user_parse_ok {
        return report_diagnostics(&source, &[], diags);
    }
    let mut inputs = make_inputs(&mut parsed, &user_indices);
    let declared_deps: Vec<String> = read_declared_deps().into_iter().collect();
    let mut hir = scoop2_hir::typecheck::run_typecheck(
        &mut inputs,
        &mut interner,
        &mut diags,
        None,
        &declared_deps,
    );
    if diags.has_errors() {
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, diags);
    }
    // 完整性闸门：run_typecheck 末尾已无条件启用 completeness::verify（untyped_node
    // 诊断已 push 进 diags）。此处无需再调用；有 untyped_node 时 diags.has_errors() 为真，
    // 报诊断退出（保证 MIR 只消费完整 HIR）。
    // M2-5 翻转前置：补建 HIR 树 + 骨架（MIR 只消费树）。
    scoop2_archive::pipeline::attach_trees(&mut hir, &parsed, &interner);
    let mir_files: Vec<(scoop2_base::FileId, &scoop2_syntax::ast::File)> = parsed
        .iter()
        .enumerate()
        .filter(|(i, _)| user_indices.contains(i))
        .map(|(i, pf)| (scoop2_base::FileId(i as u32), &pf.file))
        .collect();
    // MIR lowering。
    let mut lower_diags = scoop2_base::diag::DiagnosticSink::new();
    // M2-5 翻转：MIR 只消费 HIR 产出（树 + 骨架——lower_module_from_trees）。
    let lower_result = scoop2_mir::mir::lower_tree::lower_module_from_trees(&hir, &mut lower_diags);
    if lower_diags.has_errors() || !lower_result.errors.is_empty() {
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, lower_diags);
    }
    // 单态化（generic → monomorphic）：自 entry main 起 BFS。单态化错误（如缺模板）在此报。
    // 注意：dump 输出仍是 generic 模板模块（与 mir2 golden 一致）；materialize 仅用于
    // 触发单态化阶段错误检测。
    //
    // dump-mir 模拟可执行程序的 MIR，需要 entry `main`：从模块中查找名为 main 的函数 FQN
    // 作为种子。若无 main（库 / 缺入口），materialize 以 `Some("main")` 为种子但模板集合
    // 无 `main` → 报 `scoop::mir::monomorph_no_template`（单态化阶段明确拒绝缺入口的程序）。
    let entry = lower_result
        .module
        .items
        .iter()
        .find_map(|it| match it {
            scoop2_mir::mir::Item::Fun(fd) if fd.name == "main" => Some(fd.fqn.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "main".to_string());
    let monomorph_result =
        scoop2_mir::mir::materialize::materialize(lower_result.module.clone(), Some(&entry), &hir);
    if let Err(merr) = monomorph_result {
        lower_diags.push(merr.to_diagnostic());
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, lower_diags);
    }
    // MIR 验证：CFG 结构 + direct-style + production 语义完整性。
    // 构建外部符号集：从 HIR 收集所有已知的函数/类型 FQN（含 sysroot/prelude）。
    let mut external_symbols: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (&fqn_sym, _) in &hir.top_level_funs {
        external_symbols.insert(hir.interner.resolve(fqn_sym).to_string());
    }
    for (&type_sym, _) in &hir.enum_variants {
        external_symbols.insert(hir.interner.resolve(type_sym).to_string());
    }
    for (&type_sym, _) in &hir.members {
        external_symbols.insert(hir.interner.resolve(type_sym).to_string());
    }
    for (&type_sym, methods) in &hir.member_funs {
        let type_fqn_text = hir.interner.resolve(type_sym).to_string();
        external_symbols.insert(type_fqn_text.clone());
        // 同时插入 owner.method 形式的 FQN（如 scoop.core.Int.plus）。
        for (&method_sym, _) in methods {
            let method_name = hir.interner.resolve(method_sym);
            external_symbols.insert(format!("{}.{}", type_fqn_text, method_name));
        }
    }
    for (&val_sym, _) in &hir.top_level_vals {
        external_symbols.insert(hir.interner.resolve(val_sym).to_string());
    }
    let verify_errors = scoop2_mir::mir::verify::verify_module_with_external(
        &lower_result.module,
        &external_symbols,
    );
    if !verify_errors.is_empty() {
        for ve in &verify_errors {
            lower_diags.push(scoop2_base::diag::Diagnostic::error(
                ve.code,
                ve.message.clone(),
            ));
        }
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, lower_diags);
    }
    // materialized MIR 验证：transport 契约一致性 + 泛型参数残留检查。
    // 对单态化后的模块运行，确保无 TypeKind::Param 残留存活到后端。
    let monomorph = monomorph_result
        .as_ref()
        .expect("materialize 已成功（错误路径已 return）");
    let mat_errors = scoop2_mir::mir::verify::verify_materialized_with_external(
        &monomorph.module,
        &external_symbols,
    );
    if !mat_errors.is_empty() {
        for ve in &mat_errors {
            lower_diags.push(scoop2_base::diag::Diagnostic::error(
                ve.code,
                ve.message.clone(),
            ));
        }
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, lower_diags);
    }
    // dump（generic 模板模块）。
    let rendered = scoop2_mir::mir::dump::dump_module(&lower_result.module, &hir.interner);
    print!("{rendered}");
    ExitCode::SUCCESS
}

/// `dump-lir`：typecheck → MIR lowering → materialize → LIR lowering → 文本 dump。
fn run_dump_lir(input: &std::path::Path) -> ExitCode {
    // 复用 dump-mir 的管线到 materialize 阶段。
    let source = match load_source(input) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let BuiltProgram {
        mut parsed,
        sources,
        user_indices,
        mut interner,
        mut diags,
    } = build_program(&source);
    let user_parse_ok = !parsed[0].diagnostics.has_errors();
    for pf in &parsed {
        diags.extend(pf.diagnostics.iter().cloned());
    }
    if !user_parse_ok {
        return report_diagnostics(&source, &[], diags);
    }
    let mut inputs = make_inputs(&mut parsed, &user_indices);
    let declared_deps: Vec<String> = read_declared_deps().into_iter().collect();
    let mut hir = scoop2_hir::typecheck::run_typecheck(
        &mut inputs,
        &mut interner,
        &mut diags,
        None,
        &declared_deps,
    );
    if diags.has_errors() {
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, diags);
    }
    // M2-5 翻转：树驱动 lowering（用户文件树 + 骨架；先补建树）。
    scoop2_archive::pipeline::attach_trees(&mut hir, &parsed, &interner);
    let lower_result = scoop2_mir::mir::lower_tree::lower_module_from_trees(&hir, &mut diags);
    if diags.has_errors() {
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, diags);
    }
    // 查找 entry。
    let entry = lower_result
        .module
        .items
        .iter()
        .find_map(|it| match it {
            scoop2_mir::mir::Item::Fun(fd) if fd.name == "main" => Some(fd.fqn.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "main".to_string());
    // materialize。
    let monomorph_result =
        scoop2_mir::mir::materialize::materialize(lower_result.module.clone(), Some(&entry), &hir);
    if let Err(merr) = monomorph_result {
        diags.push(merr.to_diagnostic());
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        return report_diagnostics(&source, &extra_sources, diags);
    }
    let monomorph = monomorph_result.as_ref().expect("materialize 已成功");
    // LIR lowering。
    let lir_program = match scoop2_lir::lower_to_lir(monomorph, &hir, &interner) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    // dump。
    let rendered = scoop2_lir::dump::dump_program(&lir_program);
    print!("{rendered}");
    ExitCode::SUCCESS
}

// =========================================================================
// build / run：完整 e2e 管线（parse → typecheck[sysroot bodies] → MIR → materialize → LIR → codegen）
// =========================================================================

/// 共享 e2e 构建管线：返回 LirProgram（含 sysroot 函数体的单态化）。
/// 失败时打印诊断并返回 None。
fn build_lir_program(source: &scoop2_base::SourceFile) -> Option<scoop2_lir::LirProgram> {
    let BuiltProgram {
        mut parsed,
        sources,
        user_indices,
        mut interner,
        mut diags,
    } = build_program(source);
    let user_parse_ok = !parsed[0].diagnostics.has_errors();
    for pf in &parsed {
        diags.extend(pf.diagnostics.iter().cloned());
    }
    if !user_parse_ok {
        report_diagnostics(source, &[], diags);
        return None;
    }
    let mut inputs = make_inputs(&mut parsed, &user_indices);
    let declared_deps: Vec<String> = read_declared_deps().into_iter().collect();
    // e2e：启用 sysroot 函数体 typecheck（println<String> 等库函数可单态化）。
    let mut hir = scoop2_hir::typecheck::run_typecheck_with_options(
        &mut inputs,
        &mut interner,
        &mut diags,
        None,
        &declared_deps,
        true,
    );
    if diags.has_errors() {
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        report_diagnostics(source, &extra_sources, diags);
        return None;
    }
    // MIR lowering：sysroot 全量（库函数体）——M2-5 翻转：树驱动（sysroot
    // TypedFile 在 lower_sysroot_bodies=true 时产出，attach_trees 覆盖全量）。
    scoop2_archive::pipeline::attach_trees(&mut hir, &parsed, &interner);
    let mut lower_diags = scoop2_base::diag::DiagnosticSink::new();
    let lower_result =
        scoop2_mir::mir::lower_tree::lower_module_from_trees(&hir, &mut lower_diags);
    if lower_diags.has_errors() {
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        report_diagnostics(source, &extra_sources, lower_diags);
        return None;
    }
    // 查找 entry main。
    let entry = lower_result
        .module
        .items
        .iter()
        .find_map(|it| match it {
            scoop2_mir::mir::Item::Fun(fd) if fd.name == "main" => Some(fd.fqn.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "main".to_string());
    let monomorph_result =
        scoop2_mir::mir::materialize::materialize(lower_result.module.clone(), Some(&entry), &hir);
    if let Err(merr) = monomorph_result {
        lower_diags.push(merr.to_diagnostic());
        let extra_sources: Vec<(scoop2_base::FileId, scoop2_base::SourceFile)> = sources
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, s)| (scoop2_base::FileId(i as u32), s.clone()))
            .collect();
        report_diagnostics(source, &extra_sources, lower_diags);
        return None;
    }
    let monomorph = monomorph_result.as_ref().expect("materialize 已成功");
    let lir_program = match scoop2_lir::lower_to_lir(monomorph, &hir, &interner) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return None;
        }
    };
    let _ = sources;
    Some(lir_program)
}

/// 定位 `libscooprt.a`：优先环境变量 `SCOOP_LIBSCROOT`，否则在已知 target 目录搜索。
fn locate_libscooprt() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SCOOP_LIBSCROOT") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    let target_dir: PathBuf = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(t) => PathBuf::from(t),
        None => {
            let exe = std::env::current_exe().ok()?;
            // exe = target/<profile>/scoop2c → target/
            exe.parent()?.parent()?.to_path_buf()
        }
    };
    // 1. target/<profile>/liblibscooprt.a（cc 默认链接产物）。
    for cand in ["liblibscooprt.a", "libscooprt.a"] {
        let p = target_dir.join(cand);
        if p.is_file() {
            return Some(p);
        }
    }
    // 2. cc build script OUT_DIR：target/<profile>/build/scoop_runtime-*/out/libscooprt.a
    let build_dir = target_dir.join("debug").join("build");
    if let Ok(entries) = std::fs::read_dir(&build_dir) {
        let mut found: Vec<PathBuf> = Vec::new();
        for e in entries.flatten() {
            let name = e.file_name();
            if name.to_string_lossy().starts_with("scoop_runtime-") {
                let cand = e.path().join("out").join("libscooprt.a");
                if cand.is_file() {
                    found.push(cand);
                }
            }
        }
        // 取最新修改时间的一个（降序，最新在前）。
        found.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        if let Some(latest) = found.into_iter().next_back() {
            return Some(latest);
        }
    }
    None
}

fn run_build(args: &cli::BuildArgs) -> ExitCode {
    let source = match load_source(&args.input) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let lir_program = match build_lir_program(&source) {
        Some(p) => p,
        None => return ExitCode::from(1),
    };
    // codegen → object。
    let options = scoop2_codegen_llvm::EmitOptions::default();
    let tmp_obj = std::env::temp_dir().join(format!("scoop2c_build_{}.o", std::process::id()));
    if let Err(e) = scoop2_codegen_llvm::emit_object_to_file(&lir_program, &tmp_obj, &options) {
        eprintln!("error: codegen 失败：{e}");
        return ExitCode::from(1);
    }
    if let Some(ir_path) = &args.emit_ir {
        let emitted = scoop2_codegen_llvm::emit_program(&lir_program, &options);
        if let Ok(emitted) = emitted {
            let _ = std::fs::write(ir_path, emitted.ir_text);
        }
    }
    // 链接：clang <obj> libscooprt.a -o <exe>。
    let libscooprt = match locate_libscooprt() {
        Some(p) => p,
        None => {
            eprintln!("error: 找不到 libscooprt.a（设置 SCOOP_LIBSCROOT 环境变量）");
            return ExitCode::from(1);
        }
    };
    let clang = std::env::var("SCOOP_LINKER").unwrap_or_else(|_| "clang".to_string());
    let link_status = std::process::Command::new(&clang)
        .arg(&tmp_obj)
        .arg(&libscooprt)
        .arg("-o")
        .arg(&args.output)
        .arg("-lpthread")
        .status();
    let _ = std::fs::remove_file(&tmp_obj);
    match link_status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => {
            eprintln!("error: 链接失败（exit {:?}）", s.code());
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("error: 无法启动链接器 {clang}：{e}");
            ExitCode::from(1)
        }
    }
}

fn run_run(args: &cli::RunArgs) -> ExitCode {
    let exe = std::env::temp_dir().join(format!("scoop2c_run_{}", std::process::id()));
    let build_args = cli::BuildArgs {
        input: args.input.clone(),
        output: exe.clone(),
        emit_ir: None,
    };
    let code = run_build(&build_args);
    if code != ExitCode::SUCCESS {
        return code;
    }
    let status = std::process::Command::new(&exe).args(&args.args).status();
    let _ = std::fs::remove_file(&exe);
    match status {
        Ok(s) => match s.code() {
            Some(c) => ExitCode::from((c & 0xff) as u8),
            None => ExitCode::FAILURE,
        },
        Err(e) => {
            eprintln!("error: 无法执行 {exe:?}：{e}");
            ExitCode::from(1)
        }
    }
}
