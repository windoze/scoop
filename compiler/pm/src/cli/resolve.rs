use scoop_pm::{
    lockfile::{Lockfile, Purpose},
    resolve::{ResolveRequest, resolve},
};

use super::*;

#[derive(Args)]
pub(super) struct ResolveArgs {
    input: Option<PathBuf>,
    #[arg(long, conflicts_with = "member")]
    workspace: bool,
    #[arg(short = 'p', long, value_name = "GROUP:NAME")]
    member: Vec<DependencyCoordinateKey>,
    #[arg(long, default_value = "production")]
    purpose: Purpose,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    sysroot: Option<PathBuf>,
    #[arg(long, value_name = "DIRECTORY")]
    cone_path: Vec<PathBuf>,
    #[arg(long)]
    package_cache_dir: Option<PathBuf>,
    #[arg(long)]
    lockfile: Option<PathBuf>,
    #[arg(long)]
    locked: bool,
    #[arg(long)]
    offline: bool,
    #[arg(long)]
    frozen: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
pub(super) struct UpdateArgs {
    #[command(flatten)]
    resolve: ResolveArgs,
    #[arg(long, value_name = "GROUP:NAME")]
    package: Vec<DependencyCoordinateKey>,
    #[arg(long, requires = "package")]
    precise: Option<String>,
}

pub(super) fn update(args: UpdateArgs) -> PmResult<()> {
    let precise = match args.precise {
        Some(value) if args.package.len() == 1 => Some((args.package[0].clone(), value)),
        Some(_) => return Err(PmError::new("--precise requires exactly one --package")),
        None => None,
    };
    execute(args.resolve, false, Some(args.package), precise)
}

pub(super) fn execute(
    args: ResolveArgs,
    fetch: bool,
    update: Option<Vec<DependencyCoordinateKey>>,
    precise: Option<(DependencyCoordinateKey, String)>,
) -> PmResult<()> {
    let cwd = std::env::current_dir().map_err(|error| PmError::new(error.to_string()))?;
    let members = if args.workspace {
        MemberSelection::All
    } else if args.member.is_empty() {
        MemberSelection::Default
    } else {
        MemberSelection::Packages(args.member)
    };
    let request = ResolveRequest {
        cwd,
        input: args.input,
        members,
        purpose: args.purpose,
        target: args
            .target
            .map(Ok)
            .unwrap_or_else(|| scoop_toolchain::host_target_triple().map(str::to_owned))
            .map_err(|error| PmError::new(error.to_string()))?,
        sysroot: scoop_toolchain::configured_sysroot_root(args.sysroot)
            .map_err(|error| PmError::new(error.to_string()))?,
        artifact_roots: args.cone_path,
        package_cache: args.package_cache_dir,
        lockfile: args.lockfile,
        locked: fetch || args.locked || args.frozen,
        offline: args.offline || args.frozen,
        update,
        precise,
    };
    let result = resolve(request)?;
    if args.json {
        let lock = Lockfile {
            format: 1,
            resolutions: result
                .roots
                .into_iter()
                .map(|root| root.resolution)
                .collect(),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&lock).map_err(|error| PmError::new(error.to_string()))?
        );
    } else {
        for root in result.roots {
            println!(
                "{} [{} / {}]: {} packages",
                root.resolution.root_coordinate,
                root.resolution.purpose,
                root.resolution.target,
                root.resolution.nodes.len()
            );
            for change in root.changes {
                println!("  {change}");
            }
        }
        println!(
            "{} {}",
            if result.changed { "Updated" } else { "Using" },
            result.lockfile.display()
        );
    }
    Ok(())
}
