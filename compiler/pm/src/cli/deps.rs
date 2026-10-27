use scoop_pm::lockfile::{Lockfile, Purpose};

use super::*;

#[derive(Args)]
pub(super) struct DepsArgs {
    #[command(flatten)]
    members: MembersArgs,
    #[arg(long)]
    lockfile: Option<PathBuf>,
    #[arg(long, default_value = "production")]
    purpose: Purpose,
    #[arg(long)]
    target: Option<String>,
}

pub(super) fn execute(args: DepsArgs) -> PmResult<()> {
    let cwd = std::env::current_dir().map_err(|error| PmError::new(error.to_string()))?;
    let workspace = Workspace::discover(&cwd, args.members.input.as_deref())?;
    let path = args
        .lockfile
        .map(|path| cwd.join(path))
        .unwrap_or_else(|| workspace.root.join("Cone.lock"));
    let lock = Lockfile::read(&path)?.ok_or_else(|| {
        PmError::new(format!(
            "{} does not exist; resolve dependencies first",
            path.display()
        ))
    })?;
    let target = match args.target {
        Some(target) => target,
        None => scoop_toolchain::host_target_triple()
            .map_err(|error| PmError::new(error.to_string()))?
            .into(),
    };
    let selection = if args.members.workspace {
        MemberSelection::All
    } else if args.members.package.is_empty() {
        MemberSelection::Default
    } else {
        MemberSelection::Packages(args.members.package)
    };
    let resolutions = workspace
        .select(&selection)?
        .into_iter()
        .map(|member| {
            lock.resolution(&member.relative_path, args.purpose, &target)
                .cloned()
                .ok_or_else(|| {
                    PmError::new(format!(
                        "no locked resolution for {} / {} / {target}; resolve dependencies first",
                        member.cone.coordinate, args.purpose
                    ))
                })
        })
        .collect::<PmResult<Vec<_>>>()?;
    if args.members.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&Lockfile {
                format: 1,
                resolutions
            })
            .map_err(|error| PmError::new(error.to_string()))?
        );
    } else {
        for resolution in resolutions {
            println!(
                "{} [{} / {}]",
                resolution.root_coordinate, resolution.purpose, resolution.target
            );
            for node in resolution.nodes {
                println!("  {} ({})", node.coordinate, node.source);
                for dependency in node.dependencies {
                    println!("    -> {dependency}");
                }
                for dependency in node.test_dependencies {
                    println!("    -> {dependency} [test]");
                }
            }
        }
    }
    Ok(())
}
