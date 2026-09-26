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

pub(super) struct SharedSupport<'q, 'a> {
    pub consumer: &'q mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    pub dependencies: &'q [&'a PhysicalImportsReplayedCrossConeLayoutSections],
}

impl<'a> ShapeLinkSupportLookupV1<'a> for SharedSupport<'_, 'a> {
    fn support_source(
        &self,
        provider: ConeIdentity,
        subject: Subject,
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
            return Ok(None);
        }

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
        if !self.selected(provider, Target::InitializationUnit(unit.unit()))? {
            return Err(Error::SupportRelation(subject));
        }

        let proof = terminal
            .units
            .iter()
            .find(|proof| proof.unit() == unit.unit())
            .ok_or(Error::SupportRelation(subject))?;
        if body(proof.initializer())? != unit.initializer()
            || body(proof.ensure())? != unit.ensure()
        {
            return Err(Error::SupportRelation(subject));
        }
        match subject {
            InitializationCell(_) | InitializationDescriptor(_) => {
                Ok(Some(Source::Initialization { unit }))
            }
            StaticStorage(id) | StaticStorageRegistration(id) => {
                let explicit = self.initialization_support(provider, unit.unit())?;
                let singleton = id == unit.storage() && self.singleton(terminal, proof)?;
                if !explicit && !singleton {
                    return Err(Error::SupportRelation(subject));
                }
                let storages = terminal
                    .strong
                    .static_storage_registrations()
                    .registrations();

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

impl SharedSupport<'_, '_> {
    fn selected(&self, provider: ConeIdentity, target: Target) -> Result<bool, Error> {
        let selected = self.consumer.selected_relations();

        Ok(selected
            .binary_search(&mir::MirTypeBridgeDependencyV1::new(provider, target))
            .is_ok())
    }

    fn initialization_support(
        &self,
        provider: ConeIdentity,
        unit: PersistentInitializationUnitId,
    ) -> Result<bool, Error> {
        let uses = self.consumer.exports().initialization_uses().records();

        Ok(uses.iter().any(|edge| {
            edge.provider() == provider
                && edge.dependency_unit() == unit
                && edge.cause()
                    == mir::MirExternalInitializationCauseV1::InitializationSupport(unit)
        }))
    }

    fn singleton(
        &self,
        terminal: &PhysicalImportsReplayedCrossConeLayoutSections,
        proof: &mir::MirTypeBridgeInitializationUnitV1,
    ) -> Result<bool, Error> {
        let objects = terminal.mir.exports().objects().records();

        for object in objects {
            if object.unit() == proof.unit()
                && object.ensure() == proof.ensure()
                && self.selected(terminal.identity(), Target::Object(object.value()))?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn body(owner: StrongCallableDefinitionOwner) -> Result<PersistentCallableBodyId, Error> {
    let key = CallableBodyKey::strong(owner);

    PersistentCallableBodyId::from_key(&key).map_err(|_| Error::Contract)
}
