use scoop_protocol::{
    CompilationPurpose,
    package::{PackageMemberSelectionV1, PrepareRequestV1},
};

use super::*;

#[derive(Args)]
pub(super) struct MetadataArgs {
    #[command(flatten)]
    members: MembersArgs,
    #[arg(long)]
    lockfile: Option<PathBuf>,
    #[arg(long, default_value = "production")]
    purpose: scoop_pm::lockfile::Purpose,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    sysroot: Option<PathBuf>,
    #[arg(long, value_name = "DIRECTORY")]
    cone_path: Vec<PathBuf>,
    #[arg(long)]
    package_cache_dir: Option<PathBuf>,
    /// Only use existing source directories; do not download or unpack dependencies.
    #[arg(long)]
    offline: bool,
}

impl MetadataArgs {
    pub(super) fn json(&self) -> bool {
        self.members.json
    }
}

pub(super) fn execute(args: MetadataArgs) -> PmResult<()> {
    let cwd = std::env::current_dir().map_err(|error| PmError::new(error.to_string()))?;
    let host = |path: PathBuf| {
        HostPathCarrier::from_path(&cwd.join(path)).map_err(|error| PmError::new(error.to_string()))
    };
    let members = if args.members.workspace {
        PackageMemberSelectionV1::All
    } else if args.members.package.is_empty() {
        PackageMemberSelectionV1::Default
    } else {
        PackageMemberSelectionV1::Packages {
            keys: args.members.package,
        }
    };
    let request = PrepareRequestV1 {
        version: PACKAGE_PROTOCOL_VERSION,
        cwd: host(cwd.clone())?,
        input: args.members.input.map(host).transpose()?,
        members,
        purpose: match args.purpose {
            scoop_pm::lockfile::Purpose::Production => CompilationPurpose::Production,
            scoop_pm::lockfile::Purpose::Test => CompilationPurpose::Test,
        },
        target: args
            .target
            .map(Ok)
            .unwrap_or_else(|| scoop_toolchain::host_target_triple().map(str::to_owned))
            .map_err(|error| PmError::new(error.to_string()))?,
        sysroot: host(
            scoop_toolchain::configured_sysroot_root(args.sysroot)
                .map_err(|error| PmError::new(error.to_string()))?,
        )?,
        artifact_roots: args
            .cone_path
            .into_iter()
            .map(host)
            .collect::<PmResult<_>>()?,
        package_cache: args.package_cache_dir.map(host).transpose()?,
        lockfile: args.lockfile.map(host).transpose()?,
        locked: true,
        offline: args.offline,
    };
    let result = scoop_pm::prepare::metadata(request)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|error| PmError::new(error.to_string()))?
    );
    Ok(())
}
