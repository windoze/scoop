//! HIR call occurrences joined once to the actual MIR dependency definitions.

use scoop_hir::{
    CrossConeHirInterfaceSectionV1, ExternalHirReferenceRoleV1, ExternalHirReferenceV1,
    ExternalHirTargetV1, HirDependencyCallInstantiationV1,
};
use scoop_identity::{
    CallableTemplateOrigin, DependencyCallableDeclarationId, StrongCallableDefinitionOwner,
    ValidatedIdentityGraph,
};
use scoop_mir::{
    CanonicalMirFoundation, CrossConeMirBridgeSectionV1,
    DependencyResolvedCrossConeMirTypeBridgeSectionV1, MirTypeBridgeDependencyV1,
    MirTypeBridgeDependencyViewV1, MirTypeBridgeTargetV1, StrongCallableBridgeSurfaceV1,
};

use crate::CrossConeMirClosureRelationError;

mod applications;

#[cfg(test)]
mod tests;

pub(crate) fn validate_executable_hir_calls(
    interface: &CrossConeHirInterfaceSectionV1,
    strong: &StrongCallableBridgeSurfaceV1,
    bridge: &CrossConeMirBridgeSectionV1,
    lowered: Option<(
        &DependencyResolvedCrossConeMirTypeBridgeSectionV1,
        &[MirTypeBridgeDependencyViewV1<'_>],
    )>,
    foundation: &CanonicalMirFoundation,
    identities: &ValidatedIdentityGraph,
) -> Result<(), CrossConeMirClosureRelationError> {
    use CrossConeMirClosureRelationError as Error;
    let signatures = applications::signatures(
        foundation,
        identities,
        lowered.map(|(section, _)| section.exports()),
    )?;
    for record in bridge.selected() {
        let Some(reference) = interface
            .external_references()
            .get(dependency_hir_target(record.declaration()))
        else {
            continue;
        };
        if reference.origin() != record.provider() {
            return Err(Error::HirSelectionOriginMismatch {
                declaration: record.declaration(),
                expected: record.provider(),
                actual: reference.origin(),
            });
        }
    }
    for reference in interface.external_references().records() {
        if !reference
            .roles()
            .contains(ExternalHirReferenceRoleV1::ConcreteSelectedUse)
        {
            continue;
        }
        if !matches!(
            reference.target(),
            ExternalHirTargetV1::Callable(_) | ExternalHirTargetV1::GeneratedCallable(_)
        ) {
            continue;
        }
        let provider = reference.origin();
        let sites = reference.call_sites().records();
        let direct_signature = if let Some(target) = direct_callable(reference)? {
            Some(
                if let Some(direct) = bridge.selected().iter().find(|record| {
                    record.provider() == provider && record.implementation() == target
                }) {
                    direct.signature()
                } else {
                    let definition = lowered
                        .and_then(|(selected, dependencies)| {
                            let relation = MirTypeBridgeDependencyV1::new(
                                provider,
                                MirTypeBridgeTargetV1::Callable(
                                    scoop_identity::CallableDefinitionOwner::Strong(target),
                                ),
                            );
                            selected
                                .selected_relations()
                                .binary_search(&relation)
                                .ok()?;
                            dependencies
                                .iter()
                                .find(|view| view.provider() == provider)?
                                .exports()
                                .callables()
                                .get(target)
                        })
                        .ok_or(Error::MissingMirSelection { provider, target })?;
                    definition.semantic_signature().exact()
                },
            )
        } else {
            None
        };
        for site in sites {
            let position = site.position();
            applications::validate_root(position, strong, &signatures, identities)?;
            let signature = match site.instantiation() {
                // The HIR source boundary checked the C declaration and exact
                // signature. Native ABI validation owns the resulting leaf.
                HirDependencyCallInstantiationV1::NativeLeaf => continue,
                HirDependencyCallInstantiationV1::Direct => {
                    direct_signature.ok_or(Error::UnmaterializedHirSelection {
                        target: reference.target(),
                    })?
                }
                HirDependencyCallInstantiationV1::Application(application) => signatures
                    .applications
                    .get(&application)
                    .filter(|entry| {
                        reference.target() == ExternalHirTargetV1::Callable(entry.origin)
                    })
                    .map(|entry| entry.signature)
                    .ok_or_else(|| Error::MissingMirApplication {
                        position: Box::new(position),
                        application,
                    })?,
            };
            let arguments = signature
                .receiver()
                .into_option()
                .into_iter()
                .chain(signature.parameters().iter().copied());
            if !arguments.eq(site.arguments().iter().copied())
                || site.result() != signature.result()
            {
                return Err(Error::CallSignature {
                    position: Box::new(position),
                    provider,
                    target: reference.target(),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn direct_callable(
    reference: &ExternalHirReferenceV1,
) -> Result<Option<StrongCallableDefinitionOwner>, CrossConeMirClosureRelationError> {
    let sites = reference.call_sites().records();
    if !sites.is_empty()
        && sites.iter().all(|site| {
            !matches!(
                site.instantiation(),
                HirDependencyCallInstantiationV1::Direct
            )
        })
    {
        return Ok(None);
    }
    concrete_callable(reference.target())
}

fn concrete_callable(
    target: ExternalHirTargetV1,
) -> Result<Option<StrongCallableDefinitionOwner>, CrossConeMirClosureRelationError> {
    Ok(match target {
        ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(id)) => {
            Some(StrongCallableDefinitionOwner::Function(id))
        }
        ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(id)) => {
            Some(StrongCallableDefinitionOwner::PropertyAccessor(id))
        }
        ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(id)) => {
            Some(StrongCallableDefinitionOwner::Constructor(id))
        }
        ExternalHirTargetV1::GeneratedCallable(id) => {
            Some(StrongCallableDefinitionOwner::GeneratedCallable(id))
        }
        ExternalHirTargetV1::Callable(_) => {
            return Err(CrossConeMirClosureRelationError::UnmaterializedHirSelection { target });
        }
        _ => None,
    })
}

pub(crate) fn dependency_hir_target(
    declaration: DependencyCallableDeclarationId,
) -> ExternalHirTargetV1 {
    ExternalHirTargetV1::Callable(match declaration {
        DependencyCallableDeclarationId::Function(id) => CallableTemplateOrigin::Function(id),
        DependencyCallableDeclarationId::PropertyAccessor(id) => {
            CallableTemplateOrigin::Accessor(id)
        }
    })
}
