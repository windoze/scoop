//! An ordinary executable Cone calls the fixture's published check function.
//! The C glue only names actual image and root records from the resulting Link data.

use super::*;
use scoop_identity::{DeclarationName, SourceDeclarationKey, StrongCallableDefinitionOwner};

pub(super) fn build_runner(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess],
    closure: &scoop_slib::LinkSymbolsReplayedCrossConeLayoutClosure,
    directory: &Path,
) -> SingleConeProductionSuccess {
    let (sections, _) = closure
        .artifact(closure.physical_imports().current())
        .unwrap();
    let entries = sections
        .lir_cross_cone_bridge()
        .exports()
        .iter()
        .filter_map(|export| {
            let StrongCallableDefinitionOwner::Function(id) = export.target() else {
                return None;
            };
            let key = sections
                .identity_graph()
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap();
            matches!(key.name(), DeclarationName::Named(name) if name.as_str() == "check").then(
                || {
                    key.package()
                        .segments()
                        .iter()
                        .map(|segment| segment.as_str())
                        .collect::<Vec<_>>()
                        .join(".")
                },
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "the fixture publishes one check function");
    let import = if entries[0].is_empty() {
        String::new()
    } else {
        format!("import {}.check\n", entries[0])
    };
    let source = format!(
        "{import}fun main() {{ if (check() != 42) {{ throw IllegalStateException() }} }}\n"
    );
    let root = directory.join("runner");
    write_manifest_cone(
        &root,
        "dev.example",
        "runtime-fixture",
        "executable",
        &source,
    );
    let current = artifacts.last().unwrap().artifact();
    let coordinate = current.summary().coordinate();
    let manifest = std::fs::read_to_string(root.join("Cone.toml")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        format!(
            "{manifest}[dependencies]\n\"{}:{}\" = \"{}\"\n",
            coordinate.group(),
            coordinate.name(),
            coordinate.version()
        ),
    )
    .unwrap();
    let support = artifacts[1..artifacts.len() - 1]
        .iter()
        .map(|artifact| HostArtifactLocator::new(artifact.artifact().path()).unwrap())
        .collect();
    let request = SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(&root),
        },
        ExplicitDependencyInputs::new(
            vec![HostArtifactLocator::new(current.path()).unwrap()],
            support,
        ),
        TrustedCoreInput::Artifact(
            HostArtifactLocator::new(artifacts[0].artifact().path()).unwrap(),
        ),
        target.clone(),
        SlibOutputDestination::new(directory.join("runner.slib")).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    let result = request.build_and_publish().unwrap();
    std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
    result
}
