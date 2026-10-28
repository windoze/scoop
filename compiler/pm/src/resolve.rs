use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use scoop_manifest::DependencyCoordinateKey as Key;

use crate::{
    MemberSelection, PmError, PmResult, Workspace,
    cache::PackageCache,
    catalog::{Catalog, CatalogOptions, PackageInput},
    lockfile::{LockTransaction, LockedResolution, Purpose},
    resolver::{self, ResolvePolicy, Solution},
};

mod changes;
pub(crate) mod config;
mod solving;

pub struct ResolveRequest {
    pub cwd: PathBuf,
    pub input: Option<PathBuf>,
    pub members: MemberSelection,
    pub purpose: Purpose,
    pub target: String,
    pub sysroot: PathBuf,
    pub artifact_roots: Vec<PathBuf>,
    pub package_cache: Option<PathBuf>,
    pub lockfile: Option<PathBuf>,
    pub locked: bool,
    pub offline: bool,
    pub update: Option<Vec<Key>>,
    pub precise: Option<(Key, String)>,
}

#[derive(Debug)]
pub struct ResolvedRoot {
    pub resolution: LockedResolution,
    pub solution: Solution,
    pub inputs: BTreeMap<Key, PackageInput>,
    pub changes: Vec<String>,
}

#[derive(Debug)]
pub struct ResolvedWorkspace {
    pub workspace: Workspace,
    pub lockfile: PathBuf,
    pub roots: Vec<ResolvedRoot>,
    pub changed: bool,
    pub(crate) inputs: BTreeMap<PathBuf, crate::cache::FileStamp>,
    pub(crate) missing_inputs: BTreeSet<PathBuf>,
}

pub fn resolve(request: ResolveRequest) -> PmResult<ResolvedWorkspace> {
    resolve_with_sources(request, false)
}

/// Inspect current declarations against a fixed lock. Offline inspection only
/// uses source directories that have already been materialized.
pub fn inspect(mut request: ResolveRequest) -> PmResult<ResolvedWorkspace> {
    request.locked = true;
    let local_sources_only = request.offline;
    resolve_with_sources(request, local_sources_only)
}

pub(crate) fn resolve_with_sources(
    request: ResolveRequest,
    local_sources_only: bool,
) -> PmResult<ResolvedWorkspace> {
    if request.locked && request.update.is_some() {
        return Err(PmError::new(
            "update cannot be combined with --locked/--frozen",
        ));
    }
    let target = scoop_toolchain::select_target(&request.target)
        .map_err(|error| PmError::new(error.to_string()))?;
    let target_name = target.target().contract().canonical_triple();
    let cache = PackageCache::resolve(&request.cwd, request.package_cache.as_deref())?;
    let workspace =
        Workspace::discover_cached(&request.cwd, request.input.as_deref(), &cache, None)?;
    let lockfile = request
        .lockfile
        .as_ref()
        .map(|path| request.cwd.join(path))
        .unwrap_or_else(|| workspace.root.join("Cone.lock"));
    let transaction = LockTransaction::open(&lockfile, &cache)?;
    let lockfile = transaction.path().to_owned();
    let selected = workspace.select(&request.members)?;
    if selected.is_empty() {
        return Err(PmError::new("no workspace members selected"));
    }
    let config = config::registry(&cache)?;
    let mut observed = BTreeMap::new();
    let mut missing_inputs = BTreeSet::new();
    if let Some((path, stamp)) = config.input {
        match stamp {
            Some(stamp) => {
                observed.insert(path, stamp);
            }
            None => {
                missing_inputs.insert(path);
            }
        }
    }
    let options = CatalogOptions {
        lock_directory: lockfile.parent().expect("lock path has parent").to_owned(),
        target,
        sysroot: request.cwd.join(&request.sysroot),
        artifact_roots: request
            .artifact_roots
            .iter()
            .map(|path| request.cwd.join(path))
            .collect(),
        default_registry: config.registry,
        offline: request.offline,
        local_sources_only,
    };
    let mut roots = Vec::new();
    let mut catalog = Catalog::new(&cache, &workspace, &ResolvePolicy::default(), options);
    for member in selected {
        let previous =
            transaction
                .current()
                .resolution(&member.relative_path, request.purpose, target_name);
        if request.locked && previous.is_none() {
            return Err(PmError::new(format!(
                "no locked resolution for {} / {} / {target_name}; run scoop pm resolve",
                member.cone.coordinate, request.purpose
            )));
        }
        let mut policy = ResolvePolicy {
            locked: previous
                .into_iter()
                .flat_map(|graph| &graph.nodes)
                .map(|node| (Key::from_coordinate(&node.coordinate), node.clone()))
                .collect(),
            strict: request.locked,
            update: request.update.clone(),
            precise: request.precise.clone(),
        };
        catalog.update_policy(&policy);
        let root = catalog.root(member, request.purpose)?;
        let solution = solving::solve(root, &mut policy, &mut catalog).map_err(|mut error| {
            if request.locked && !error.resolution_conflicts.is_empty() {
                error.message.push_str("\nCone.lock cannot satisfy the current requirements; run scoop pm resolve");
            }
            if request.offline && !error.resolution_conflicts.is_empty() {
                error.message.push_str("\n--offline used only local metadata and Git objects; remote versions were not checked");
            }
            error
        })?;
        let resolution = solution.lock(
            member.relative_path.clone(),
            request.purpose,
            target_name.into(),
        )?;
        if request.locked && previous != Some(&resolution) {
            return Err(PmError::new(format!(
                "Cone.lock no longer matches {}; run scoop pm resolve without --locked",
                member.cone.coordinate
            )));
        }
        catalog.materialize(&solution)?;
        observed.extend(
            catalog
                .input_stamps()
                .iter()
                .map(|(path, stamp)| (path.clone(), stamp.clone())),
        );
        missing_inputs.extend(catalog.missing_inputs().iter().cloned());
        let inputs = solution
            .packages
            .iter()
            .map(|(key, candidate)| (key.clone(), catalog.input(candidate).clone()))
            .collect();
        roots.push(ResolvedRoot {
            changes: changes::describe(previous, &resolution, &solution),
            resolution,
            solution,
            inputs,
        });
    }
    for key in request.update.iter().flatten() {
        if !roots
            .iter()
            .any(|root| root.solution.packages.contains_key(key))
        {
            return Err(PmError::new(format!(
                "package {key} is not in any selected dependency graph"
            )));
        }
    }
    for (path, stamp) in &observed {
        stamp.unchanged(path)?;
    }
    for path in &missing_inputs {
        if crate::cache::FileStamp::optional(path)?.is_some() {
            return Err(PmError::new(format!(
                "input appeared while preparing packages: {}; retry the command",
                path.display()
            )));
        }
    }
    let changed = roots.iter().any(|root| {
        transaction.current().resolution(
            &root.resolution.root,
            root.resolution.purpose,
            target_name,
        ) != Some(&root.resolution)
    });
    if !request.locked {
        let members: BTreeSet<_> = workspace
            .members
            .values()
            .map(|member| member.relative_path.clone())
            .collect();
        transaction.publish(
            roots.iter().map(|root| root.resolution.clone()).collect(),
            &members,
        )?;
    }
    observed.insert(lockfile.clone(), crate::cache::FileStamp::read(&lockfile)?);
    Ok(ResolvedWorkspace {
        workspace,
        lockfile,
        roots,
        changed,
        inputs: observed,
        missing_inputs,
    })
}
