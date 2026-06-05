//! Compiler-owned diagnostic/tooling commands exposed through the `scoopc` binary.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result};
use object::{Architecture, Object as _, ObjectSection as _, ObjectSymbol as _, SymbolKind};

use crate::session::{Session, SessionOptions};
use crate::source::SourceFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSourcePhase {
    Parse,
    Resolve,
    Typecheck,
    Infer,
}

impl CheckSourcePhase {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "parse" => Ok(Self::Parse),
            "resolve" => Ok(Self::Resolve),
            "typecheck" => Ok(Self::Typecheck),
            "infer" => Ok(Self::Infer),
            other => Err(miette::miette!(
                "未知 check-source phase `{other}`（期望 parse|resolve|typecheck|infer）"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitArtifactKind {
    LlvmIr,
    Object,
    Asm,
}

impl EmitArtifactKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "llvm-ir" | "llvm" | "ll" => Ok(Self::LlvmIr),
            "obj" | "object" => Ok(Self::Object),
            "asm" => Ok(Self::Asm),
            other => Err(miette::miette!(
                "未知 emit artifact kind `{other}`（期望 llvm-ir|obj|asm）"
            )),
        }
    }

    #[cfg(feature = "llvm")]
    fn llvm_kind(self) -> crate::pipeline::LlvmArtifactKind {
        match self {
            EmitArtifactKind::LlvmIr => crate::pipeline::LlvmArtifactKind::LlvmIr,
            EmitArtifactKind::Object => crate::pipeline::LlvmArtifactKind::Object,
            EmitArtifactKind::Asm => crate::pipeline::LlvmArtifactKind::Asm,
        }
    }
}

#[cfg(feature = "llvm")]
pub fn run_emit_artifact(
    input: PathBuf,
    output: PathBuf,
    kind: EmitArtifactKind,
    opt_level: crate::opt::OptLevel,
    session_options: SessionOptions,
) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .into_diagnostic()
            .wrap_err_with(|| format!("无法创建输出目录：{}", parent.display()))?;
    }
    let session = Session::with_options(session_options.clone())?;
    let project = crate::frontend::load_project_input_from_path(&input, None, &session_options)?;
    let context = crate::frontend::ProjectContext::new(project);
    let front = crate::frontend::run_project_frontend(&session, context)?;
    crate::pipeline::emit_project_llvm_artifact_to_file(
        &session,
        &front,
        &output,
        opt_level,
        kind.llvm_kind(),
    )?;
    Ok(())
}

pub fn run_check_source(
    input: PathBuf,
    source: Option<PathBuf>,
    phase: CheckSourcePhase,
    target_platform: Option<String>,
    session_options: SessionOptions,
) -> Result<()> {
    let session_options = if let Some(platform) = target_platform {
        session_options.with_target_platform(crate::target::TargetPlatform::new(platform))
    } else {
        session_options
    };
    let input = canonical_input(input)?;
    let session = Session::with_options(session_options.clone())?;

    if input.is_file() {
        if source.is_some() {
            return Err(miette::miette!(
                "`check-source --source` 只能配合 cone project 目录输入使用"
            ));
        }
        let source = SourceFile::load(&input)?;
        return run_check_source_file(&session, &source, phase);
    }

    if input.is_dir() {
        let project =
            crate::frontend::load_single_cone_project_input_from_path(&input, &session_options)?;
        let selected = select_project_sources(&project, &input, source.as_deref())?;
        return run_check_source_project(&session, &project, &selected, phase);
    }

    Err(miette::miette!(
        "check-source 输入既不是文件也不是目录：{}",
        input.display()
    ))
}

