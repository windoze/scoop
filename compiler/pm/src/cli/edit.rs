use std::path::Path;

use scoop_pm::lockfile::Purpose;
use toml_edit::{InlineTable, Value};

use super::*;

#[derive(Args)]
pub(super) struct EditArgs {
    /// Dependency to add, replace, or remove.
    dependency: DependencyCoordinateKey,
    /// Workspace/Cone directory or Cone.toml; otherwise discover from cwd.
    input: Option<PathBuf>,
    #[arg(short = 'p', long = "package", value_name = "GROUP:NAME")]
    package: Option<DependencyCoordinateKey>,
    #[arg(long, default_value = "production")]
    purpose: Purpose,
}

#[derive(Args)]
pub(super) struct AddArgs {
    #[command(flatten)]
    edit: EditArgs,
    #[arg(long)]
    version: Option<String>,
    #[arg(long, group = "source")]
    path: Option<PathBuf>,
    #[arg(long, group = "source")]
    artifact: Option<PathBuf>,
    #[arg(long, group = "source")]
    git: Option<String>,
    #[arg(long, group = "source")]
    github: Option<String>,
    #[arg(long, group = "source")]
    registry: Option<String>,
    #[arg(long, group = "git_reference")]
    branch: Option<String>,
    #[arg(long, group = "git_reference")]
    tag: Option<String>,
    #[arg(long, group = "git_reference")]
    rev: Option<String>,
    #[arg(long)]
    subdir: Option<String>,
    /// Inherit this key from workspace.dependencies.
    #[arg(long, group = "source")]
    inherit: bool,
}

pub(super) fn add(args: AddArgs) -> PmResult<()> {
    let (cwd, workspace) = discover(&args.edit)?;
    let member = selected(&workspace, &args.edit)?;
    let value = declaration(&args, &cwd, &member.directory)?;
    scoop_pm::edit::dependency(
        &workspace,
        member,
        args.edit.purpose,
        &args.edit.dependency,
        Some(value),
    )?;
    println!(
        "Added {} to {}",
        args.edit.dependency,
        member.manifest.display()
    );
    Ok(())
}

pub(super) fn remove(args: EditArgs) -> PmResult<()> {
    let (_, workspace) = discover(&args)?;
    let member = selected(&workspace, &args)?;
    scoop_pm::edit::dependency(&workspace, member, args.purpose, &args.dependency, None)?;
    println!(
        "Removed {} from {}",
        args.dependency,
        member.manifest.display()
    );
    Ok(())
}

fn discover(args: &EditArgs) -> PmResult<(PathBuf, Workspace)> {
    let cwd = std::env::current_dir().map_err(|error| PmError::new(error.to_string()))?;
    let workspace = Workspace::discover(&cwd, args.input.as_deref())?;
    Ok((cwd, workspace))
}

fn selected<'a>(
    workspace: &'a Workspace,
    args: &EditArgs,
) -> PmResult<&'a scoop_pm::WorkspaceMember> {
    let selection = args
        .package
        .clone()
        .map(|key| MemberSelection::Packages(vec![key]))
        .unwrap_or_default();
    let members = workspace.select(&selection)?;
    match members.as_slice() {
        [member] => Ok(member),
        _ => Err(PmError::new(
            "add/remove requires exactly one selected Cone; use -p group:name",
        )),
    }
}

fn declaration(args: &AddArgs, cwd: &Path, member: &Path) -> PmResult<Value> {
    let mut table = InlineTable::new();
    for (key, value) in [
        ("version", &args.version),
        ("github", &args.github),
        ("registry", &args.registry),
        ("branch", &args.branch),
        ("tag", &args.tag),
        ("rev", &args.rev),
        ("subdir", &args.subdir),
    ] {
        if let Some(value) = value {
            table.insert(key, Value::from(value.as_str()));
        }
    }
    for (key, path) in [("path", &args.path), ("artifact", &args.artifact)] {
        if let Some(path) = path {
            table.insert(key, Value::from(relative(cwd, member, path)?));
        }
    }
    if let Some(git) = &args.git {
        let repository = if git.is_empty() || git.contains(':') {
            git.clone()
        } else {
            relative(cwd, member, Path::new(git))?
        };
        table.insert("git", Value::from(repository));
    }
    if args.inherit {
        table.insert("workspace", Value::from(true));
    }
    if table.len() == 1
        && let Some(version) = table.get("version")
    {
        return Ok(version.clone());
    }
    table.fmt();
    Ok(Value::InlineTable(table))
}

fn relative(cwd: &Path, member: &Path, path: &Path) -> PmResult<String> {
    if path.as_os_str().is_empty() {
        return Err(PmError::new("dependency path must not be empty"));
    }
    let absolute = cwd.join(path);
    let relative = pathdiff::diff_paths(&absolute, member).unwrap_or(absolute);
    relative
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| PmError::new("manifest dependency path must be UTF-8"))
}
