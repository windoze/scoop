use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::ConeIdentity;
use scoop_manifest::LoadedConeManifest;
use scoop_slib::{
    ConeKind, ConeSourceForm, DecodedSlibEnvelope, PublishableArtifactValidationError,
    PublishableSingleConeArtifact, SlibClosureDecodeMeterV1, SlibClosureDecodePurposeV1,
    SlibClosureResourceErrorV1, SlibClosureResourceKindV1, probe_prebuilt_manifest_summary,
    validate_self_describing_publishable_single_cone_artifact,
};
use scoop_toolchain::ResolvedTargetProfile;
use scoop_wire::sha256;

use super::{
    DependencyValidationResult, ExplicitDependencyArtifactInput, ExplicitDependencyRole,
    ExplicitDependencyValidationError, LoadedExplicitDependencyInputs, NonCoreDependencyInput,
};
use crate::{ValidatedExplicitDependencyInputSet, ValidatedTrustedCoreArtifact};

impl LoadedExplicitDependencyInputs {
    pub(crate) fn validate(
        &self,
        manifest: Option<&LoadedConeManifest>,
        current_identity: ConeIdentity,
        trusted_core: &ValidatedTrustedCoreArtifact<'_>,
        target: &ResolvedTargetProfile,
    ) -> DependencyValidationResult<ValidatedExplicitDependencyInputSet> {
        self.validate_inner(manifest, current_identity, trusted_core, target, None)
    }

    pub(crate) fn validate_metered(
        &self,
        manifest: Option<&LoadedConeManifest>,
        current_identity: ConeIdentity,
        trusted_core: &ValidatedTrustedCoreArtifact<'_>,
        target: &ResolvedTargetProfile,
        meter: &mut SlibClosureDecodeMeterV1,
    ) -> DependencyValidationResult<ValidatedExplicitDependencyInputSet> {
        self.validate_inner(
            manifest,
            current_identity,
            trusted_core,
            target,
            Some(meter),
        )
    }

