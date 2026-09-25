use std::collections::BTreeMap;
use std::rc::Rc;

use scoop_identity::{ConeIdentity, SemanticIdentitySession};
use scoop_manifest::LoadedConeManifest;
use scoop_slib::{CrossConeArtifactClosureInput, validate_cross_cone_artifact_closure};
use scoop_toolchain::ResolvedTargetProfile;

use super::graph::{
    ValidatedDependencyNode, dependency_first_order, validate_acyclic, validate_artifact_shape,
    validate_dependency_records, validate_manifest_direct_set, validate_support_closure,
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
    ) -> DependencyValidationResult<ValidatedExplicitDependencyInputSet<'input>> {
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
            let summary = loaded.summary(self.limits, target.lir_target_selection())?;

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
        validate_acyclic(&nodes)?;
        validate_support_closure(&nodes)?;
        let dependency_order = dependency_first_order(&nodes)?;

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
