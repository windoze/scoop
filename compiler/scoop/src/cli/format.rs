use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_manifest::{CurrentConeInput, classify_current_cone_operand, select_format_sources};

use super::args::MessageFormat;
use super::commands::{absolute, config};
use super::package_manager::{self, MemberOptions};
use super::presentation::Reporter;

mod diagnostics;
#[cfg(test)]
mod tests;

#[derive(Args)]
pub(super) struct FormatArgs {
    #[command(flatten)]
    members: MemberOptions,
    /// Cone directories, Cone.toml files, or .scoop files; otherwise discover local members.
    inputs: Vec<PathBuf>,
    /// Report formatting differences without writing files.
    #[arg(long)]
    check: bool,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FileStatus {
    Unchanged,
    Formatted,
    NeedsFormat,
    Failed,
}

impl FileStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Formatted => "formatted",
            Self::NeedsFormat => "needs-format",
            Self::Failed => "failed",
        }
    }
}

pub(super) struct FileResult {
    pub path: PathBuf,
    pub status: FileStatus,
}

pub(super) fn execute(
    args: FormatArgs,
    package_manager: Option<PathBuf>,
    reporter: &Reporter,
) -> BuildResult<bool> {
    let cwd = std::env::current_dir().map_err(config)?;
    if args.members.explicit() && args.inputs.len() > 1 {
        return Err(config(
            "workspace member selection accepts at most one workspace input",
        ));
    }
    let inputs = if args.inputs.is_empty() {
        vec![None]
    } else {
        args.inputs.into_iter().map(Some).collect()
    };
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    let mut success = true;
    for input in inputs {
        match expand(&cwd, input, &args.members, package_manager.clone()) {
            Ok(files) => {
                selected.extend(files.into_iter().filter(|path| seen.insert(path.clone())))
            }
            Err(error) => {
                reporter.failure(&error).map_err(config)?;
                success = false;
            }
        }
    }
    let mut files = Vec::new();
    for path in selected {
        scoop_process::finish_interruption();
        let status = match format_file(&path, args.check, reporter) {
            Ok(status) => status,
            Err(error) => {
                reporter.failure(&error).map_err(config)?;
                FileStatus::Failed
            }
        };
        success &= !matches!(status, FileStatus::NeedsFormat | FileStatus::Failed);
        files.push(FileResult { path, status });
    }
    reporter
        .formatted(args.check, success, &files)
        .map_err(config)?;
    Ok(success)
}

fn expand(
    cwd: &Path,
    input: Option<PathBuf>,
    options: &MemberOptions,
    executable: Option<PathBuf>,
) -> BuildResult<Vec<PathBuf>> {
    if let Some(path) = &input
        && path.extension().is_some_and(|ext| ext == "scoop")
    {
        let path = absolute(cwd, path.clone())?;
        if let CurrentConeInput::SingleFile { source } = classify_current_cone_operand(&path)
            .map_err(|error| failure("SCOOP_FMT_INPUT", error.path(), &error))?
        {
            if options.explicit() {
                return Err(config(
                    "workspace member selection requires a manifest input",
                ));
            }
            return Ok(vec![source.resolved_path().to_owned()]);
        }
    }
    let workspace = package_manager::members(cwd, input, options, executable)?;
    let mut sources = Vec::new();
    for member in workspace
        .members
        .into_iter()
        .filter(|member| member.selected)
    {
        sources.extend(
            select_format_sources(
                &member.directory.to_path_buf().map_err(config)?,
                &member.sources,
                &member.test_sources,
            )
            .map_err(|error| failure("SCOOP_FMT_INPUT", error.path(), &error))?,
        );
    }
    Ok(sources)
}

fn format_file(path: &Path, check: bool, reporter: &Reporter) -> BuildResult<FileStatus> {
    let source =
        std::fs::read_to_string(path).map_err(|error| failure("SCOOP_FMT_IO", path, error))?;
    let parsed = scoop_parser::parse_recovering(&source)
        .map_err(|error| failure("SCOOP_FMT_PARSE", path, error))?;
    let diagnostics = diagnostics::convert(path, parsed.diagnostics())?;
    reporter
        .host_diagnostics(&diagnostics, path, &source)
        .map_err(config)?;
    if parsed.ast().is_none() {
        return Ok(FileStatus::Failed);
    }
    let formatted = scoop_fmt::format(&parsed.syntax());
    if formatted == source {
        return Ok(FileStatus::Unchanged);
    }
    if check {
        return Ok(FileStatus::NeedsFormat);
    }
    write_file(path, &formatted).map_err(|error| failure("SCOOP_FMT_IO", path, error))?;
    Ok(FileStatus::Formatted)
}

fn write_file(path: &Path, text: &str) -> std::io::Result<()> {
    let permissions = std::fs::metadata(path)?.permissions();
    let parent = path.parent().expect("selected sources have absolute paths");
    let mut temporary = tempfile::Builder::new()
        .prefix(".scoop-fmt-")
        .tempfile_in(parent)
        .map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("cannot create formatted output: {}", error.kind()),
            )
        })?;
    temporary.write_all(text.as_bytes())?;
    temporary.flush()?;
    temporary.as_file().set_permissions(permissions)?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn failure(code: &str, path: &Path, error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool(code, BuildFailurePhase::Request, error).at_host(path)
}