fn run_check_source_file(
    session: &Session,
    source: &SourceFile,
    phase: CheckSourcePhase,
) -> Result<()> {
    match phase {
        CheckSourcePhase::Parse => crate::pipeline::load_ast_stage_output_for_dump(session, source)
            .map(|_| ())
            .map_err(|err| located_report(source, err)),
        CheckSourcePhase::Resolve => {
            let mut ast =
                parse_source(session, source).map_err(|err| located_report(source, err))?;
            let index = build_single_source_index(session, source, &ast)
                .map_err(|err| located_report(source, err))?;
            crate::resolve::check_file_bindings(source, &mut ast, &index)
                .map_err(|err| located_report(source, err))
        }
        CheckSourcePhase::Typecheck => {
            let mut ast =
                parse_source(session, source).map_err(|err| located_report(source, err))?;
            run_typecheck_for_source(session, source, &mut ast)
        }
        CheckSourcePhase::Infer => crate::pipeline::load_hir_stage_output_for_dump(session, source)
            .map(|_| ())
            .map_err(|err| located_report(source, err)),
    }
}

fn parse_source(
    session: &Session,
    source: &SourceFile,
) -> std::result::Result<crate::ast::File, crate::parser::ParseError> {
    session.parse(source)
}

fn build_single_source_index(
    session: &Session,
    source: &SourceFile,
    ast: &crate::ast::File,
) -> std::result::Result<crate::resolve::Index, crate::resolve::ResolveError> {
    let mut pairs: Vec<(&SourceFile, &crate::ast::File)> = Vec::new();
    for file in session.sysroot().index_files() {
        pairs.push((&file.source, &file.ast));
    }
    pairs.push((source, ast));
    crate::resolve::Index::build(&pairs)
}

fn run_typecheck_for_source(
    session: &Session,
    source: &SourceFile,
    ast: &mut crate::ast::File,
) -> Result<()> {
    crate::typecheck::check_file_headers(source, ast).map_err(|err| located_report(source, err))?;
    crate::typecheck::check_file_struct_decls(source, ast)
        .map_err(|err| located_report(source, err))?;

    let index = build_single_source_index(session, source, ast)
        .map_err(|err| located_report(source, err))?;

    let headers = crate::resolve::check_file_headers(source, ast, &index)
        .map_err(|err| located_report(source, err))?;
    crate::resolve::check_file_bodies(source, ast, &index, &headers)
        .map_err(|err| located_report(source, err))?;

    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::from)?;
    env.extend_from_file(source, ast, &index)
        .map_err(|err| located_report(source, err))?;
    if let Some(target_platform) = session.options().target_platform().cloned() {
        env.set_target_platform(target_platform);
    }

    let mut types = crate::ty::TypeStore::new();
    let builtins = types.intern_builtins();
    typecheck_sysroot_overlay_files(session, &index, &env, &mut types, builtins)?;
    run_typecheck_passes(source, ast, &index, &headers, &env, &mut types, builtins)?;
    crate::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::from)?;
    Ok(())
}

fn typecheck_sysroot_overlay_files(
    session: &Session,
    index: &crate::resolve::Index,
    env: &crate::typecheck::TypeEnv,
    types: &mut crate::ty::TypeStore,
    builtins: crate::ty::BuiltinTypes,
) -> Result<()> {
    let Some(overlay_root) = session.options().sysroot_overlay() else {
        return Ok(());
    };
    let overlay_root = overlay_root
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot overlay")?;

    for file in session.sysroot().index_files() {
        if !file.source.path().starts_with(&overlay_root) {
            continue;
        }
        let source = &file.source;
        let mut ast = file.ast.clone();
        crate::typecheck::check_file_headers(source, &ast)
            .map_err(|err| located_report(source, err))?;
        crate::typecheck::check_file_struct_decls(source, &ast)
            .map_err(|err| located_report(source, err))?;
        let headers = crate::resolve::check_file_headers(source, &ast, index)
            .map_err(|err| located_report(source, err))?;
        crate::resolve::check_file_bodies(source, &mut ast, index, &headers)
            .map_err(|err| located_report(source, err))?;
        run_typecheck_passes(source, &ast, index, &headers, env, types, builtins)?;
    }
    Ok(())
}

