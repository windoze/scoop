use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBoxedShapeSupportV1 {
    Available(PersistentExactTypeId),
    ReferenceNominalRequiresNoBox,
}

/// A complete semantic family; physical definitions are proven by LIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeMirShapeSupportV1 {
    pub(super) source: PersistentTypeId,
    pub(super) exact: PersistentExactTypeId,
    pub(super) boxed: MirBoxedShapeSupportV1,
    pub(super) coroutine_step: PersistentExactTypeId,
    pub(super) coroutine_slot: PersistentExactTypeId,
    pub(super) provider: ConeIdentity,
}
impl ParamFreeMirShapeSupportV1 {
    pub fn try_new(
        authority: MirShapeSupportAuthority<'_>,
        source: PersistentTypeId,
        exact: PersistentExactTypeId,
        boxed: MirBoxedShapeSupportV1,
        coroutine_step: PersistentExactTypeId,
        coroutine_slot: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirShapeSupportError> {
        meter.charge_work(1, &WirePath::root())?;
        let provider = authority
            .identities
            .canonical_key::<_, SourceDeclarationKey>(source)?
            .origin();
        let record = Self {
            source,
            exact,
            boxed,
            coroutine_step,
            coroutine_slot,
            provider,
        };
        authority.validate(&record, meter)?;
        Ok(record)
    }
    pub const fn source(&self) -> PersistentTypeId {
        self.source
    }
    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }
    pub const fn boxed(&self) -> MirBoxedShapeSupportV1 {
        self.boxed
    }
    pub const fn coroutine_step(&self) -> PersistentExactTypeId {
        self.coroutine_step
    }
    pub const fn coroutine_slot(&self) -> PersistentExactTypeId {
        self.coroutine_slot
    }
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
}

#[derive(Clone, Copy)]
pub struct MirShapeSupportAuthority<'a> {
    pub identities: &'a ValidatedIdentityGraph,
    pub types: &'a dyn MirTypeBridgeTypeLookupV1,
}