    fn validate_inner(
        &self,
        manifest: Option<&LoadedConeManifest>,
        current_identity: ConeIdentity,
        trusted_core: &ValidatedTrustedCoreArtifact<'_>,
        target: &ResolvedTargetProfile,
        mut meter: Option<&mut SlibClosureDecodeMeterV1>,
    ) -> DependencyValidationResult<ValidatedExplicitDependencyInputSet> {
        if let Some(meter) = meter.as_deref_mut() {
            let dependency_nodes = u64::try_from(self.artifacts.len()).map_err(|_| {
                Box::new(ExplicitDependencyValidationError::Resource(
                    SlibClosureResourceErrorV1::Overflow {
                        resource: SlibClosureResourceKindV1::ConeNodes,
                    },
                ))
            })?;
            let nodes = dependency_nodes.checked_add(2).ok_or_else(|| {
                Box::new(ExplicitDependencyValidationError::Resource(
                    SlibClosureResourceErrorV1::Overflow {
                        resource: SlibClosureResourceKindV1::ConeNodes,
                    },
                ))
            })?;
            meter
                .charge_graph(nodes, 0, 1)
                .map_err(|source| Box::new(ExplicitDependencyValidationError::Resource(source)))?;
        }
        let mut nodes = BTreeMap::<ConeIdentity, ValidatedDependencyNode>::new();
        let mut group_names = BTreeMap::<(String, String), (String, ConeIdentity)>::new();
        if let Some(manifest) = manifest {
            let coordinate = manifest.parsed().semantic().coordinate();
            group_names.insert(
                (coordinate.group().to_owned(), coordinate.name().to_owned()),
                (coordinate.version().to_owned(), current_identity),
            );
        }

        for loaded in &self.artifacts {
            let snapshot = sha256(&loaded.bytes);
            if let Some(meter) = meter.as_deref_mut() {
                let summary = probe_prebuilt_manifest_summary(
                    &loaded.bytes,
                    self.limits,
                    target.lir_target_selection(),
                )
                .map_err(|source| {
                    Box::new(ExplicitDependencyValidationError::Summary {
                        input: loaded.input.clone(),
                        source: Box::new(source),
                    })
                })?;
                meter
                    .observe_artifact_snapshot(&summary, snapshot)
                    .map_err(|source| {
                        Box::new(ExplicitDependencyValidationError::Resource(source))
                    })?;
                meter
                    .charge_artifact_decode(
                        SlibClosureDecodePurposeV1::GraphSummary,
                        summary.artifact_fingerprint(),
                        snapshot,
                        summary.decode_usage(),
                    )
                    .map_err(|source| {
                        Box::new(ExplicitDependencyValidationError::Resource(source))
                    })?;
            }
            let graph = DecodedSlibEnvelope::open(
                &loaded.bytes,
                self.limits,
                target.lir_target_selection(),
            )
            .map_err(|source| ExplicitDependencyValidationError::Artifact {
                input: loaded.input.clone(),
                source: Box::new(PublishableArtifactValidationError::CompileEnvelope(
                    Box::new(source),
                )),
            })?
            .validate_graph()
            .map_err(|source| ExplicitDependencyValidationError::Artifact {
                input: loaded.input.clone(),
                source: Box::new(PublishableArtifactValidationError::CompileGraph(Box::new(
                    source,
                ))),
            })?;
            if graph.identity() == ConeIdentity::CORE {
                return Err(ExplicitDependencyValidationError::ReservedCoreArtifact {
                    input: loaded.input.clone(),
                }
                .into());
            }
            if graph.identity() == current_identity {
                return Err(ExplicitDependencyValidationError::CurrentConeArtifact {
                    input: loaded.input.clone(),
                    identity: current_identity,
                }
                .into());
            }

            let artifact = validate_self_describing_publishable_single_cone_artifact(
                &loaded.bytes,
                self.limits,
                target.lir_target_selection(),
                trusted_core.defined_symbols(),
                target.c_bridge_toolchain().profile(),
            )
            .map_err(|source| ExplicitDependencyValidationError::Artifact {
                input: loaded.input.clone(),
                source: Box::new(source),
            })?;
            if let Some(meter) = meter.as_deref_mut() {
                meter
                    .charge_artifact_decode(
                        SlibClosureDecodePurposeV1::Compile,
                        artifact.artifact_fingerprint(),
                        snapshot,
                        artifact.compile_summary().decode_usage(),
                    )
                    .map_err(|source| {
                        Box::new(ExplicitDependencyValidationError::Resource(source))
                    })?;
                meter
                    .charge_artifact_decode(
                        SlibClosureDecodePurposeV1::Link,
                        artifact.artifact_fingerprint(),
                        snapshot,
                        artifact.link_summary().decode_usage(),
                    )
                    .map_err(|source| {
                        Box::new(ExplicitDependencyValidationError::Resource(source))
                    })?;
            }
            validate_artifact_shape(&loaded.input, &artifact, current_identity)?;

            let identity = artifact.identity();
            if let Some(previous) = nodes.get(&identity) {
                return Err(ExplicitDependencyValidationError::DuplicateIdentity {
                    identity,
                    first: previous.input.clone(),
                    second: loaded.input.clone(),
                    same_fingerprint: previous.artifact.artifact_fingerprint()
                        == artifact.artifact_fingerprint(),
                }
                .into());
            }

            let coordinate = artifact.coordinate();
            let group_name = (coordinate.group().to_owned(), coordinate.name().to_owned());
            if let Some((version, other_identity)) = group_names.get(&group_name)
                && version != coordinate.version()
            {
                return Err(ExplicitDependencyValidationError::MultipleVersions {
                    group: coordinate.group().to_owned(),
                    name: coordinate.name().to_owned(),
                    first_version: version.clone(),
                    first_identity: *other_identity,
                    second_version: coordinate.version().to_owned(),
                    second_identity: identity,
                }
                .into());
            }
            group_names.insert(
                group_name,
                (coordinate.version().to_owned(), artifact.identity()),
            );
            nodes.insert(
                identity,
                ValidatedDependencyNode {
                    input: loaded.input.clone(),
                    artifact,
                },
            );
        }

        validate_manifest_direct_set(manifest, &nodes)?;
        validate_dependency_records(current_identity, trusted_core, &nodes)?;
        let dependency_depth = validate_acyclic(&nodes)?;
        validate_support_closure(&nodes)?;
        if let Some(meter) = meter {
            charge_graph_edges_and_depth(meter, manifest, &nodes, dependency_depth)?;
        }

        let capability = manifest
            .map(manifest_capability_input)
            .transpose()?
            .flatten()
            .or_else(|| {
                self.artifacts
                    .first()
                    .map(|loaded| loaded.input.capability_input())
            });
        match capability {
            Some(input) => {
                Err(ExplicitDependencyValidationError::CapabilityUnavailable(input).into())
            }
            None => Ok(ValidatedExplicitDependencyInputSet { _core_only: () }),
        }
    }