fn select_project_sources(
    project: &crate::frontend::ProjectInput,
    input_root: &Path,
    source: Option<&Path>,
) -> Result<Vec<usize>> {
    let Some(source) = source else {
        return Ok((0..project.build_closure_sources().len()).collect());
    };
    let raw = if source.is_absolute() {
        source.to_path_buf()
    } else {
        input_root.join(source)
    };
    let selected = raw
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| format!("无法定位 `--source` 源文件：{}", raw.display()))?;
    let Some((index, _)) = project
        .build_closure_sources()
        .iter()
        .enumerate()
        .find(|(_, candidate)| candidate.path() == selected)
    else {
        return Err(miette::miette!(
            "`--source` 不属于该 cone project 的 source graph：{}",
            selected.display()
        ));
    };
    Ok(vec![index])
}

fn run_check_source_project(
    session: &Session,
    project: &crate::frontend::ProjectInput,
    selected: &[usize],
    phase: CheckSourcePhase,
) -> Result<()> {
    if phase == CheckSourcePhase::Parse {
        for index_to_check in selected {
            let source = &project.build_closure_sources()[*index_to_check];
            parse_source(session, source).map_err(|err| located_report(source, err))?;
        }
        return Ok(());
    }

    let mut asts = parse_project_asts(session, project)?;
    let index = build_project_index(project, &asts)?;
    if phase == CheckSourcePhase::Resolve {
        for index_to_check in selected {
            let source = &project.build_closure_sources()[*index_to_check];
            crate::resolve::check_file_bindings(source, &mut asts[*index_to_check], &index)
                .map_err(|err| located_report(source, err))?;
        }
        return Ok(());
    }

    let typed = typecheck_project(session, project, asts, index, selected)?;
    if phase == CheckSourcePhase::Infer {
        for index_to_check in selected {
            infer_project_source(project, &typed, *index_to_check)?;
        }
    }
    Ok(())
}

fn parse_project_asts(
    session: &Session,
    project: &crate::frontend::ProjectInput,
) -> Result<Vec<crate::ast::File>> {
    let mut asts = Vec::with_capacity(project.build_closure_sources().len());
    for source in project.build_closure_sources() {
        asts.push(parse_source(session, source).map_err(|err| located_report(source, err))?);
    }
    Ok(asts)
}

fn build_project_index(
    project: &crate::frontend::ProjectInput,
    asts: &[crate::ast::File],
) -> Result<crate::resolve::Index> {
    let indexed = project
        .build_closure_sources()
        .iter()
        .enumerate()
        .map(|(index, source)| crate::resolve::IndexedFile {
            cone: project.source_cone_id(index),
            cone_kind: project.source_cone_kind(index),
            source,
            file: &asts[index],
        })
        .collect::<Vec<_>>();
    let mut index =
        crate::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::from)?;
    index.set_export_entry_points(project.cone_manifest().export_entry_points.clone());
    if let Some(entry_main) = project.entry_main_fqn() {
        index.set_runtime_entry_point(entry_main.to_owned());
    }
    Ok(index)
}

struct TypecheckedProject {
    asts: Vec<crate::ast::File>,
    index: crate::resolve::Index,
    env: crate::typecheck::TypeEnv,
    types: crate::ty::TypeStore,
}

fn typecheck_project(
    session: &Session,
    project: &crate::frontend::ProjectInput,
    mut asts: Vec<crate::ast::File>,
    index: crate::resolve::Index,
    selected: &[usize],
) -> Result<TypecheckedProject> {
    let sysroot_paths = session
        .sysroot()
        .index_files()
        .map(|file| file.source.path().to_path_buf())
        .collect::<HashSet<_>>();
    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::from)?;
    for (source, ast) in project.build_closure_sources().iter().zip(asts.iter()) {
        if sysroot_paths.contains(source.path()) {
            continue;
        }
        env.extend_from_file(source, ast, &index)
            .map_err(|err| located_report(source, err))?;
    }
    if let Some(target_platform) = session.options().target_platform().cloned() {
        env.set_target_platform(target_platform);
    }

    let mut types = crate::ty::TypeStore::new();
    let builtins = types.intern_builtins();
    for index_to_check in selected {
        let index_to_check = *index_to_check;
        let source = &project.build_closure_sources()[index_to_check];
        let ast = &mut asts[index_to_check];

        crate::typecheck::check_file_headers(source, ast)
            .map_err(|err| located_report(source, err))?;
        crate::typecheck::check_file_struct_decls(source, ast)
            .map_err(|err| located_report(source, err))?;
        let headers = crate::resolve::check_file_headers(source, ast, &index)
            .map_err(|err| located_report(source, err))?;
        crate::resolve::check_file_bodies(source, ast, &index, &headers)
            .map_err(|err| located_report(source, err))?;
        run_typecheck_passes(source, ast, &index, &headers, &env, &mut types, builtins)?;
    }
    crate::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::from)?;

    Ok(TypecheckedProject {
        asts,
        index,
        env,
        types,
    })
}

