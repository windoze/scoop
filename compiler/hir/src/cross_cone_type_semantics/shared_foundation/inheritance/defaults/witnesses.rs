use super::*;
use crate::{
    DeclaredVisibilityV1, ExportDefaultAccessWitnessV1, PersistentAccessDomainV1,
    ProtectedDefaultAccessWitnessV1, ProtectedDefaultAccessWitnessViewV1, SourceAccessConstraintV1,
    SourceAccessDomainV1,
};
use scoop_identity::{
    CallableTemplateOrigin, DispatchDeclarationOwner, DispatchSlotKey, PersistentDispatchSlotId,
};

pub(super) struct Witnesses<'g, 'a> {
    graph: &'g CheckedNominalInheritanceGraphV1<'a>,
    generic: bool,
    roots: Vec<(PersistentDispatchSlotId, PersistentAccessDomainV1)>,
}

impl<'g, 'a> Witnesses<'g, 'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        current: SharedTypeMetadataV1<'_>,
        dependencies: &[CheckedSharedTypeFoundationV1<'_>],
        context: &Context<'_>,
        graph: &'g CheckedNominalInheritanceGraphV1<'a>,
        key: ProtectedDefaultTemplateKeyV1,
        template: &ExportDefaultTemplateV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let source = contracts::callable(current, key.owner(), meter)?;
        let access = contracts::callable_access(current, source, meter)?;
        let mut generic = matches!(
            source.owner().nominal_owner(),
            Some(SourceNominalId::GenericTemplate(_))
        ) || generic_access(&access);
        for owner in access.lexical_owners() {
            contracts::lookup(context.sources.len(), meter)?;
            generic |= generic_access(&context.source(*owner)?.access);
        }
        let r = template.references();
        for witness in r
            .callables()
            .iter()
            .map(|r| r.witness())
            .chain(r.constructors().iter().map(|r| r.witness()))
            .chain(r.types().iter().map(|r| r.witness()))
            .chain(r.globals().iter().map(|r| r.witness()))
            .chain(r.singleton_values().iter().map(|r| r.witness()))
            .chain(r.fields().iter().map(|r| r.witness()))
        {
            for domain in std::iter::once(witness.direct_call_domain())
                .chain(witness.slot_call_domain())
                .chain(std::iter::once(witness.target_domain()))
            {
                meter.charge_work(domain.constraints().len() as u64 + 1, &WirePath::root())?;
                generic |= generic_domain(domain);
            }
        }
        let mut roots = Vec::new();
        if !generic {
            meter.try_reserve_collection_slots(
                &mut roots,
                source.slot_relations().values().len(),
                &WirePath::root(),
            )?;
            for slot in source.slot_relations().values() {
                contracts::lookup(current.identities.identity_count(), meter)?;
                let slot_key = current
                    .identities
                    .canonical_key::<_, DispatchSlotKey>(*slot)?;
                let DispatchDeclarationOwner::Function(function) = slot_key.owner() else {
                    return Err(Error::DefaultWitness(key));
                };
                let function_key = current
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(function)?;
                let provider = metadata(current, dependencies, function_key.origin(), meter)?;
                let root = contracts::callable(
                    provider,
                    CallableTemplateOrigin::Function(function),
                    meter,
                )?;
                let access = contracts::callable_access(provider, root, meter)?;
                let domains = graph
                    .replay_access(&access, meter)
                    .map_err(Error::InheritanceDomains)?;
                let domain = domains.lookup().domain();
                let bytes = scoop_wire::encoded_length(domain)
                    .map_err(|error| Error::Key(error.to_string()))?;
                meter.charge_owned_bytes(bytes, &WirePath::root())?;
                meter.charge_collection_slots(
                    domain.constraints().len() as u64,
                    &WirePath::root(),
                )?;
                roots.push((*slot, domain.clone()));
            }
        }
        Ok(Self {
            graph,
            generic,
            roots,
        })
    }

    pub(super) fn validate(
        &self,
        witness: &ProtectedDefaultAccessWitnessV1,
        expected: &ExportDefaultAccessWitnessV1,
        key: ProtectedDefaultTemplateKeyV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        meter.charge_work(1, &WirePath::root())?;
        if witness.owner() != key.owner() || expected.owner() != key.owner() {
            return Err(Error::DefaultWitness(key));
        }
        match (witness.view(), self.generic) {
            (ProtectedDefaultAccessWitnessViewV1::GenericSourceMetadata { .. }, true) => Ok(()),
            (ProtectedDefaultAccessWitnessViewV1::ParamFree(witness), false) => {
                let direct = self
                    .graph
                    .replay_source_domain(expected.direct_call_domain(), meter)
                    .map_err(Error::InheritanceDomains)?;
                let target = self
                    .graph
                    .replay_source_domain(expected.target_domain(), meter)
                    .map_err(Error::InheritanceDomains)?;
                contracts::charge_compare(
                    witness.direct_call_domain().domain(),
                    direct.domain(),
                    meter,
                )?;
                contracts::charge_compare(
                    witness.target_domain().domain(),
                    target.domain(),
                    meter,
                )?;
                let slots = witness.slot_call_domains().records();
                meter.charge_work(
                    slots.len() as u64 + self.roots.len() as u64,
                    &WirePath::root(),
                )?;
                if witness.direct_call_domain().domain() != direct.domain()
                    || witness.target_domain().domain() != target.domain()
                    || slots.len() != self.roots.len()
                    || !target
                        .covers(&direct, meter)
                        .map_err(Error::InheritanceDomains)?
                {
                    return Err(Error::DefaultWitness(key));
                }
                for (slot, (id, domain)) in slots.iter().zip(&self.roots) {
                    contracts::charge_compare(slot.domain().domain(), domain, meter)?;
                    if slot.slot() != *id || slot.domain().domain() != domain {
                        return Err(Error::DefaultWitness(key));
                    }
                    let required = self
                        .graph
                        .validate_access_domain(domain, meter)
                        .map_err(Error::InheritanceDomains)?;
                    if !target
                        .covers(&required, meter)
                        .map_err(Error::InheritanceDomains)?
                    {
                        return Err(Error::DefaultWitness(key));
                    }
                }
                Ok(())
            }
            _ => Err(Error::DefaultWitness(key)),
        }
    }
}

fn generic_access(access: &DeclarationAccessSourceV1) -> bool {
    access.declared_visibility() == DeclaredVisibilityV1::Protected
        && matches!(
            access.lexical_owners().last(),
            Some(SourceNominalId::GenericTemplate(_))
        )
}

fn generic_domain(domain: &SourceAccessDomainV1) -> bool {
    domain.constraints().iter().any(|constraint| {
        matches!(
            constraint,
            SourceAccessConstraintV1::SubclassesOf(SourceNominalId::GenericTemplate(_))
        )
    })
}
