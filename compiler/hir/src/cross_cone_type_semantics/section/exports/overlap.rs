//! Agreement of independently checked public and restricted source surfaces.
use super::*;
use scoop_identity::{CallableTemplateOrigin, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{WireError, WireErrorKind};

mod callables;
mod defaults;
mod nested;
mod nominals;
mod properties;
mod protocols;
mod resources;
use resources::*;
#[cfg(test)]
mod tests;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate<F: TypeSectionFoundationSemanticAuthority<E>, E>(
    candidate: &CrossConeTypeSemanticsSectionV1,
    public: CheckedTypeSectionPublicSupportV1<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    sources: &CheckedProtectedSourceInterfacesV1<'_>,
    defaults: &CheckedProtectedDefaultTemplatesV1<'_>,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    if !std::ptr::eq(sources.table(), candidate.protected_source_interfaces())
        || !std::ptr::eq(defaults.table(), candidate.protected_defaults())
    {
        return Err(TypeSectionExportValidationError::PublicOverlap);
    }
    let public = public.section();
    nominals::validate(candidate, public, graph, foundation, meter, path)?;
    sequence(sources.entries().len(), meter, path)?;
    for source in sources.entries() {
        callables::validate(
            source.owner(),
            source.declaration_access(),
            source.payload(),
            public,
            graph,
            meter,
            path,
        )?;
        protocols::validate(source, public, graph, meter, path)?;
    }
    nested::validate(candidate, public, graph, meter, path)?;
    defaults::validate(defaults, public, meter, &path.clone().field(6))
}

fn require<E>(matches: bool) -> Result<(), TypeSectionExportValidationError<E>> {
    matches
        .then_some(())
        .ok_or(TypeSectionExportValidationError::PublicOverlap)
}

fn effective_public<E>(
    access: &DeclarationAccessSourceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, TypeSectionExportValidationError<E>> {
    if access.declared_visibility() != DeclaredVisibilityV1::Public {
        return Ok(false);
    }
    public_owners(access, graph, meter, path)
}

fn public_owners<E>(
    access: &DeclarationAccessSourceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, TypeSectionExportValidationError<E>> {
    sequence(access.lexical_owners().len(), meter, path)?;
    for owner in access.lexical_owners() {
        let domain = graph
            .replay_nominal_access(*owner, meter)
            .map_err(|e| match e {
                AccessDomainSemanticError::Resource(e) => {
                    TypeSectionExportValidationError::Resource(e)
                }
                _ => TypeSectionExportValidationError::PublicOverlap,
            })?;
        if !domain.lookup().domain().is_universal() {
            return Ok(false);
        }
    }
    Ok(true)
}