fn run_typecheck_passes(
    source: &SourceFile,
    ast: &crate::ast::File,
    index: &crate::resolve::Index,
    headers: &crate::resolve::FileHeaders,
    env: &crate::typecheck::TypeEnv,
    types: &mut crate::ty::TypeStore,
    builtins: crate::ty::BuiltinTypes,
) -> Result<()> {
    crate::typecheck::check_file_annotations(
        source,
        ast,
        index,
        &headers.imports,
        env,
        types,
        builtins,
    )
    .map_err(|err| located_report(source, err))?;
    crate::typecheck::check_file_properties(source, ast, index, env)
        .map_err(|err| located_report(source, *err))?;
    crate::typecheck::check_file_inheritance(
        source,
        ast,
        index,
        &headers.imports,
        env,
        types,
        builtins,
    )
    .map_err(|err| located_report(source, err))?;
    crate::typecheck::check_file_interfaces(
        source,
        ast,
        index,
        &headers.imports,
        env,
        types,
        builtins,
    )
    .map_err(|err| located_report(source, err))?;
    crate::typecheck::check_file_override_effects(
        source,
        ast,
        index,
        &headers.imports,
        env,
        types,
        builtins,
    )
    .map_err(|err| located_report(source, *err))?;
    crate::typecheck::check_file_type_refs(
        source,
        ast,
        index,
        &headers.imports,
        env,
        types,
        builtins,
    )
    .map_err(|err| located_report(source, err))?;
    crate::typecheck::check_file_where_clauses(
        source,
        ast,
        index,
        &headers.imports,
        env,
        types,
        builtins,
    )
    .map_err(|err| located_report(source, err))?;
    crate::typecheck::check_file_overload_conflicts(
        source,
        ast,
        index,
        &headers.imports,
        env,
        types,
        builtins,
    )
    .map_err(|err| located_report(source, err))?;
    crate::typecheck::check_file_exprs(source, ast, index, &headers.imports, env, types, builtins)
        .map_err(|err| located_report(source, err))?;
    Ok(())
}

fn infer_project_source(
    project: &crate::frontend::ProjectInput,
    typed: &TypecheckedProject,
    index_to_check: usize,
) -> Result<()> {
    let compilation_unit = project
        .build_closure_sources()
        .iter()
        .zip(typed.asts.iter())
        .collect::<Vec<_>>();
    let source = &project.build_closure_sources()[index_to_check];
    let files_to_lower = [(source, &typed.asts[index_to_check])];
    let lowered = crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
        project.source_cone_info(index_to_check).stable_key.clone(),
        &typed.index,
        &compilation_unit,
        &files_to_lower,
        Some(&typed.env),
        &typed.types,
    )
    .map_err(|err| located_report(source, err))?;
    crate::pipeline::HirStageOutput::new(lowered, source.path())
        .map(|_| ())
        .map_err(|err| located_report(source, err))
}

#[derive(Debug)]
struct LocatedCheckSourceDiagnostic {
    message: String,
    code: Option<String>,
}

impl std::fmt::Display for LocatedCheckSourceDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LocatedCheckSourceDiagnostic {}

