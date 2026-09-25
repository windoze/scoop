use super::super::inheritance::source_errors::invalid;
use super::super::{CrossConeTypeSemanticsProductionError as Error, nominal_parameters, nominals};
use crate::*;

/// Classify access proof availability, independently of generic execution gates.
pub(in crate::production::type_semantics) fn from_source(
    export: &ExportHir,
    local: ExportParameterOwner,
    source: &DefaultSourceTemplateV1,
) -> Result<ProtectedDefaultWitnessSourceProfileV1, Error> {
    let (owner, key) = nominal_parameters::owners::identity(export, local)
        .ok_or_else(|| invalid("default profile has no source identity"))?;
    if owner != source.key().owner() {
        return Err(invalid("default profile source owner differs"));
    }

    let lexical = nominals::lexical_owners(key)?;
    let mut generic = match local {
        ExportParameterOwner::VariantConstructor(id) => {
            !export.enums[id.enumeration()].type_params.is_empty()
        }
        ExportParameterOwner::Function(_)
        | ExportParameterOwner::ClassConstructor(_)
        | ExportParameterOwner::StructConstructor(_) => {
            matches!(lexical.last(), Some(SourceNominalId::GenericTemplate(_)))
        }
    };
    let lookup = match local {
        ExportParameterOwner::Function(id) => &export.functions[id].access.lookup,
        ExportParameterOwner::ClassConstructor(id) => &export.class_constructors[id].access.lookup,
        ExportParameterOwner::StructConstructor(id) => {
            &export.struct_constructors[id].access.lookup
        }
        ExportParameterOwner::VariantConstructor(id) => {
            &export.enums[id.enumeration()].access.lookup
        }
    };
    let domain =
        DefaultSourceAccessDomainV1::from_export_hir(export, &lookup.0).map_err(invalid)?;
    generic |= !domain.generic_subclasses().is_empty();
    let r = source.references();
    let witnesses = r
        .callables()
        .iter()
        .map(|r| r.witness())
        .chain(r.constructors().iter().map(|r| r.witness()))
        .chain(r.types().iter().map(|r| r.witness()))
        .chain(r.globals().iter().map(|r| r.witness()))
        .chain(r.singleton_values().iter().map(|r| r.witness()))
        .chain(r.fields().iter().map(|r| r.witness()));
    for witness in witnesses {
        generic |= !witness.direct_call_domain().generic_subclasses().is_empty()
            || !witness.target_domain().generic_subclasses().is_empty()
            || matches!(witness.slot_call_domain(), OptionalDefaultSourceSlotDomainV1::Present(domain) if !domain.generic_subclasses().is_empty());
    }
    Ok(if generic {
        ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata
    } else {
        ProtectedDefaultWitnessSourceProfileV1::ParamFree
    })
}
