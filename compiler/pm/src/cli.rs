use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use scoop_manifest::DependencyCoordinateKey;
use scoop_pm::{MemberSelection, PmError, PmResult, Workspace};
use scoop_protocol::HostPathCarrier;
use scoop_protocol::package::{PACKAGE_PROTOCOL_VERSION, PackageMemberV1, PackageMembersV1};

mod deps;
mod edit;
mod metadata;
mod resolve;

#[derive(Parser)]
#[command(name = "scoop-pm", version = scoop_toolchain::VERSION, about = "Manage Scoop workspaces and packages")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create an empty virtual workspace.
    New {
        #[arg(long, value_name = "DIRECTORY")]
        workspace: PathBuf,
    },
    /// List local members and expanded source selections without resolving dependencies.
    Members(LocalMembersArgs),
    /// Add or replace one dependency declaration without resolving or fetching.
    Add(edit::AddArgs),
    /// Remove one dependency declaration without updating Cone.lock.
    Remove(edit::EditArgs),
    /// Read exact dependency graphs from Cone.lock without fetching or updating.
    Deps(deps::DepsArgs),
    /// Return the locked local project model as versioned JSON without updating Cone.lock.
    Metadata(metadata::MetadataArgs),
    /// Resolve selected dependency graphs and prepare their source packages.
    Resolve(resolve::ResolveArgs),
    /// Fetch the exact source packages from existing selected lock records.
    Fetch(resolve::ResolveArgs),
    /// Update compatible registry versions or Git references.
    Update(resolve::UpdateArgs),
    /// Prepare exact local inputs for the build driver using the versioned JSON protocol.
    Prepare {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        response: PathBuf,
    },
}

#[derive(Args)]
struct MembersArgs {
    /// Workspace/Cone directory or Cone.toml; otherwise discover from the current directory.
    input: Option<PathBuf>,
    #[arg(long, conflicts_with = "package")]
    workspace: bool,
    #[arg(short = 'p', long = "package", value_name = "GROUP:NAME")]
    package: Vec<DependencyCoordinateKey>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct LocalMembersArgs {
    #[command(flatten)]
    members: MembersArgs,
    /// Read unsaved local Cone.toml texts as versioned JSON from stdin.
    #[arg(long)]
    overlays_stdin: bool,
}

pub fn run() -> Result<(), (PmError, bool)> {
    let cli = Cli::parse();
    let machine = matches!(&cli.command, Command::Prepare { .. })
        || matches!(&cli.command, Command::Members(args) if args.members.json)
        || matches!(&cli.command, Command::Metadata(args) if args.json());
    execute(cli).map_err(|error| (error, machine))
}

fn execute(cli: Cli) -> PmResult<()> {
    match cli.command {
        Command::New { workspace } => create_workspace(&workspace),
        Command::Members(args) => members(args),
        Command::Add(args) => edit::add(args),
        Command::Remove(args) => edit::remove(args),
        Command::Deps(args) => deps::execute(args),
        Command::Metadata(args) => metadata::execute(args),
        Command::Resolve(args) => resolve::execute(args, false, None, None),
        Command::Fetch(args) => resolve::execute(args, true, None, None),
        Command::Update(args) => resolve::update(args),
        Command::Prepare { request, response } => {
            scoop_pm::prepare::prepare_files(&request, &response)
        }
    }
}

fn create_workspace(directory: &std::path::Path) -> PmResult<()> {
    let cwd = std::env::current_dir().map_err(|error| PmError::new(error.to_string()))?;
    let directory = cwd.join(directory);
    if directory
        .try_exists()
        .map_err(|error| PmError::new(error.to_string()))?
    {
        return Err(PmError::new(format!(
            "refusing to overwrite {}",
            directory.display()
        )));
    }
    if let Some(parent) = directory.parent() {
        std::fs::create_dir_all(parent).map_err(|error| PmError::new(error.to_string()))?;
    }
    std::fs::create_dir(&directory).map_err(|error| PmError::new(error.to_string()))?;
    let manifest = directory.join("Cone.toml");
    std::fs::write(&manifest, "schema = 1\n\n[workspace]\nmembers = []\n")
        .map_err(|error| PmError::new(format!("{}: {error}", manifest.display())))?;
    println!("Created workspace {}", directory.display());
    Ok(())
}

fn members(args: LocalMembersArgs) -> PmResult<()> {
    let cwd = std::env::current_dir().map_err(|error| PmError::new(error.to_string()))?;
    let overlays = metadata::read_overlays(args.overlays_stdin)?;
    let args = args.members;
    let workspace = Workspace::discover_with_overlays(&cwd, args.input.as_deref(), overlays)?;
    let selection = if args.workspace {
        MemberSelection::All
    } else if args.package.is_empty() {
        MemberSelection::Default
    } else {
        MemberSelection::Packages(args.package)
    };
    let selected: Vec<_> = workspace
        .select(&selection)?
        .iter()
        .map(|member| member.manifest.clone())
        .collect();
    if args.json {
        let host_path = |path: &std::path::Path| {
            HostPathCarrier::from_path(path).map_err(|error| PmError::new(error.to_string()))
        };
        let members = workspace
            .members
            .values()
            .map(|member| {
                Ok(PackageMemberV1 {
                    coordinate: member.cone.coordinate.clone(),
                    kind: member.cone.kind,
                    path: member.relative_path.clone(),
                    directory: host_path(&member.directory)?,
                    manifest: host_path(&member.manifest)?,
                    selected: selected.contains(&member.manifest),
                    sources: member.cone.sources.clone(),
                    test_sources: member.cone.test_sources.clone(),
                })
            })
            .collect::<PmResult<Vec<_>>>()?;
        let result = PackageMembersV1 {
            format: PACKAGE_PROTOCOL_VERSION,
            root: host_path(&workspace.root)?,
            members,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&result)
                .map_err(|error| PmError::new(error.to_string()))?
        );
    } else {
        for member in workspace.members.values() {
            println!(
                "{} {} ({})",
                if selected.contains(&member.manifest) {
                    "*"
                } else {
                    " "
                },
                member.cone.coordinate,
                member.relative_path
            );
        }
    }
    Ok(())
}
