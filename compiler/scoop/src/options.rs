//! Shared build option declarations and replay for the driver and PGO command.
use crate::BuildProfile;
use clap::{Args, ValueEnum};
use scoop_manifest::DependencyCoordinateKey;
use scoop_protocol::package::PackageMemberSelectionV1;
use std::ffi::OsString;
use std::path::PathBuf;

mod native;
mod paths;
mod replay;
pub use paths::CommandArgument;
pub use replay::BuildInvocation;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageFormat {
    #[default]
    Human,
    Json,
}

#[derive(Args, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BuildOptions {
    #[command(flatten)]
    #[serde(flatten)]
    pub packages: PackageOptions,
    /// Cone directory, Cone.toml, or one .scoop file; defaults to ./Cone.toml.
    #[serde(with = "paths::optional")]
    pub root_input: Option<PathBuf>,
    #[arg(long)]
    pub target: Option<String>,
    #[command(flatten)]
    #[serde(flatten)]
    pub native: NativeOptions,
    /// Build profile: debug, release, or profiling (release optimization with runtime counters).
    #[arg(long, conflicts_with = "release")]
    pub profile: Option<BuildProfile>,
    #[arg(long, conflicts_with = "profile")]
    pub release: bool,
    /// Native debug information: debug defaults to full, profiling to line-tables, release to none.
    #[arg(long)]
    #[serde(with = "paths::debug_info")]
    pub debug_info: Option<scoop_lir::DebugInfoLevel>,
    /// Optional cross-Cone IR templates: off, default or full.
    #[arg(long, default_value = "default")]
    #[serde(with = "paths::export_ir")]
    pub export_ir: scoop_protocol::IrTemplateExport,
    /// Use a Scoop profile index to guide release optimization.
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub profile_use: Option<PathBuf>,
    /// Fail before publication unless an optimization consumes matching feedback.
    #[arg(long, requires = "profile_use")]
    pub profile_use_required: bool,
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub target_dir: Option<PathBuf>,
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub scoopc: Option<PathBuf>,
    /// Register a host method-body generator; may be repeated for distinct names.
    #[arg(long = "generator", value_name = "NAME=PATH")]
    #[serde(with = "paths::arguments")]
    pub generators: Vec<OsString>,
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub cache_dir: Option<PathBuf>,
    /// Maximum concurrent compiler workers across Cones and code generation; defaults to available CPUs.
    #[arg(short = 'j', long)]
    pub jobs: Option<std::num::NonZeroUsize>,
    #[arg(long)]
    #[serde(with = "paths::multiple")]
    pub cone_path: Vec<PathBuf>,
    /// Override the target system library directory for sysroot maintenance.
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub system_library_root: Option<PathBuf>,
    #[arg(long, conflicts_with = "runtime_objects")]
    #[serde(with = "paths::optional")]
    pub runtime_root: Option<PathBuf>,
    #[arg(long, conflicts_with = "runtime_root")]
    #[serde(with = "paths::optional")]
    pub runtime_objects: Option<PathBuf>,
    #[arg(long)]
    #[serde(with = "paths::multiple")]
    pub library_path: Vec<PathBuf>,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

#[derive(Args, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NativeOptions {
    /// Native C compiler driver for the selected target.
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub cc: Option<PathBuf>,
    /// Native C++ compiler driver; otherwise use the paired driver.
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub cxx: Option<PathBuf>,
    /// Darwin LLD or Windows linker executable (lld-link, link.exe, or MinGW LLD).
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub linker: Option<PathBuf>,
    /// Target C development sysroot, separate from the Scoop sysroot.
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub native_sysroot: Option<PathBuf>,
    /// LLVM unwind headers and archive prefix for the selected target.
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub unwind_prefix: Option<PathBuf>,
    /// Executable link mode; Linux gnu defaults to dynamic and musl to static.
    #[arg(long, value_enum)]
    pub link_mode: Option<LinkMode>,
    /// Darwin native-app install name.
    #[arg(long)]
    pub install_name: Option<String>,
    /// ELF native-app SONAME.
    #[arg(long)]
    pub soname: Option<String>,
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

#[derive(Clone, Copy, Debug, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkMode {
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

#[derive(Args, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PackageOptions {
    #[command(flatten)]
    #[serde(flatten)]
    pub members: MemberOptions,
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub lockfile: Option<PathBuf>,
    /// Require an existing dependency selection without changing Cone.lock.
    #[arg(long)]
    pub locked: bool,
    /// Do not access the network.
    #[arg(long)]
    pub offline: bool,
    /// Require both --locked and --offline.
    #[arg(long)]
    pub frozen: bool,
    /// Shared Git and registry source cache; defaults to ~/.scoop/packages.
    #[arg(long)]
    #[serde(with = "paths::optional")]
    pub package_cache: Option<PathBuf>,
}

impl PackageOptions {
    pub fn selects_members(&self) -> bool {
        self.members.explicit()
    }
}

#[derive(Args, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MemberOptions {
    /// Select all members of the workspace.
    #[arg(long, conflicts_with = "package")]
    pub workspace: bool,
    /// Select a workspace member; may be repeated.
    #[arg(short = 'p', long = "package", value_name = "GROUP:NAME")]
    pub package: Vec<DependencyCoordinateKey>,
}

impl MemberOptions {
    pub fn explicit(&self) -> bool {
        self.workspace || !self.package.is_empty()
    }

    pub fn selection(&self) -> PackageMemberSelectionV1 {
        if self.workspace {
            PackageMemberSelectionV1::All
        } else if self.package.is_empty() {
            PackageMemberSelectionV1::Default
        } else {
            PackageMemberSelectionV1::Packages {
                keys: self.package.clone(),
            }
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Emit {
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
pub enum DumpScope {
    #[default]
    Root,
    Sources,
}
