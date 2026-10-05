use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use scoop::BuildProfile;

#[derive(Parser)]
#[command(name = "scoop", version, about = "Build, run, and link Scoop programs")]
pub(super) struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub(super) enum Command {
    /// Build a library or executable from a Cone or one source file.
    Build(BuildArgs),
    /// Build an executable and run it with the caller's environment and stdio.
    Run(RunArgs),
    /// Link existing artifacts without compiling sources.
    Link(LinkArgs),
}

#[derive(Clone, Copy, Default, ValueEnum)]
pub(super) enum MessageFormat {
    #[default]
    Human,
    Json,
}

#[derive(Args)]
pub(super) struct BuildOptions {
    /// Cone directory, Cone.toml, or one .scoop file; defaults to ./Cone.toml.
    pub root_input: Option<PathBuf>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub sysroot: Option<PathBuf>,
    #[command(flatten)]
    pub native: NativeOptions,
    /// Output profile; debug and release currently use the same compiler settings.
    #[arg(long, conflicts_with = "release")]
    pub profile: Option<BuildProfile>,
    #[arg(long, conflicts_with = "profile")]
    pub release: bool,
    #[arg(long)]
    pub target_dir: Option<PathBuf>,
    #[arg(long)]
    pub scoopc: Option<PathBuf>,
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
    #[arg(long)]
    pub cone_path: Vec<PathBuf>,
    #[arg(long, conflicts_with = "runtime_objects")]
    pub runtime_root: Option<PathBuf>,
    #[arg(long, conflicts_with = "runtime_root")]
    pub runtime_objects: Option<PathBuf>,
    #[arg(long)]
    pub library_path: Vec<PathBuf>,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

#[derive(Args)]
pub(super) struct BuildArgs {
    #[command(flatten)]
    pub options: BuildOptions,
    #[arg(short = 'o')]
    pub output: Option<PathBuf>,
    #[arg(long, requires = "dump_dir")]
    pub emit: Option<Emit>,
    #[arg(long, requires = "emit")]
    pub dump_dir: Option<PathBuf>,
    #[arg(long, requires = "emit")]
    pub dump_scope: Option<DumpScope>,
}

#[derive(Clone, Copy, ValueEnum)]
pub(super) enum Emit {
    Ast,
    Hir,
    Mir,
    Lir,
    All,
}
impl Emit {
    pub fn stages(self) -> scoop_protocol::StageDumpSet {
        use scoop_protocol::{StageDumpKindV1 as Stage, StageDumpSet as Set};
        match self {
            Self::Ast => Set::one(Stage::Ast),
            Self::Hir => Set::one(Stage::Hir),
            Self::Mir => Set::one(Stage::Mir),
            Self::Lir => Set::one(Stage::Lir),
            Self::All => Set::all(),
        }
    }
}
#[derive(Clone, Copy, Default, ValueEnum)]
pub(super) enum DumpScope {
    #[default]
    Root,
    Sources,
}

#[derive(Args)]
pub(super) struct RunArgs {
    #[command(flatten)]
    pub options: BuildOptions,
    #[arg(last = true, allow_hyphen_values = true)]
    pub program_args: Vec<OsString>,
}

#[derive(Args)]
pub(super) struct LinkArgs {
    #[arg(long)]
    pub root_slib: PathBuf,
    #[arg(long)]
    pub dependency_slib: Vec<PathBuf>,
    #[arg(long)]
    pub cone_path: Vec<PathBuf>,
    #[arg(long)]
    pub sysroot: Option<PathBuf>,
    #[arg(long)]
    pub target: Option<String>,
    #[command(flatten)]
    pub native: NativeOptions,
    #[arg(long)]
    pub runtime_objects: PathBuf,
    #[arg(long)]
    pub library_path: Vec<PathBuf>,
    #[arg(short = 'o')]
    pub output: PathBuf,
    #[arg(long)]
    pub dump_plan: bool,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

#[derive(Args)]
pub(super) struct NativeOptions {
    /// Linux C compiler driver.
    #[arg(long)]
    pub cc: Option<PathBuf>,
    /// Target C development sysroot, separate from the Scoop sysroot.
    #[arg(long)]
    pub native_sysroot: Option<PathBuf>,
    /// LLVM unwind headers and archive prefix for the selected target.
    #[arg(long)]
    pub unwind_prefix: Option<PathBuf>,
    /// Executable link mode; Linux gnu defaults to dynamic and musl to static.
    #[arg(long, value_enum)]
    pub link_mode: Option<LinkMode>,
}

#[derive(Clone, Copy, ValueEnum)]
pub(super) enum LinkMode {
    Static,
    Dynamic,
}

impl From<LinkMode> for scoop_toolchain::LinkMode {
    fn from(value: LinkMode) -> Self {
        match value {
            LinkMode::Static => Self::Static,
            LinkMode::Dynamic => Self::Dynamic,
        }
    }
}

impl Command {
    pub fn message_format(&self) -> MessageFormat {
        match self {
            Self::Build(args) => args.options.message_format,
            Self::Run(args) => args.options.message_format,
            Self::Link(args) => args.message_format,
        }
    }
}
