use super::*;

pub(super) struct Source {
    pub provider: ConeIdentity,
    pub expected: MirTypeBridgeExportConstituentsV1,
    pub types: Vec<PersistentExactTypeId>,
    pub callables: Vec<StrongCallableDefinitionOwner>,
    pub dispatch: Vec<PersistentExactTypeId>,
    pub objects: Vec<PersistentObjectValueId>,
    pub roots: Vec<PersistentTypeId>,
    pub uses: Vec<MirTypeBridgeDependencyV1>,
    pub units: Vec<PersistentInitializationUnitId>,
    pub signatures: Vec<(
        PersistentInitializationUnitId,
        MirBridgeCallableSignatureV1,
        MirBridgeCallableSignatureV1,
    )>,
}
impl Source {
    pub fn new(
        provider: ConeIdentity,
        roots: Vec<PersistentTypeId>,
        expected: MirTypeBridgeExportConstituentsV1,
    ) -> Self {
        Self {
            provider,
            roots,
            types: expected
                .types()
                .records()
                .iter()
                .map(|r| r.exact())
                .collect(),
            callables: expected
                .callables()
                .entries()
                .iter()
                .map(|r| r.implementation())
                .collect(),
            dispatch: expected
                .dispatch()
                .records()
                .iter()
                .map(|r| r.owner())
                .collect(),
            objects: expected
                .objects()
                .records()
                .iter()
                .map(|r| r.value())
                .collect(),
            expected,
            uses: vec![],
            units: vec![],
            signatures: vec![],
        }
    }
    pub fn exports(&self) -> MirTypeBridgeExportConstituentsV1 {
        copy_exports(&self.expected)
    }
}
pub(super) fn copy_exports(
    source: &MirTypeBridgeExportConstituentsV1,
) -> MirTypeBridgeExportConstituentsV1 {
    MirTypeBridgeExportConstituentsV1::new(
        source.types().clone(),
        source.callables().clone(),
        source.dispatch().clone(),
        source.objects().clone(),
        source.shapes().clone(),
        source.initialization_uses().clone(),
    )
}
impl MirTypeBridgeSourceSemanticAuthorityV1<&'static str> for Source {
    fn provider(&self) -> ConeIdentity {
        self.provider
    }
    fn required_types(&self) -> Result<&[PersistentExactTypeId], &'static str> {
        Ok(&self.types)
    }
    fn required_callables(&self) -> Result<&[StrongCallableDefinitionOwner], &'static str> {
        Ok(&self.callables)
    }
    fn required_dispatch(&self) -> Result<&[PersistentExactTypeId], &'static str> {
        Ok(&self.dispatch)
    }
    fn required_objects(&self) -> Result<&[PersistentObjectValueId], &'static str> {
        Ok(&self.objects)
    }
    fn required_source_roots(&self) -> Result<&[PersistentTypeId], &'static str> {
        Ok(&self.roots)
    }
    fn type_source(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirTypeExportV1, &'static str> {
        self.expected
            .types()
            .get(exact)
            .ok_or("missing source type")
    }
    fn callable_source(
        &self,
        target: StrongCallableDefinitionOwner,
    ) -> Result<&ParamFreeMirCallableBindingV1, &'static str> {
        self.expected
            .callables()
            .get(target)
            .ok_or("missing source callable")
    }
    fn dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirDispatchSchemaV1, &'static str> {
        self.expected
            .dispatch()
            .get(owner)
            .ok_or("missing source dispatch")
    }
    fn object_source(
        &self,
        value: PersistentObjectValueId,
    ) -> Result<&ParamFreeMirObjectValueV1, &'static str> {
        self.expected
            .objects()
            .get(value)
            .ok_or("missing source object")
    }
    fn committed_initialization_uses(
        &self,
    ) -> Result<&CanonicalMirExternalInitializationUsesV1, &'static str> {
        Ok(self.expected.initialization_uses())
    }
}
impl MirTypeBridgeSectionSourceAuthorityV1<&'static str> for Source {
    fn committed_external_uses(&self) -> Result<&[MirTypeBridgeDependencyV1], &'static str> {
        Ok(&self.uses)
    }
    fn local_initialization_units(
        &self,
    ) -> Result<&[PersistentInitializationUnitId], &'static str> {
        Ok(&self.units)
    }
    fn initialization_signature(
        &self,
        unit: PersistentInitializationUnitId,
        role: InitializationCallableRole,
    ) -> Result<&MirBridgeCallableSignatureV1, &'static str> {
        let (_, initializer, ensure) = self
            .signatures
            .iter()
            .find(|row| row.0 == unit)
            .ok_or("missing source unit")?;
        Ok(match role {
            InitializationCallableRole::Initializer => initializer,
            InitializationCallableRole::Ensure => ensure,
        })
    }
}
