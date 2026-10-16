use super::Fixture;
use crate::*;
use scoop_identity::*;
use std::sync::Arc;

impl PersistentIdResolver<PersistentExactTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.schema.resolve(id)
    }
}
impl PersistentIdResolver<PersistentDispatchSlotId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentDispatchSlotId>,
    ) -> Result<PersistentDispatchSlotId, Self::Error> {
        self.schema.resolve(id)
    }
}
impl PersistentIdResolver<PersistentTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        self.inheritance
            .keys
            .keys()
            .find_map(|key| match key {
                SourceNominalId::Concrete(known) if known.as_array() == id.as_array() => {
                    Some(*known)
                }
                _ => None,
            })
            .ok_or("unknown nominal")
    }
}
impl PersistentIdResolver<PersistentGenericTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        _: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err("generic owner is outside this fixture")
    }
}
impl PersistentIdResolver<PersistentFunctionId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFunctionId>,
    ) -> Result<PersistentFunctionId, Self::Error> {
        self.functions
            .keys()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown function")
    }
}
impl PersistentIdResolver<PersistentPropertyAccessorId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentPropertyAccessorId>,
    ) -> Result<PersistentPropertyAccessorId, Self::Error> {
        self.accessors
            .keys()
            .copied()
            .find(|known| known.as_array() == id.as_array())
            .ok_or("unknown accessor")
    }
}
impl PersistentIdResolver<ConeIdentity> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE).map_err(|_| "unknown cone")
    }
}
impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Fixture {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        let context = SourceContextKey::File {
            source: self.inheritance.origins[&self.unit.source]
                .origin()
                .source()
                .clone(),
        };
        id.verify(PersistentSourceContextId::from_key(&context).unwrap())
            .map_err(|_| "unknown context")?;
        Ok(Arc::new(context))
    }
}
