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
        roots: &SlotMap,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let (owner, key) = super::super::nominal_parameters::owners::identity(export, local)
            .ok_or_else(|| invalid("default publication has no source identity"))?;
        resources::canonical(key.owners().owners().len(), meter)?;
        let lexical = super::super::nominals::lexical_owners(key)?;
        if lexical
            .iter()
            .any(|owner| matches!(owner, SourceNominalId::GenericTemplate(_)))
        {
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
        resources::canonical(lookup.0.constraints().len(), meter)?;
        let domain = DefaultSourceAccessDomainV1::from_export_hir(export, &lookup.0, meter)
            .map_err(invalid)?;
        if !domain.generic_subclasses().is_empty() {
            return Err(invalid(
                "concrete default owner retains a generic access constraint",
            ));
        }
        let direct = PersistentLookupDomainV1::new(copied(domain.persistent(), meter)?);
        work(meter, roots.len())?;
        let mut slots = Vec::new();
        if let Some(roots) = roots.get(&owner) {
            resources::canonical(roots.len(), meter)?;
            for (slot, domain) in roots {
                resources::canonical(domain.domain().constraints().len(), meter)?;
                resources::push(
                    &mut slots,
                    ProtectedDefaultSlotCallDomainV1::new(*slot, copied(domain, meter)?),
                    meter,
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
        meter: &mut BudgetMeter,
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
                resources::canonical(direct.domain().constraints().len(), meter)?;
                resources::canonical(target.persistent().constraints().len(), meter)?;
                resources::canonical(slots.records().len(), meter)?;
                for slot in slots.records() {
                    resources::canonical(slot.domain().domain().constraints().len(), meter)?;
                }
                ProtectedDefaultAccessWitnessV1::param_free(
                    *owner,
                    copied(direct, meter)?,
                    copied(slots, meter)?,
                    PersistentLookupDomainV1::new(copied(target.persistent(), meter)?),
                )
                .map_err(invalid)
            }
        }
    }
}

pub(super) fn root_slots(
    inheritance: &CanonicalNominalInheritanceInterfacesV1,
    meter: &mut BudgetMeter,
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
                work(meter, roots.len())?;
                meter
                    .charge_collection_slots(2, &WirePath::root())
                    .map_err(resource)?;
                let slots = roots
                    .entry(CallableTemplateOrigin::Function(id))
                    .or_default();
                work(meter, slots.len())?;
                resources::canonical(slot.domain().domain().constraints().len(), meter)?;
                if let Some(previous) = slots.insert(slot.slot(), copied(slot.domain(), meter)?)
                    && previous != *slot.domain()
                {
                    return Err(invalid("shared default root slot has conflicting domains"));
                }
            }
        }
    }
    Ok(roots)
}

fn copied<T: Clone + scoop_wire::WireEncode>(
    value: &T,
    meter: &mut BudgetMeter,
) -> Result<T, Error> {
    // Structural slots are charged by the caller; encoded length also accounts
    // for variable-sized source paths in file visibility constraints.
    let length = scoop_wire::encoded_length(value).map_err(invalid)?;
    let path = WirePath::root();
    meter.charge_owned_bytes(length, &path).map_err(resource)?;
    meter
        .charge_work(length.saturating_mul(2), &path)
        .map_err(resource)?;
    Ok(value.clone())
}