    pub(crate) fn validate_bootstrap_empty(
        &self,
    ) -> DependencyValidationResult<ValidatedExplicitDependencyInputSet> {
        if let Some(loaded) = self.artifacts.first() {
            return Err(ExplicitDependencyValidationError::BootstrapArtifact {
                input: loaded.input.clone(),
            }
            .into());
        }
        Ok(ValidatedExplicitDependencyInputSet { _core_only: () })
    }

    pub(crate) fn validate_bootstrap_empty_metered(
        &self,
        meter: &mut SlibClosureDecodeMeterV1,
    ) -> DependencyValidationResult<ValidatedExplicitDependencyInputSet> {
        let dependency_nodes = u64::try_from(self.artifacts.len()).map_err(|_| {
            Box::new(ExplicitDependencyValidationError::Resource(
                SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::ConeNodes,
                },
            ))
        })?;
        let nodes = dependency_nodes.checked_add(1).ok_or_else(|| {
            Box::new(ExplicitDependencyValidationError::Resource(
                SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::ConeNodes,
                },
            ))
        })?;
        meter
            .charge_graph(nodes, 0, 1)
            .map_err(|source| Box::new(ExplicitDependencyValidationError::Resource(source)))?;
        self.validate_bootstrap_empty()
    }
}

fn charge_graph_edges_and_depth(
    meter: &mut SlibClosureDecodeMeterV1,
    manifest: Option<&LoadedConeManifest>,
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode>,
    dependency_depth: u64,
) -> DependencyValidationResult<()> {
    let artifact_edges = nodes.values().try_fold(0_u64, |total, node| {
        let count = u64::try_from(node.artifact.direct_dependencies().len()).map_err(|_| {
            Box::new(ExplicitDependencyValidationError::Resource(
                SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::DependencyEdges,
                },
            ))
        })?;
        total.checked_add(count).ok_or_else(|| {
            Box::new(ExplicitDependencyValidationError::Resource(
                SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::DependencyEdges,
                },
            ))
        })
    })?;
    let declared = manifest
        .map(|manifest| manifest.parsed().semantic().dependency_iter().count())
        .unwrap_or(0);
    let current_edges = u64::try_from(declared)
        .ok()
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| {
            Box::new(ExplicitDependencyValidationError::Resource(
                SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::DependencyEdges,
                },
            ))
        })?;
    let edges = artifact_edges.checked_add(current_edges).ok_or_else(|| {
        Box::new(ExplicitDependencyValidationError::Resource(
            SlibClosureResourceErrorV1::Overflow {
                resource: SlibClosureResourceKindV1::DependencyEdges,
            },
        ))
    })?;
    let depth = dependency_depth.checked_add(2).ok_or_else(|| {
        Box::new(ExplicitDependencyValidationError::Resource(
            SlibClosureResourceErrorV1::Overflow {
                resource: SlibClosureResourceKindV1::GraphDepth,
            },
        ))
    })?;
    meter
        .charge_graph(0, edges, depth)
        .map_err(|source| Box::new(ExplicitDependencyValidationError::Resource(source)))
}

