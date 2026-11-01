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
mod tests_external;
#[cfg(test)]
mod tests_forwarding;

#[derive(Parser)]
#[command(
    name = if cfg!(windows) { "sco" } else { "scoop" },
    version = scoop_toolchain::VERSION,
    about = "Build, check, format, test, run, and link Scoop programs",
    after_help = "Other commands run <SYSROOT>/commands/scoop-<command> with all following arguments unchanged. Put Scoop options before an external command."
)]
pub(super) struct Cli {
    /// Scoop installation root; defaults to SCOOP_SYSROOT or the installation directory.
    #[arg(long, global = true)]
    pub sysroot: Option<PathBuf>,
    /// Explicit package manager; defaults to scoop-pm beside this executable.
    #[arg(long)]
    pub package_manager: Option<PathBuf>,
    /// Include runtime GC stress support, controlled by environment variables at run time.
    #[arg(long, global = true)]
    pub enable_gc_stress: bool,
    /// Include startup timing and metadata-count diagnostics in the linked runtime.
    #[arg(long, global = true)]
    pub enable_startup_stats: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub(super) enum Command {
    /// Run the separate Scoop package manager with all following arguments unchanged.
    Pm(super::package_manager::PmArgs),
    /// Create a new Cone with a fixed executable or library template.
    New(super::new::NewArgs),
    /// Build a library or executable from a Cone or one source file.
    Build(BuildArgs),
    /// Remove the entire target directory, including all profiles and build caches.
    Clean(super::clean::CleanArgs),
    /// Check a Cone or source file without producing a root artifact.
    Check(super::check::CheckArgs),
    /// Format source files, or check their formatting without writing.
    Fmt(super::format::FormatArgs),
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
    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(Clone, Copy, Default, ValueEnum)]
pub(super) enum MessageFormat {
    #[default]
    Human,
    Json,
}

#[derive(Args)]
pub(super) struct BuildOptions {
    #[command(flatten)]
    pub packages: super::package_manager::PackageOptions,
    /// Cone directory, Cone.toml, or one .scoop file; defaults to ./Cone.toml.
    pub root_input: Option<PathBuf>,
    #[arg(long)]
    pub target: Option<String>,
    #[command(flatten)]
    pub native: NativeOptions,
    /// Compiler optimization and output profile; release enables optimized code.
    #[arg(long, conflicts_with = "release")]
    pub profile: Option<BuildProfile>,
    #[arg(long, conflicts_with = "profile")]
    pub release: bool,
    /// Native source debug information: none, line-tables or full; debug defaults to full.
    #[arg(long)]
    pub debug_info: Option<scoop_lir::DebugInfoLevel>,
    #[arg(long)]
    pub target_dir: Option<PathBuf>,
    #[arg(long)]
    pub scoopc: Option<PathBuf>,
    /// Register a host method-body generator; may be repeated for distinct names.
    #[arg(long = "generator", value_name = "NAME=PATH")]
    pub generators: Vec<OsString>,
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
    /// Maximum concurrent compiler workers across Cones and code generation; defaults to available CPUs.
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
    /// Publish the root .slib without preparing runtime or linking an executable.
    #[arg(long)]
    pub emit_slib: bool,
    /// Compile the root test artifact; requires --emit-slib.
    #[arg(long, requires = "emit_slib")]
    pub tests: bool,
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
    Optimizations,
    Lir,
    Symbols,
    All,
}
impl Emit {
    pub fn stages(self) -> scoop_protocol::StageDumpSet {
        use scoop_protocol::{StageDumpKindV1 as Stage, StageDumpSet as Set};
        match self {
            Self::Ast => Set::one(Stage::Ast),
            Self::Hir => Set::one(Stage::Hir),
            Self::Mir => Set::one(Stage::Mir),
            Self::Optimizations => Set::one(Stage::Optimizations),
            Self::Lir => Set::one(Stage::Lir),
            Self::Symbols => Set::one(Stage::Symbols),
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
    /// Native C compiler driver for the selected target.
    #[arg(long)]
    pub cc: Option<PathBuf>,
    /// Native C++ compiler driver; otherwise use the paired driver.
    #[arg(long)]
    pub cxx: Option<PathBuf>,
    /// Final Windows linker executable (lld-link, link.exe, or MinGW LLD).
    #[arg(long)]
    pub linker: Option<PathBuf>,
    /// Target C development sysroot, separate from the Scoop sysroot.
    #[arg(long)]
    pub native_sysroot: Option<PathBuf>,
    /// LLVM unwind headers and archive prefix for the selected target.
    #[arg(long)]
    pub unwind_prefix: Option<PathBuf>,
    /// Executable link mode; Linux gnu defaults to dynamic and musl to static.
    #[arg(long, value_enum)]
    pub link_mode: Option<LinkMode>,
    /// Keep ordinary executable symbols; debug builds already retain them.
    #[arg(long)]
    pub keep_symbols: bool,
    /// Produce a .dSYM bundle or .debug file from existing object debug information.
    #[arg(long)]
    pub emit_debug_file: bool,
    /// Capture exception/trap stacks; debug defaults to true, release and link to false.
    #[arg(long, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
    pub enable_stack_trace: Option<bool>,
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
            Self::New(_) | Self::External(_) | Self::Pm(_) => MessageFormat::Human,
            Self::Build(args) => args.options.message_format,
            Self::Run(args) => args.options.message_format,
            Self::Test(args) => args.options.message_format,
            Self::Link(args) => args.message_format,
            Self::Doc(args) => args.message_format,
            Self::Check(args) => args.message_format,
            Self::Clean(args) => args.message_format,
            Self::Fmt(args) => args.message_format,
        }
    }
}
