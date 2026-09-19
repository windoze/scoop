use super::*;

/// Expected inventories come from a separately built source fixture.
pub(super) struct SourceTables {
    pub expected: MirTypeBridgeExportConstituentsV1,
    pub types: Vec<PersistentExactTypeId>,
    callables: Vec<StrongCallableDefinitionOwner>,
    dispatch: Vec<PersistentExactTypeId>,
    objects: Vec<PersistentObjectValueId>,
    pub object_override: Option<ParamFreeMirObjectValueV1>,
}
impl SourceTables {
    pub fn new(expected: MirTypeBridgeExportConstituentsV1) -> Self {
        Self {
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
            object_override: None,
        }
    }
    pub fn check(
        &self,
        candidate: &MirTypeBridgeExportConstituentsV1,
        graph: &ValidatedIdentityGraph,
    ) -> Result<(), MirTypeBridgeSourceJoinError<&'static str>> {
        candidate
            .validate_sources(
                ConeIdentity::SINGLE_FILE,
                graph,
                MirTypeBridgeShapeRootAuthorityV1::Ordinary,
                self,
                &mut meter(),
            )
            .map(|_| ())
    }
}
impl MirTypeBridgeSourceSemanticAuthorityV1<&'static str> for SourceTables {
    fn provider(&self) -> ConeIdentity {
        ConeIdentity::SINGLE_FILE
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
        Ok(&[])
    }
    fn type_source(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirTypeExportV1, &'static str> {
        self.expected
            .types()
            .get(exact)
            .ok_or("unknown source type")
    }
    fn callable_source(
        &self,
        target: StrongCallableDefinitionOwner,
    ) -> Result<&ParamFreeMirCallableBindingV1, &'static str> {
        self.expected
            .callables()
            .get(target)
            .ok_or("unknown source callable")
    }
    fn dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirDispatchSchemaV1, &'static str> {
        self.expected
            .dispatch()
            .get(owner)
            .ok_or("unknown source dispatch")
    }
    fn object_source(
        &self,
        value: PersistentObjectValueId,
    ) -> Result<&ParamFreeMirObjectValueV1, &'static str> {
        self.object_override
            .as_ref()
            .or_else(|| self.expected.objects().get(value))
            .ok_or("unknown source object")
    }
    fn committed_initialization_uses(
        &self,
    ) -> Result<&CanonicalMirExternalInitializationUsesV1, &'static str> {
        Ok(self.expected.initialization_uses())
    }
}

pub(super) fn empty_shapes(
    graph: &ValidatedIdentityGraph,
    types: &CanonicalParamFreeMirTypeExportsV1,
) -> CanonicalMirShapeSupportsV1 {
    CanonicalMirShapeSupportsV1::try_new(
        ConeIdentity::SINGLE_FILE,
        MirShapeSupportAuthority {
            identities: graph,
            types,
        },
        vec![],
        &mut meter(),
    )
    .unwrap()
}