impl Diagnostic for LocatedCheckSourceDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.code
            .as_ref()
            .map(|code| Box::new(code.as_str()) as Box<dyn std::fmt::Display + 'a>)
    }
}

fn located_report<E>(source: &SourceFile, error: E) -> miette::Report
where
    E: Diagnostic + std::error::Error + Send + Sync + 'static,
{
    let code = error.code().map(|code| code.to_string());
    let Some((offset, line, col)) = diagnostic_primary_location(source, &error) else {
        return miette::Report::new(error);
    };
    miette::Report::new(LocatedCheckSourceDiagnostic {
        message: format!("{error}\nlocation: {line}:{col}\nspan-offset: {offset}"),
        code,
    })
}

fn diagnostic_primary_location(
    source: &SourceFile,
    diagnostic: &dyn Diagnostic,
) -> Option<(usize, usize, usize)> {
    let mut first = None;
    let mut primary = None;
    if let Some(labels) = diagnostic.labels() {
        for label in labels {
            first.get_or_insert((label.offset(), label.len()));
            if label.primary() {
                primary = Some((label.offset(), label.len()));
                break;
            }
        }
    }
    let (offset, _len) = primary.or(first)?;
    let (line, col) = source.offset_to_line_col(offset).ok()?;
    Some((offset, line, col))
}

pub fn run_dump_ast(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let ast_output = crate::pipeline::load_ast_stage_output_for_dump(&session, &file)
        .map_err(miette::Report::from)?;
    println!("{:#?}", ast_output.ast());
    Ok(())
}

