//! Shared HIR occurrence to MIR callable checks for ordinary and layout profiles.

use scoop_hir::{CrossConeHirInterfaceSectionV1, ExternalHirReferenceRoleV1, ExternalHirTargetV1};
use scoop_identity::{
    CallableMaterializationContext, CallableOwner, CallableTemplateOrigin, CallableTemplateOwner,
    DependencyCallableDeclarationId,
};
use scoop_mir::{CrossConeMirBridgeSectionV1, StrongCallableBridgeSurfaceV1};
use scoop_wire::{BudgetMeter, WirePath};

use crate::CrossConeMirClosureRelationError;

#[cfg(test)]
mod tests;

pub(crate) fn validate_executable_hir_calls(
    interface: &CrossConeHirInterfaceSectionV1,
    strong: &StrongCallableBridgeSurfaceV1,
    bridge: &CrossConeMirBridgeSectionV1,
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeMirClosureRelationError> {
    use CrossConeMirClosureRelationError as Error;
    let path = WirePath::root();
    meter
        .charge_work(
            (interface.external_references().records().len() + bridge.selected().len()) as u64
                * (u64::from(bridge.selected().len().max(1).ilog2())
                    + u64::from(
                        interface
                            .external_references()
                            .records()
                            .len()
                            .max(1)
                            .ilog2(),
                    )
                    + 2),
            &path,
        )
        .map_err(Error::Resource)?;
    validate_hir_selected_set(interface, bridge)?;
    for selected in bridge.selected() {
        let reference = interface
            .external_references()
            .get(dependency_hir_target(selected.declaration()))
            .ok_or(Error::MissingHirSelection {
                declaration: selected.declaration(),
            })?;
        for site in reference.call_sites().records() {
            let position = site.position();
            meter
                .charge_work(
                    site.arguments().len() as u64
                        + u64::from(strong.bridges().len().max(1).ilog2())
                        + 2,
                    &path,
                )
                .map_err(Error::Resource)?;
            if position.root.context() != CallableMaterializationContext::NoSubstitution {
                return Err(Error::CallRoot { position });
            }
            let owner = match position.root.template() {
                CallableTemplateOwner::Function(id) => CallableOwner::Function(id),
                CallableTemplateOwner::Constructor(id) => CallableOwner::Constructor(id),
                CallableTemplateOwner::Accessor(id) => CallableOwner::Accessor(id),
                CallableTemplateOwner::Generated(id) => CallableOwner::Generated(id),
                CallableTemplateOwner::GenericFunction(_)
                | CallableTemplateOwner::VariantConstructor(_) => {
                    return Err(Error::CallRoot { position });
                }
            };
            if strong.get(owner).is_none() {
                return Err(Error::CallRoot { position });
            }
            let signature = selected.signature();
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
                    provider: selected.provider(),
                    declaration: selected.declaration(),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_hir_selected_set(
    interface: &CrossConeHirInterfaceSectionV1,
    bridge: &CrossConeMirBridgeSectionV1,
) -> Result<(), CrossConeMirClosureRelationError> {
    let selected = bridge.selected();
    for record in selected {
        let target = dependency_hir_target(record.declaration());
        let reference = interface.external_references().get(target).ok_or(
            CrossConeMirClosureRelationError::MissingHirSelection {
                declaration: record.declaration(),
            },
        )?;
        if reference.origin() != record.provider() {
            return Err(
                CrossConeMirClosureRelationError::HirSelectionOriginMismatch {
                    declaration: record.declaration(),
                    expected: record.provider(),
                    actual: reference.origin(),
                },
            );
        }
        if !reference
            .roles()
            .contains(ExternalHirReferenceRoleV1::ConcreteSelectedUse)
        {
            return Err(CrossConeMirClosureRelationError::MissingHirSelectionRole {
                declaration: record.declaration(),
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
        let declaration = match reference.target() {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(declaration)) => {
                Some(DependencyCallableDeclarationId::Function(declaration))
            }
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(declaration)) => Some(
                DependencyCallableDeclarationId::PropertyAccessor(declaration),
            ),
            target @ (ExternalHirTargetV1::Callable(_)
            | ExternalHirTargetV1::GeneratedCallable(_)) => {
                return Err(CrossConeMirClosureRelationError::UnsupportedHirSelection { target });
            }
            ExternalHirTargetV1::Nominal(_)
            | ExternalHirTargetV1::Property(_)
            | ExternalHirTargetV1::ObjectValue(_)
            | ExternalHirTargetV1::TypeAlias(_)
            | ExternalHirTargetV1::Field(_)
            | ExternalHirTargetV1::EnumVariantField(_) => None,
        };
        let Some(declaration) = declaration else {
            continue;
        };
        if selected
            .binary_search_by_key(&(reference.origin(), declaration), |record| {
                (record.provider(), record.declaration())
            })
            .is_err()
        {
            return Err(CrossConeMirClosureRelationError::MissingMirSelection {
                provider: reference.origin(),
                declaration,
            });
        }
    }
    Ok(())
}

pub(crate) fn dependency_hir_target(
    declaration: DependencyCallableDeclarationId,
) -> ExternalHirTargetV1 {
    let callable = match declaration {
        DependencyCallableDeclarationId::Function(declaration) => {
            CallableTemplateOrigin::Function(declaration)
        }
        DependencyCallableDeclarationId::PropertyAccessor(declaration) => {
            CallableTemplateOrigin::Accessor(declaration)
        }
    };
    ExternalHirTargetV1::Callable(callable)
}
