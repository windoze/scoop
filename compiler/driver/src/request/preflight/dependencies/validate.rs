use std::collections::BTreeMap;
use std::rc::Rc;

use scoop_identity::{ConeIdentity, SemanticIdentitySession};
use scoop_manifest::LoadedConeManifest;
use scoop_slib::{
    CrossConeArtifactClosureInput, SlibClosureDecodeMeterV1, SlibClosureDecodePurposeV1,
    SlibClosureResourceErrorV1, SlibClosureResourceKindV1, validate_cross_cone_artifact_closure,
};
use scoop_toolchain::ResolvedTargetProfile;
use scoop_wire::sha256;

use super::graph::{
    ValidatedDependencyNode, charge_graph_edges_and_depth, dependency_first_order,
    validate_acyclic, validate_artifact_shape, validate_dependency_records,
    validate_manifest_direct_set, validate_support_closure,
};
use super::{
    DependencyValidationResult, ExplicitDependencyRole, ExplicitDependencyValidationError,
    LoadedExplicitDependencyInputs,
};
use crate::ValidatedExplicitDependencyInputSet;

impl LoadedExplicitDependencyInputs {
    pub(crate) fn validate_inner<'input>(
        &'input self,
        manifest: Option<&LoadedConeManifest>,
        current_identity: ConeIdentity,
        target: &ResolvedTargetProfile,
        mut meter: Option<&mut SlibClosureDecodeMeterV1>,
    ) -> DependencyValidationResult<ValidatedExplicitDependencyInputSet<'input>> {
        charge_graph_nodes(meter.as_deref_mut(), self.artifacts.len())?;

        let mut nodes = BTreeMap::<ConeIdentity, ValidatedDependencyNode<'input>>::new();
        let mut group_names = BTreeMap::<(String, String), (String, ConeIdentity)>::new();
        if let Some(manifest) = manifest {
            let coordinate = manifest.parsed().semantic().coordinate();
            group_names.insert(
                (coordinate.group().to_owned(), coordinate.name().to_owned()),
                (coordinate.version().to_owned(), current_identity),
            );
        }

        for loaded in &self.artifacts {
            let summary = loaded.summary(
                self.limits,
                target.lir_target_selection(),
                meter.as_deref_mut(),
            )?;

            let identity = summary.cone().identity();
            if identity == current_identity {
                return Err(ExplicitDependencyValidationError::CurrentConeArtifact {
                    input: loaded.input.clone(),
                    identity: current_identity,
                }
                .into());
            }
            validate_artifact_shape(&loaded.input, &summary, current_identity)?;

            if let Some(previous) = nodes.get(&identity) {
                return Err(ExplicitDependencyValidationError::DuplicateIdentity {
                    identity,
                    first: previous.input.clone(),
                    second: loaded.input.clone(),
                    same_fingerprint: previous.summary.artifact_fingerprint()
                        == summary.artifact_fingerprint(),
                }
                .into());
            }

            let coordinate = summary.cone().coordinate();
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
                (coordinate.version().to_owned(), summary.cone().identity()),
            );
            nodes.insert(
                identity,
                ValidatedDependencyNode {
                    input: loaded.input.clone(),
                    bytes: &loaded.bytes,
                    summary,
                },
            );
        }

        validate_manifest_direct_set(manifest, current_identity, &nodes)?;
        validate_dependency_records(current_identity, &nodes)?;
        let dependency_depth = validate_acyclic(&nodes)?;
        validate_support_closure(&nodes)?;
        let dependency_order = dependency_first_order(&nodes)?;
        if let Some(meter) = meter.as_deref_mut() {
            charge_graph_edges_and_depth(meter, &nodes, dependency_depth)?;
        }

        let direct = nodes
            .iter()
            .filter(|(_, node)| node.input.role == ExplicitDependencyRole::Direct)
            .map(|(identity, _)| *identity)
            .collect::<Vec<_>>();
        let dependency_first = dependency_order
            .iter()
            .map(|identity| nodes[identity].bytes)
            .collect::<Vec<_>>();

        let mut semantic_session = SemanticIdentitySession::new();
        let closure = validate_cross_cone_artifact_closure(
            CrossConeArtifactClosureInput::dependencies(
                current_identity,
                target.lir_target_selection(),
                direct.clone(),
                dependency_first.clone(),
            ),
            self.limits,
            target.c_bridge_toolchain().profile(),
            &mut semantic_session,
        )
        .map_err(|source| Box::new(ExplicitDependencyValidationError::Closure(Box::new(source))))?;

        if let Some(meter) = meter {
            charge_artifact_views(meter, &nodes, &dependency_order, &closure)?;
        }
        let direct_dependencies = direct
            .into_iter()
            .map(|identity| {
                closure
                    .publication(identity)
                    .expect("validated direct dependency belongs to the closure")
                    .dependency_record()
            })
            .collect();
        Ok(ValidatedExplicitDependencyInputSet::new(
            Rc::new(closure),
            dependency_first,
            direct_dependencies,
            semantic_session,
        ))
    }
}

fn charge_graph_nodes(
    meter: Option<&mut SlibClosureDecodeMeterV1>,
    dependency_count: usize,
) -> DependencyValidationResult<()> {
    let Some(meter) = meter else {
        return Ok(());
    };
    let dependency_nodes = u64::try_from(dependency_count).map_err(|_| {
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
        .map_err(|source| Box::new(ExplicitDependencyValidationError::Resource(source)))
}

fn charge_artifact_views(
    meter: &mut SlibClosureDecodeMeterV1,
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode<'_>>,
    dependency_order: &[ConeIdentity],
    closure: &scoop_slib::ValidatedCrossConeArtifactClosure<'_>,
) -> DependencyValidationResult<()> {
    for identity in dependency_order {
        let node = &nodes[identity];
        let snapshot = sha256(node.bytes);
        let publication = closure
            .publication(*identity)
            .expect("validated dependency belongs to the closure");
        meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Compile,
                publication.artifact_fingerprint(),
                snapshot,
                publication.compile_summary().decode_usage(),
            )
            .map_err(|source| Box::new(ExplicitDependencyValidationError::Resource(source)))?;
        meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Link,
                publication.artifact_fingerprint(),
                snapshot,
                publication.link_summary().decode_usage(),
            )
            .map_err(|source| Box::new(ExplicitDependencyValidationError::Resource(source)))?;
    }
    Ok(())
}