pub fn run_dump_hir(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_hir_stage_output_for_dump(&session, &file)
        .map_err(miette::Report::from)?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_mir(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_direct_style_mir_stage_output_for_dump(&session, &file)
        .map_err(miette::Report::from)?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_ir(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_p4_ready_mir_stage_output_for_dump(&session, &file)
        .map_err(|err| miette::miette!("failed to build P4-ready MIR dump: {err}"))?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_effect_facts(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_effect_facts_stage_output_for_dump(&session, &file)
        .map_err(|err| miette::miette!(err.to_string()))?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_effect_lowered(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let output = crate::pipeline::load_lir_stage_output_for_dump(&session, &file)
        .map_err(|err| miette::miette!(err.to_string()))?;
    print!("{}", output.stable_dump());
    Ok(())
}

pub fn run_dump_rtti(
    input: PathBuf,
    type_name: Option<String>,
    session_options: SessionOptions,
) -> Result<()> {
    let input = canonical_input(input)?;
    let file = SourceFile::load(&input)?;
    let session = Session::with_options(session_options)?;
    let dump = crate::rtti::type_desc::dump_file_type_desc(&session, &file)
        .map_err(miette::Report::from)?;

    if let Some(query) = type_name {
        if let Some(found) = dump.types.iter().find(|t| t.name == query) {
            println!("{}", serde_json::to_string_pretty(found).into_diagnostic()?);
            return Ok(());
        }
        if let Some(found) = dump.interfaces.iter().find(|i| i.name == query) {
            println!("{}", serde_json::to_string_pretty(found).into_diagnostic()?);
            return Ok(());
        }
        enum DumpItem<'a> {
            Type(&'a crate::rtti::type_desc::TypeDesc),
            Interface(&'a crate::rtti::type_desc::InterfaceDesc),
        }
        let mut by_simple: std::collections::BTreeMap<&str, Vec<DumpItem<'_>>> =
            std::collections::BTreeMap::new();
        for ty in &dump.types {
            let simple = ty.name.rsplit('.').next().unwrap_or(ty.name.as_str());
            by_simple
                .entry(simple)
                .or_default()
                .push(DumpItem::Type(ty));
        }
        for iface in &dump.interfaces {
            let simple = iface.name.rsplit('.').next().unwrap_or(iface.name.as_str());
            by_simple
                .entry(simple)
                .or_default()
                .push(DumpItem::Interface(iface));
        }
        let Some(cands) = by_simple.get(query.as_str()) else {
            return Err(miette::miette!("未知类型：{query}"));
        };
        if cands.len() == 1 {
            match cands[0] {
                DumpItem::Type(ty) => {
                    println!("{}", serde_json::to_string_pretty(ty).into_diagnostic()?)
                }
                DumpItem::Interface(iface) => {
                    println!("{}", serde_json::to_string_pretty(iface).into_diagnostic()?)
                }
            }
            return Ok(());
        }
        let names = cands
            .iter()
            .map(|c| match c {
                DumpItem::Type(ty) => ty.name.as_str(),
                DumpItem::Interface(iface) => iface.name.as_str(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(miette::miette!("类型名不唯一：{query}（候选：{names}）"));
    }

    println!("{}", serde_json::to_string_pretty(&dump).into_diagnostic()?);
    Ok(())
}

fn canonical_input(input: PathBuf) -> Result<PathBuf> {
    input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempProject {
        root: PathBuf,
    }

    impl TempProject {
        fn new(label: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "scoopc_check_source_{label}_{}_{}",
                std::process::id(),
                nanos
            ));
            std::fs::create_dir_all(root.join("src")).unwrap();
            Self { root }
        }

        fn write(&self, rel: &str, text: &str) {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, text).unwrap();
        }

        fn write_manifest(&self) {
            self.write(
                "Cone.toml",
                "[cone]\nname = \"sample.check_source\"\nversion = \"0.0.0\"\nkind = \"lib\"\n",
            );
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn check_source_typecheck_source_selection_does_not_check_unselected_bodies() {
        let project = TempProject::new("mixed_typecheck");
        project.write_manifest();
        project.write(
            "src/a_def.scoop",
            r#"
package sample.check_source

import scoop.core.*

class C() {
    private companion object {
        val x: Int = 1
    }
}
"#,
        );
        project.write(
            "src/b_use.scoop",
            r#"
package sample.check_source

import scoop.core.*

val y: Int = C.x
"#,
        );

        run_check_source(
            project.path().to_path_buf(),
            Some(PathBuf::from("src/a_def.scoop")),
            CheckSourcePhase::Typecheck,
            None,
            SessionOptions::new(),
        )
        .expect("selected passing source should not be blocked by an unselected failing body");

        let err = run_check_source(
            project.path().to_path_buf(),
            Some(PathBuf::from("src/b_use.scoop")),
            CheckSourcePhase::Typecheck,
            None,
            SessionOptions::new(),
        )
        .expect_err("selected failing source should still report its diagnostic");
        assert_eq!(
            err.code().map(|code| code.to_string()).as_deref(),
            Some("scoop::resolve::not_visible")
        );
    }
}

pub fn run_dump_stackmaps(input: PathBuf, verify_roots: bool, dump_records: bool) -> Result<()> {
    let input = canonical_input(input)?;
    let bytes = std::fs::read(&input)
        .into_diagnostic()
        .wrap_err_with(|| format!("读取输入文件失败：{}", input.display()))?;
    let obj = object::File::parse(bytes.as_slice())
        .into_diagnostic()
        .wrap_err("解析二进制文件失败（object::File::parse）")?;
    let (section_name, section_bytes) = find_stackmaps_section(&obj).wrap_err_with(|| {
        format!(
            "未找到 stackmap section（期望 `.llvm_stackmaps` / `__llvm_stackmaps`）：{}",
            input.display()
        )
    })?;
    let header = crate::stackmap::StackMapHeader::parse(section_bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("解析 stackmap header 失败（section: {section_name}）"))?;
    println!("stackmaps:");
    println!("section: {section_name}");
    println!("version: {}", header.version);
    println!("functions: {}", header.num_functions);
    println!("constants: {}", header.num_constants);
    println!("records: {}", header.num_records);
    if verify_roots || dump_records {
        let section = crate::stackmap::StackMapSection::parse(section_bytes)
            .into_diagnostic()
            .wrap_err("解析 stackmap section 失败（StackMapSection::parse）")?;
        let symbols = collect_text_symbols(&obj);
        let cfg = roots_contract_config_from_arch(obj.architecture())?;
        if verify_roots {
            section
                .verify_roots_contract(cfg)
                .into_diagnostic()
                .wrap_err("stackmap roots 契约校验失败（--verify-roots）")?;
            println!("verify-roots: ok");
        }
        dump_record_root_slots(&section, &symbols, cfg);
        if dump_records {
            dump_records_locations(&section, &symbols, cfg);
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct TextSymbol {
    addr: u64,
    name: String,
}

fn collect_text_symbols(obj: &object::File<'_>) -> Vec<TextSymbol> {
    let mut out = Vec::new();
    for sym in obj.symbols().chain(obj.dynamic_symbols()) {
        if sym.kind() != SymbolKind::Text || sym.address() == 0 {
            continue;
        }
        let Ok(name) = sym.name() else { continue };
        if !name.trim().is_empty() {
            out.push(TextSymbol {
                addr: sym.address(),
                name: name.to_string(),
            });
        }
    }
    out.sort_by_key(|s| s.addr);
    out
}

fn symbol_name_by_addr(symbols: &[TextSymbol], addr: u64) -> Option<&str> {
    let idx = symbols.binary_search_by_key(&addr, |s| s.addr).ok()?;
    Some(symbols[idx].name.as_str())
}

fn format_function_label(symbols: &[TextSymbol], function_address: u64) -> String {
    if let Some(name) = symbol_name_by_addr(symbols, function_address) {
        format!("{name} (0x{function_address:x})")
    } else {
        format!("0x{function_address:x}")
    }
}

fn format_base_reg_name(
    cfg: crate::stackmap::StackMapRootsContractConfig,
    dwarf_reg: u16,
) -> String {
    if dwarf_reg == cfg.sp_dwarf_reg {
        return format!("SP({dwarf_reg})");
    }
    if cfg.fp_dwarf_reg.is_some_and(|fp| fp == dwarf_reg) {
        return format!("FP({dwarf_reg})");
    }
    format!("reg({dwarf_reg})")
}

fn format_i32_signed_hex(v: i32) -> String {
    if v >= 0 {
        format!("+0x{:x}", v as u32)
    } else {
        format!("-0x{:x}", v.unsigned_abs())
    }
}

fn is_root_slot_location(
    loc: crate::stackmap::StackMapLocation,
    cfg: crate::stackmap::StackMapRootsContractConfig,
) -> bool {
    matches!(
        loc.kind,
        crate::stackmap::StackMapLocationKind::Direct
            | crate::stackmap::StackMapLocationKind::Indirect
    ) && loc.size == cfg.pointer_size
        && (loc.dwarf_reg == cfg.sp_dwarf_reg
            || cfg.fp_dwarf_reg.is_some_and(|fp| fp == loc.dwarf_reg))
}

fn roots_suffix_start(
    rec: &crate::stackmap::StackMapRecord,
    cfg: crate::stackmap::StackMapRootsContractConfig,
) -> usize {
    let mut i = rec.locations.len();
    while i > 0 {
        let idx = i - 1;
        if is_root_slot_location(rec.locations[idx], cfg) {
            i -= 1;
        } else {
            break;
        }
    }
    i
}

fn dump_record_root_slots(
    section: &crate::stackmap::StackMapSection,
    symbols: &[TextSymbol],
    cfg: crate::stackmap::StackMapRootsContractConfig,
) {
    println!();
    println!("root-slots:");
    println!(
        "config: ptr={} sp={} fp={}",
        cfg.pointer_size,
        cfg.sp_dwarf_reg,
        cfg.fp_dwarf_reg
            .map_or("none".to_string(), |v| v.to_string())
    );
    for (record_index, rec) in section.records.iter().enumerate() {
        let ra = rec
            .function_address
            .saturating_add(rec.instruction_offset as u64);
        let roots_start = roots_suffix_start(rec, cfg);
        let roots_len = rec.locations.len().saturating_sub(roots_start);
        let func = format_function_label(symbols, rec.function_address);
        println!(
            "- record[{record_index}] func={func} inst_off=0x{:x} ra=0x{ra:x} patchpoint_id=0x{:x} roots={roots_len}",
            rec.instruction_offset, rec.patchpoint_id
        );
        for pair in 0..(roots_len / 2) {
            let base_i = roots_start + pair * 2;
            let derived_i = base_i + 1;
            let base = rec.locations[base_i];
            let derived = rec.locations[derived_i];
            println!(
                "  pair[{pair}] base loc[{base_i}] kind={:?} base={} off={:+}({}) size={}",
                base.kind,
                format_base_reg_name(cfg, base.dwarf_reg),
                base.offset,
                format_i32_signed_hex(base.offset),
                base.size
            );
            println!(
                "  pair[{pair}] derived loc[{derived_i}] kind={:?} base={} off={:+}({}) size={}",
                derived.kind,
                format_base_reg_name(cfg, derived.dwarf_reg),
                derived.offset,
                format_i32_signed_hex(derived.offset),
                derived.size
            );
        }
    }
}

fn dump_records_locations(
    section: &crate::stackmap::StackMapSection,
    symbols: &[TextSymbol],
    cfg: crate::stackmap::StackMapRootsContractConfig,
) {
    println!();
    println!("records-detail:");
    for (record_index, rec) in section.records.iter().enumerate() {
        let ra = rec
            .function_address
            .saturating_add(rec.instruction_offset as u64);
        let roots_start = roots_suffix_start(rec, cfg);
        let roots_len = rec.locations.len().saturating_sub(roots_start);
        let func = format_function_label(symbols, rec.function_address);
        println!(
            "- record[{record_index}] func={func} inst_off=0x{:x} ra=0x{ra:x} patchpoint_id=0x{:x} locs={} roots_start={roots_start} roots={roots_len}",
            rec.instruction_offset,
            rec.patchpoint_id,
            rec.locations.len()
        );
        for (loc_index, loc) in rec.locations.iter().enumerate() {
            let role = if loc_index >= roots_start {
                "root"
            } else {
                "meta"
            };
            let writable = if is_root_slot_location(*loc, cfg) {
                "writable"
            } else {
                "non-writable"
            };
            println!(
                "  loc[{loc_index}] role={role} {writable} kind={:?} size={} base={} off={:+}({})",
                loc.kind,
                loc.size,
                format_base_reg_name(cfg, loc.dwarf_reg),
                loc.offset,
                format_i32_signed_hex(loc.offset),
            );
        }
    }
}

fn find_stackmaps_section<'data>(obj: &object::File<'data>) -> Result<(&'data str, &'data [u8])> {
    for section in obj.sections() {
        let name = section.name().ok();
        let Some(name) = name else { continue };
        if !(name == ".llvm_stackmaps"
            || name == "__llvm_stackmaps"
            || name.ends_with("llvm_stackmaps"))
        {
            continue;
        }
        let data = section
            .data()
            .into_diagnostic()
            .wrap_err_with(|| format!("读取 section 数据失败：{name}"))?;
        return Ok((name, data));
    }
    Err(miette::miette!("stackmap section not found"))
}

fn roots_contract_config_from_arch(
    arch: Architecture,
) -> Result<crate::stackmap::StackMapRootsContractConfig> {
    match arch {
        Architecture::Aarch64 => Ok(crate::stackmap::StackMapRootsContractConfig {
            pointer_size: 8,
            sp_dwarf_reg: 31,
            fp_dwarf_reg: Some(29),
        }),
        Architecture::X86_64 => Ok(crate::stackmap::StackMapRootsContractConfig {
            pointer_size: 8,
            sp_dwarf_reg: 7,
            fp_dwarf_reg: Some(6),
        }),
        other => Err(miette::miette!(
            "暂不支持的目标架构：{other:?}（--verify-roots 目前仅支持 aarch64/x86_64）"
        )),
    }
}
