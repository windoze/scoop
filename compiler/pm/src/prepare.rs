//! Project the selected portable graph onto local, fully bound compiler inputs.

use std::{collections::BTreeMap, path::Path};

use scoop_manifest::{DependencyCoordinateKey as Key, ParsedConeManifest};
use scoop_protocol::{
    CompilationPurpose, HostPathCarrier,
    package::{
        ExpandedConeInputV1, PACKAGE_PROTOCOL_VERSION, PackageMemberSelectionV1, PrepareRequestV1,
        PreparedConeV1, PreparedNodeV1, PreparedResolutionV1, PreparedWorkspaceV1,
    },
};

use crate::{
    MemberSelection, PmError, PmResult,
    catalog::PackageInput,
    lockfile::Purpose,
    resolve::{ResolveRequest, ResolvedRoot, ResolvedWorkspace},
};

mod record;

pub fn prepare(request: PrepareRequestV1) -> PmResult<PreparedWorkspaceV1> {
    request.validate().map_err(PmError::new)?;
    let cwd = path(&request.cwd)?;
    let explicit_cache = request.package_cache.as_ref().map(path).transpose()?;
    let cache = crate::cache::PackageCache::resolve(&cwd, explicit_cache.as_deref())?;
    let record = record::PreparedRecord::new(&cache, &request)?;
    let _guard = cache.lock("prepared", record.key())?;
    if let Some(prepared) = record.load()? {
        return Ok(prepared);
    }
    let request = ResolveRequest {
        cwd,
        input: request.input.as_ref().map(path).transpose()?,
        members: match request.members {
            PackageMemberSelectionV1::Default => MemberSelection::Default,
            PackageMemberSelectionV1::All => MemberSelection::All,
            PackageMemberSelectionV1::Packages { keys } => MemberSelection::Packages(keys),
        },
        purpose: match request.purpose {
            CompilationPurpose::Production => Purpose::Production,
            CompilationPurpose::Test => Purpose::Test,
        },
        target: request.target,
        sysroot: path(&request.sysroot)?,
        artifact_roots: request
            .artifact_roots
            .iter()
            .map(path)
            .collect::<PmResult<_>>()?,
        package_cache: Some(cache.root().to_owned()),
        lockfile: request.lockfile.as_ref().map(path).transpose()?,
        locked: request.locked,
        offline: request.offline,
        update: None,
        precise: None,
    };
    let resolved = crate::resolve::resolve(request)?;
    let inputs = record::Inputs::capture(&resolved)?;
    let prepared = project(resolved)?;
    record.publish(&prepared, &inputs)?;
    Ok(prepared)
}

pub fn prepare_files(request: &Path, response: &Path) -> PmResult<()> {
    let bytes = std::fs::read(request).map_err(|error| PmError::io(request, error))?;
    let request =
        serde_json::from_slice(&bytes).map_err(|error| PmError::new(error.to_string()))?;
    let prepared = prepare(request)?;
    let bytes = serde_json::to_vec(&prepared).map_err(|error| PmError::new(error.to_string()))?;
    crate::cache::atomic_write(response, &bytes)
}

fn project(resolved: ResolvedWorkspace) -> PmResult<PreparedWorkspaceV1> {
    Ok(PreparedWorkspaceV1 {
        version: PACKAGE_PROTOCOL_VERSION,
        workspace: host(&resolved.workspace.root)?,
        lockfile: host(&resolved.lockfile)?,
        resolutions: resolved
            .roots
            .into_iter()
            .map(project_root)
            .collect::<PmResult<_>>()?,
    })
}

fn project_root(root: ResolvedRoot) -> PmResult<PreparedResolutionV1> {
    let bindings: BTreeMap<_, _> = root
        .solution
        .packages
        .iter()
        .map(|(key, candidate)| (key.clone(), candidate.coordinate.clone()))
        .collect();
    let mut nodes = Vec::new();
    for node in &root.resolution.nodes {
        let input = &root.inputs[&Key::from_coordinate(&node.coordinate)];
        let cone = match input {
            PackageInput::Source(member) => {
                let include_tests = root.resolution.purpose == Purpose::Test
                    && node.coordinate == root.resolution.root_coordinate;
                let manifest = ParsedConeManifest::bind_package(
                    &member.document,
                    node.coordinate.clone(),
                    &bindings,
                    include_tests,
                )
                .map_err(|error| PmError::new(format!("{}: {error}", member.manifest.display())))?;
                PreparedConeV1::Source {
                    config: Box::new(ExpandedConeInputV1 {
                        root: host(&member.directory)?,
                        manifest_path: host(&member.manifest)?,
                        manifest_text: member.manifest_text.clone(),
                        manifest,
                        metadata: member.cone.metadata.clone(),
                    }),
                }
            }
            PackageInput::Prebuilt { path, summary } => PreparedConeV1::Prebuilt {
                artifact: host(path)?,
                summary: summary.clone(),
            },
        };
        nodes.push(PreparedNodeV1 {
            cone,
            dependencies: node.dependencies.clone(),
            test_dependencies: node.test_dependencies.clone(),
        });
    }
    Ok(PreparedResolutionV1 {
        root: root.resolution.root,
        coordinate: root.resolution.root_coordinate,
        purpose: match root.resolution.purpose {
            Purpose::Production => CompilationPurpose::Production,
            Purpose::Test => CompilationPurpose::Test,
        },
        target: root.resolution.target,
        nodes,
    })
}

fn path(carrier: &HostPathCarrier) -> PmResult<std::path::PathBuf> {
    carrier
        .to_path_buf()
        .map_err(|error| PmError::new(error.to_string()))
}

fn host(path: &Path) -> PmResult<HostPathCarrier> {
    HostPathCarrier::from_path(path).map_err(|error| PmError::new(error.to_string()))
}
