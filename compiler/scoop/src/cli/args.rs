use std::ffi::OsString;
use std::path::PathBuf;

use super::doc::DocArgs;
use clap::{Args, Parser, Subcommand, ValueEnum};
use scoop::BuildProfile;

mod forwarding;
pub(super) use forwarding::with_single_file_arguments;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_forwarding;

#[derive(Parser)]
#[command(
    name = "scoop",
    version = scoop_toolchain::VERSION,
    about = "Build, test, run, and link Scoop programs"
)]
pub(super) struct Cli {
    /// Include runtime GC stress support, controlled by environment variables at run time.
    #[arg(long, global = true)]
    pub enable_gc_stress: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub(super) enum Command {
    /// Create a new Cone with a fixed executable or library template.
    New(super::new::NewArgs),
    /// Build a library or executable from a Cone or one source file.
    Build(BuildArgs),
    /// Build an executable and run it with the caller's environment and stdio.
    #[command(
        after_help = "For a .scoop file, the first program positional argument starts forwarding. Use -- before a first program option, such as --help. All later arguments are forwarded unchanged."
    )]
    Run(RunArgs),
    /// Build and run the current manifest Cone’s tests.
    Test(TestArgs),
    /// Link existing artifacts without compiling sources.
    Link(LinkArgs),
    /// Generate an offline API documentation site from manifest Cones.
    Doc(DocArgs),
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
    /// Compiler optimization and output profile; release enables optimized code.
    #[arg(long, conflicts_with = "release")]
    pub profile: Option<BuildProfile>,
    #[arg(long, conflicts_with = "profile")]
    pub release: bool,
    #[arg(long)]
    pub target_dir: Option<PathBuf>,
    #[arg(long)]
    pub scoopc: Option<PathBuf>,
    /// Register a host method-body generator; may be repeated for distinct names.
    #[arg(long = "generator", value_name = "NAME=PATH")]
    pub generators: Vec<OsString>,
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
    /// Maximum concurrent compiler processes; defaults to available CPUs.
    #[arg(short = 'j', long)]
    pub jobs: Option<std::num::NonZeroUsize>,
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
pub(super) struct TestArgs {
    #[command(flatten)]
    pub options: BuildOptions,
    /// Build and link without running the test executable.
    #[arg(long, conflicts_with = "list")]
    pub no_run: bool,
    /// List compiled test containers and cases without running them.
    #[arg(long)]
    pub list: bool,
    /// Match a substring of the complete test name, including its container.
    #[arg(long)]
    pub filter: Option<String>,
    /// Select only tests marked Ignore.
    #[arg(long)]
    pub ignored: bool,
    /// Show test stdout/stderr as it is produced instead of capturing it.
    #[arg(long, conflicts_with_all = ["list", "no_run"])]
    pub nocapture: bool,
}

#[derive(Args)]
pub(super) struct LinkArgs {
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
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
    pub runtime_objects: Option<PathBuf>,
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
            Self::New(_) => MessageFormat::Human,
            Self::Build(args) => args.options.message_format,
            Self::Run(args) => args.options.message_format,
            Self::Test(args) => args.options.message_format,
            Self::Link(args) => args.message_format,
            Self::Doc(args) => args.message_format,
        }
    }
}