#[derive(Debug)]
struct ValidatedDependencyNode {
    input: ExplicitDependencyArtifactInput,
    artifact: PublishableSingleConeArtifact,
}

fn validate_artifact_shape(
    input: &ExplicitDependencyArtifactInput,
    artifact: &PublishableSingleConeArtifact,
    current_identity: ConeIdentity,
) -> DependencyValidationResult<()> {
    debug_assert_ne!(artifact.identity(), ConeIdentity::CORE);
    debug_assert_ne!(artifact.identity(), current_identity);
    if artifact.kind() != ConeKind::Library || artifact.source_form() != ConeSourceForm::Manifest {
        return Err(
            ExplicitDependencyValidationError::UnsupportedArtifactShape {
                input: input.clone(),
                kind: artifact.kind(),
                source_form: artifact.source_form(),
            }
            .into(),
        );
    }
    Ok(())
}

fn validate_manifest_direct_set(
    manifest: Option<&LoadedConeManifest>,
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode>,
) -> DependencyValidationResult<()> {
    let mut declared = BTreeMap::new();
    for coordinate in manifest
        .map(|manifest| {
            manifest
                .parsed()
                .semantic()
                .dependency_iter()
                .map(|(_, coordinate)| coordinate)
        })
        .into_iter()
        .flatten()
    {
        let identity = coordinate.identity().map_err(|source| {
            Box::new(ExplicitDependencyValidationError::ManifestIdentity {
                coordinate: coordinate.clone(),
                source,
            })
        })?;
        declared.insert(identity, coordinate.clone());
    }
    let actual = nodes
        .iter()
        .filter(|(_, node)| node.input.role == ExplicitDependencyRole::Direct)
        .map(|(identity, node)| (*identity, node.artifact.coordinate().clone()))
        .collect::<BTreeMap<_, _>>();
    if declared != actual {
        return Err(ExplicitDependencyValidationError::ManifestDirectSet {
            declared: declared.into_values().collect(),
            actual: actual.into_values().collect(),
        }
        .into());
    }
    Ok(())
}

fn validate_dependency_records(
    current_identity: ConeIdentity,
    trusted_core: &ValidatedTrustedCoreArtifact<'_>,
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode>,
) -> DependencyValidationResult<()> {
    let core_record = trusted_core.dependency_record();
    for node in nodes.values() {
        for dependency in node.artifact.direct_dependencies() {
            if dependency.identity() == node.artifact.identity() {
                return Err(ExplicitDependencyValidationError::SelfDependency {
                    coordinate: node.artifact.coordinate().clone(),
                }
                .into());
            }
            if dependency.identity() == current_identity {
                return Err(ExplicitDependencyValidationError::CycleToCurrentCone {
                    coordinate: node.artifact.coordinate().clone(),
                    dependency: dependency.coordinate().clone(),
                }
                .into());
            }
            if dependency.identity() == ConeIdentity::CORE {
                if dependency != &core_record {
                    return Err(
                        ExplicitDependencyValidationError::DependencyRecordMismatch {
                            coordinate: node.artifact.coordinate().clone(),
                            dependency: dependency.coordinate().clone(),
                        }
                        .into(),
                    );
                }
                continue;
            }
            let Some(target) = nodes.get(&dependency.identity()) else {
                return Err(
                    ExplicitDependencyValidationError::MissingDependencyArtifact {
                        coordinate: node.artifact.coordinate().clone(),
                        dependency: dependency.coordinate().clone(),
                    }
                    .into(),
                );
            };
            if dependency != &target.artifact.dependency_record() {
                return Err(
                    ExplicitDependencyValidationError::DependencyRecordMismatch {
                        coordinate: node.artifact.coordinate().clone(),
                        dependency: dependency.coordinate().clone(),
                    }
                    .into(),
                );
            }
        }
    }
    Ok(())
}

