use super::*;
use MirTypeBridgeSourceProjectionError as Error;
use mir::MirTypeBridgeSourceRecordV1 as Record;

impl mir::MirTypeBridgeSourceSemanticAuthorityV1<Error> for MirTypeBridgeSourceProjectionV1 {
    fn provider(&self) -> ConeIdentity {
        self.provider
    }
    fn required_types(&self) -> Result<&[PersistentExactTypeId], Error> {
        Ok(&self.inventory.types)
    }
    fn required_callables(&self) -> Result<&[StrongCallableDefinitionOwner], Error> {
        Ok(&self.inventory.callables)
    }
    fn required_dispatch(&self) -> Result<&[PersistentExactTypeId], Error> {
        Ok(&self.inventory.dispatch)
    }
    fn required_objects(&self) -> Result<&[PersistentObjectValueId], Error> {
        Ok(&self.inventory.objects)
    }
    fn required_source_roots(&self) -> Result<&[PersistentTypeId], Error> {
        Ok(&self.inventory.roots)
    }
    fn type_source(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&mir::ParamFreeMirTypeExportV1, Error> {
        self.expected
            .types()
            .get(exact)
            .ok_or(Error::MissingSource(Record::Type(exact)))
    }
    fn callable_source(
        &self,
        target: StrongCallableDefinitionOwner,
    ) -> Result<&mir::ParamFreeMirCallableBindingV1, Error> {
        self.expected
            .callables()
            .get(target)
            .ok_or(Error::MissingSource(Record::Callable(target)))
    }
    fn dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&mir::ParamFreeMirDispatchSchemaV1, Error> {
        self.expected
            .dispatch()
            .get(owner)
            .ok_or(Error::MissingSource(Record::Dispatch(owner)))
    }
    fn object_source(
        &self,
        value: PersistentObjectValueId,
    ) -> Result<&mir::ParamFreeMirObjectValueV1, Error> {
        self.expected
            .objects()
            .get(value)
            .ok_or(Error::MissingSource(Record::Object(value)))
    }
    fn committed_initialization_uses(
        &self,
    ) -> Result<&mir::CanonicalMirExternalInitializationUsesV1, Error> {
        Ok(self.expected.initialization_uses())
    }
}

impl mir::MirTypeBridgeSectionSourceAuthorityV1<Error> for MirTypeBridgeSourceProjectionV1 {
    fn committed_external_uses(&self) -> Result<&[mir::MirTypeBridgeDependencyV1], Error> {
        Ok(&self.uses)
    }

    fn local_initialization_units(&self) -> Result<&[PersistentInitializationUnitId], Error> {
        Ok(&self.units.inventory)
    }

    fn initialization_signature(
        &self,
        unit: PersistentInitializationUnitId,
        role: scoop_identity::InitializationCallableRole,
    ) -> Result<&mir::MirBridgeCallableSignatureV1, Error> {
        self.units.signature(unit, role)
    }
}
