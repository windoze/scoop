//! Candidate support comes only from this reader's same-provider MIR/Strong join.

use super::*;
use lir::{
    ExternalStrongShapeSubjectV1 as Subject, ShapeLinkError as Error, ShapeLinkSupportLookupV1,
    ShapeLinkSupportSourceV1 as Source,
};
use mir::MirTypeBridgeTargetV1 as Target;
use scoop_identity::{
    CallableBodyKey, PersistentCallableBodyId, PersistentInitializationUnitId,
    StrongCallableDefinitionOwner,
};

pub(super) struct SharedSupport<'q, 'a, 'input> {
    pub consumer: &'q mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    pub dependencies: &'q [&'a PhysicalImportsReplayedCrossConeLayoutSections<'input, 'a>],
}

impl<'a> ShapeLinkSupportLookupV1<'a> for SharedSupport<'_, 'a, '_> {
    fn support_source(
        &self,
        provider: ConeIdentity,
        subject: Subject,
        meter: &mut BudgetMeter,
    ) -> Result<Option<Source<'a>>, Error> {
        use Subject::*;
        if matches!(
            subject,
            Callable(_)
                | Layout(_)
                | Scan(_)
                | TypeDescriptor(_)
                | DispatchTable(_)
                | TypeRegistration(_)
        ) {
            meter.charge_work(1, &WirePath::root())?;
            return Ok(None);
        }
        meter.charge_work(self.dependencies.len() as u64, &WirePath::root())?;
        let terminal = self
            .dependencies
            .iter()
            .copied()
            .find(|value| value.identity() == provider)
            .ok_or(Error::MissingProvider(provider))?;
        let units = terminal
            .strong
            .initialization_registrations()
            .registrations();
        meter.charge_work(units.len() as u64, &WirePath::root())?;
        let unit = units
            .iter()
            .map(|record| record.semantic())
            .find(|unit| match subject {
                InitializationCell(id) | InitializationDescriptor(id) => unit.unit() == id,
                StaticStorage(id) | StaticStorageRegistration(id) => {
                    unit.storage() == id || unit.failure_root() == id
                }
                _ => false,
            })
            .ok_or(Error::SupportRelation(subject))?;
        if !self.selected(provider, Target::InitializationUnit(unit.unit()), meter)? {
            return Err(Error::SupportRelation(subject));
        }
        meter.charge_work(terminal.units.len() as u64, &WirePath::root())?;
        let proof = terminal
            .units
            .iter()
            .find(|proof| proof.unit() == unit.unit())
            .ok_or(Error::SupportRelation(subject))?;
        if body(proof.initializer(), meter)? != unit.initializer()
            || body(proof.ensure(), meter)? != unit.ensure()
        {
            return Err(Error::SupportRelation(subject));
        }
        match subject {
            InitializationCell(_) | InitializationDescriptor(_) => {
                Ok(Some(Source::Initialization { unit }))
            }
            StaticStorage(id) | StaticStorageRegistration(id) => {
                let explicit = self.initialization_support(provider, unit.unit(), meter)?;
                let singleton = id == unit.storage() && self.singleton(terminal, proof, meter)?;
                if !explicit && !singleton {
                    return Err(Error::SupportRelation(subject));
                }
                let storages = terminal
                    .strong
                    .static_storage_registrations()
                    .registrations();
                meter.charge_work(storages.len() as u64, &WirePath::root())?;
                let storage = storages
                    .iter()
                    .find(|record| record.semantic().storage() == id)
                    .ok_or(Error::SupportRelation(subject))?
                    .semantic();
                Ok(Some(Source::StaticStorage { unit, storage }))
            }
            _ => Err(Error::SupportRelation(subject)),
        }
    }
}

impl SharedSupport<'_, '_, '_> {
    fn selected(
        &self,
        provider: ConeIdentity,
        target: Target,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        let selected = self.consumer.selected_relations();
        meter.charge_work(
            u64::from(selected.len().max(1).ilog2()) + 1,
            &WirePath::root(),
        )?;
        Ok(selected
            .binary_search(&mir::MirTypeBridgeDependencyV1::new(provider, target))
            .is_ok())
    }

    fn initialization_support(
        &self,
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        let uses = self.consumer.exports().initialization_uses().records();
        meter.charge_work(uses.len() as u64, &WirePath::root())?;
        Ok(uses.iter().any(|edge| {
            edge.provider() == provider
                && edge.dependency_unit() == unit
                && edge.cause()
                    == mir::MirExternalInitializationCauseV1::InitializationSupport(unit)
        }))
    }

    fn singleton(
        &self,
        terminal: &PhysicalImportsReplayedCrossConeLayoutSections<'_, '_>,
        proof: &mir::MirTypeBridgeInitializationUnitV1,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        let objects = terminal.mir.exports().objects().records();
        meter.charge_work(objects.len() as u64, &WirePath::root())?;
        for object in objects {
            if object.unit() == proof.unit()
                && object.ensure() == proof.ensure()
                && self.selected(terminal.identity(), Target::Object(object.value()), meter)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn body(
    owner: StrongCallableDefinitionOwner,
    meter: &mut BudgetMeter,
) -> Result<PersistentCallableBodyId, Error> {
    let key = CallableBodyKey::strong(owner);
    meter.charge_sha256(
        PersistentCallableBodyId::hash_stream_length(&key).map_err(|_| Error::Contract)?,
        &WirePath::root(),
    )?;
    PersistentCallableBodyId::from_key(&key).map_err(|_| Error::Contract)
}