fn validate_acyclic(
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode>,
) -> DependencyValidationResult<u64> {
    let mut complete = BTreeSet::new();
    let mut maximum_depth = 0_u64;
    for start in nodes.keys().copied() {
        if complete.contains(&start) {
            continue;
        }
        let mut visiting = BTreeMap::<ConeIdentity, usize>::new();
        let mut stack = vec![(start, 0_usize)];
        maximum_depth = maximum_depth.max(1);
        visiting.insert(start, 0);
        while let Some((identity, next_index)) = stack.last_mut() {
            let dependencies = nodes[identity].artifact.direct_dependencies();
            let mut next = None;
            while *next_index < dependencies.len() {
                let dependency = dependencies[*next_index].identity();
                *next_index += 1;
                if nodes.contains_key(&dependency) {
                    next = Some(dependency);
                    break;
                }
            }
            let Some(next) = next else {
                let (finished, _) = stack.pop().expect("stack is known non-empty");
                visiting.remove(&finished);
                complete.insert(finished);
                continue;
            };
            if let Some(position) = visiting.get(&next).copied() {
                let mut cycle = stack[position..]
                    .iter()
                    .map(|(identity, _)| nodes[identity].artifact.coordinate().clone())
                    .collect::<Vec<_>>();
                cycle.push(nodes[&next].artifact.coordinate().clone());
                return Err(ExplicitDependencyValidationError::DependencyCycle { cycle }.into());
            }
            if !complete.contains(&next) {
                visiting.insert(next, stack.len());
                stack.push((next, 0));
                maximum_depth = maximum_depth.max(u64::try_from(stack.len()).unwrap_or(u64::MAX));
            }
        }
    }
    Ok(maximum_depth)
}

fn validate_support_closure(
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode>,
) -> DependencyValidationResult<()> {
    let direct = nodes
        .iter()
        .filter(|(_, node)| node.input.role == ExplicitDependencyRole::Direct)
        .map(|(identity, _)| *identity)
        .collect::<BTreeSet<_>>();
    let actual_support = nodes
        .iter()
        .filter(|(_, node)| node.input.role == ExplicitDependencyRole::Support)
        .map(|(identity, _)| *identity)
        .collect::<BTreeSet<_>>();
    let mut reachable = direct.clone();
    let mut pending = direct.iter().copied().collect::<Vec<_>>();
    while let Some(identity) = pending.pop() {
        for dependency in nodes[&identity].artifact.direct_dependencies() {
            let dependency = dependency.identity();
            if nodes.contains_key(&dependency) && reachable.insert(dependency) {
                pending.push(dependency);
            }
        }
    }
    let expected_support = reachable
        .difference(&direct)
        .copied()
        .collect::<BTreeSet<_>>();
    if expected_support != actual_support {
        return Err(ExplicitDependencyValidationError::SupportClosure {
            missing: expected_support
                .difference(&actual_support)
                .map(|identity| nodes[identity].artifact.coordinate().clone())
                .collect(),
            unexpected: actual_support
                .difference(&expected_support)
                .map(|identity| nodes[identity].artifact.coordinate().clone())
                .collect(),
        }
        .into());
    }
    Ok(())
}

fn manifest_capability_input(
    manifest: &LoadedConeManifest,
) -> DependencyValidationResult<Option<NonCoreDependencyInput>> {
    let Some((key, coordinate)) = manifest.parsed().semantic().dependency_iter().next() else {
        return Ok(None);
    };
    let declaration = manifest
        .parsed()
        .diagnostic_spans()
        .dependency(key)
        .ok_or_else(
            || ExplicitDependencyValidationError::MissingManifestDependencySpan {
                coordinate: coordinate.clone(),
            },
        )?
        .declaration()
        .clone();
    Ok(Some(NonCoreDependencyInput::Manifest {
        coordinate: coordinate.clone(),
        declaration,
    }))
}
