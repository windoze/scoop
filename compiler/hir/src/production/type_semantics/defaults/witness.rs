use super::*;
use scoop_identity::PersistentDispatchSlotId;
use std::collections::BTreeMap;

type SlotMap = BTreeMap<
    CallableTemplateOrigin,
    BTreeMap<PersistentDispatchSlotId, PersistentSlotContractDomainV1>,
>;

pub(super) enum Publication {
    ParamFree {
        owner: CallableTemplateOrigin,
        direct: PersistentLookupDomainV1,
        slots: CanonicalProtectedDefaultSlotCallDomainsV1,
    },
    Generic {
        owner: CallableTemplateOrigin,
    },
}

impl Publication {
    pub(super) fn new(
        export: &ExportHir,
        local: ExportParameterOwner,
        source: &DefaultSourceTemplateV1,
        roots: &SlotMap,
    ) -> Result<Self, Error> {
        let profile = super::super::source_defaults::profile::from_source(export, local, source)?;
        let owner = source.key().owner();
        if profile == ProtectedDefaultWitnessSourceProfileV1::GenericSourceMetadata {
            return Ok(Self::Generic { owner });
        }
        let lookup = match local {
            ExportParameterOwner::Function(id) => &export.functions[id].access.lookup,
            ExportParameterOwner::ClassConstructor(id) => {
                &export.class_constructors[id].access.lookup
            }
            ExportParameterOwner::StructConstructor(id) => {
                &export.struct_constructors[id].access.lookup
            }
            ExportParameterOwner::VariantConstructor(id) => {
                &export.enums[id.enumeration()].access.lookup
            }
        };

        let domain =
            DefaultSourceAccessDomainV1::from_export_hir(export, &lookup.0).map_err(invalid)?;
        if !domain.generic_subclasses().is_empty() {
            return Err(invalid(
                "concrete default owner retains a generic access constraint",
            ));
        }
        let direct = PersistentLookupDomainV1::new(domain.persistent().clone());

        let mut slots = Vec::new();
        if let Some(roots) = roots.get(&owner) {
            for (slot, domain) in roots {
                resources::push(
                    &mut slots,
                    ProtectedDefaultSlotCallDomainV1::new(*slot, domain.clone()),
                )?;
            }
        }
        let slots = CanonicalProtectedDefaultSlotCallDomainsV1::try_new(slots).map_err(invalid)?;
        Ok(Self::ParamFree {
            owner,
            direct,
            slots,
        })
    }

    pub(super) fn reference(
        &self,
        source: &DefaultSourceAccessWitnessV1,
    ) -> Result<ProtectedDefaultAccessWitnessV1, Error> {
        match self {
            Self::Generic { owner } => {
                ProtectedDefaultAccessWitnessV1::generic_source_metadata(*owner).map_err(invalid)
            }
            Self::ParamFree {
                owner,
                direct,
                slots,
            } => {
                let target = source.target_domain();
                if !target.generic_subclasses().is_empty() {
                    return Err(invalid(
                        "concrete default reference retains a generic access constraint",
                    ));
                }

                ProtectedDefaultAccessWitnessV1::param_free(
                    *owner,
                    direct.clone(),
                    slots.clone(),
                    PersistentLookupDomainV1::new(target.persistent().clone()),
                )
                .map_err(invalid)
            }
        }
    }
}

pub(super) fn root_slots(
    inheritance: &CanonicalNominalInheritanceInterfacesV1,
) -> Result<SlotMap, Error> {
    let mut roots = SlotMap::new();
    for nominal in inheritance.records() {
        for slot in nominal.slots().records() {
            let declarations = std::iter::once(slot.declaration()).chain(
                slot.implementation()
                    .target()
                    .map(|target| target.declaration()),
            );
            for declaration in declarations {
                let InheritanceCallableDeclarationV1::Function(id) = declaration else {
                    continue;
                };

                let slots = roots
                    .entry(CallableTemplateOrigin::Function(id))
                    .or_default();

                if let Some(previous) = slots.insert(slot.slot(), slot.domain().clone())
                    && previous != *slot.domain()
                {
                    return Err(invalid("shared default root slot has conflicting domains"));
                }
            }
        }
    }
    Ok(roots)
}
