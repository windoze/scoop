use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_manifest::{
    CurrentConeInput, classify_current_cone_operand, select_manifest_format_sources,
};

use super::args::MessageFormat;
use super::commands::{absolute, config};
use super::presentation::Reporter;

mod diagnostics;
#[cfg(test)]
mod tests;

#[derive(Args)]
pub(super) struct FormatArgs {
    /// Cone directories, Cone.toml files, or .scoop files; defaults to ./Cone.toml.
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

pub(super) fn execute(args: FormatArgs, reporter: &Reporter) -> BuildResult<bool> {
    let cwd = std::env::current_dir().map_err(config)?;
    let inputs = if args.inputs.is_empty() {
        vec![PathBuf::from("Cone.toml")]
    } else {
        args.inputs
    };
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    let mut success = true;
    for input in inputs {
        match absolute(&cwd, input).and_then(|path| expand(&path)) {
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

fn expand(path: &Path) -> BuildResult<Vec<PathBuf>> {
    match classify_current_cone_operand(path)
        .map_err(|error| failure("SCOOP_FMT_INPUT", error.path(), &error))?
    {
        CurrentConeInput::SingleFile { source } => Ok(vec![source.resolved_path().to_owned()]),
        CurrentConeInput::Manifest { root } => {
            let manifest = scoop_toolchain::load_cone_manifest(&root).map_err(|error| {
                let mut failure = failure("SCOOP_FMT_INPUT", error.path(), &error);
                if let scoop_manifest::ManifestRootErrorKind::Parse(parsed) = error.kind()
                    && let Some(span) = parsed.span()
                {
                    failure.location = scoop::BuildFailureLocation::Host {
                        path: error.path().to_owned(),
                        span: Some(span.range()),
                    };
                }
                failure
            })?;
            select_manifest_format_sources(&manifest)
                .map_err(|error| failure("SCOOP_FMT_INPUT", error.path(), &error))
        }
    }
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
