use super::*;
use crate::cross_cone_type_bridge::tests::support::Fixture;
use scoop_identity::{CallableOwner, CallableTemplateOwner, SourceNominalKind};
use scoop_wire::DecodeLimits;

mod dispatch;
mod objects;
mod records;
mod roots;
mod tables;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

struct Source {
    fixture: Fixture,
    provider: ConeIdentity,
    expected: CanonicalParamFreeMirTypeExportsV1,
    required_types: Vec<PersistentExactTypeId>,
    required_callables: Vec<StrongCallableDefinitionOwner>,
    required_dispatch: Vec<PersistentExactTypeId>,
    required_objects: Vec<PersistentObjectValueId>,
    roots: Vec<PersistentTypeId>,
    uses: CanonicalMirExternalInitializationUsesV1,
}
impl Source {
    fn new(provider: ConeIdentity) -> Self {
        let fixture = Fixture::with_source_provider("Empty", SourceNominalKind::Struct, provider);
        let expected = Self::types(&fixture);
        Self {
            provider,
            required_types: expected.records().iter().map(|r| r.exact()).collect(),
            required_callables: vec![],
            required_dispatch: vec![],
            required_objects: vec![],
            roots: vec![fixture.empty.id()],
            uses: CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter()).unwrap(),
            fixture,
            expected,
        }
    }
    fn types(fixture: &Fixture) -> CanonicalParamFreeMirTypeExportsV1 {
        CanonicalParamFreeMirTypeExportsV1::try_new(vec![
            fixture.empty_export(),
            fixture.boxed_export(),
            fixture.step_export(),
            fixture.slot_export(),
        ])
        .unwrap()
    }
    fn exports(&self) -> MirTypeBridgeExportConstituentsV1 {
        let types = Self::types(&self.fixture);
        let shape_authority = MirShapeSupportAuthority {
            identities: &self.fixture.graph,
            types: &types,
        };
        let shapes = if self.provider == ConeIdentity::CORE {
            vec![]
        } else {
            vec![
                ParamFreeMirShapeSupportV1::try_new(
                    shape_authority,
                    self.fixture.empty.id(),
                    self.fixture.payload.id(),
                    MirBoxedShapeSupportV1::Available(self.fixture.boxed_export().exact()),
                    self.fixture.step_export().exact(),
                    self.fixture.slot_export().exact(),
                    &mut meter(),
                )
                .unwrap(),
            ]
        };
        let shapes = CanonicalMirShapeSupportsV1::try_new(
            self.provider,
            shape_authority,
            shapes,
            &mut meter(),
        )
        .unwrap();
        let callables = CanonicalMirCallableBindingsV1::try_new(vec![]).unwrap();
        let dispatch = CanonicalMirDispatchSchemasV1::try_new(
            MirDispatchSchemaAuthority {
                identities: &self.fixture.graph,
                types: &types,
                callables: &callables,
            },
            vec![],
            &mut meter(),
        )
        .unwrap();
        MirTypeBridgeExportConstituentsV1::new(
            types,
            callables,
            dispatch,
            CanonicalMirObjectValuesV1::try_new(vec![], &mut meter()).unwrap(),
            shapes,
            CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter()).unwrap(),
        )
    }
    fn frame_owner(&self) -> scoop_identity::PersistentFunctionId {
        let GeneratedNominalKey::CoroutineFrame { source_callable } = self.fixture.frame.key()
        else {
            unreachable!()
        };
        let CallableTemplateOwner::Function(id) = source_callable.template() else {
            unreachable!()
        };
        id
    }
    fn core_bridge(&self) -> crate::CoreMirBridgeV1 {
        crate::CoreMirBridgeV1::try_new(
            vec![
                crate::CoreMirShapeSupportRootV1::new(
                    self.fixture.empty.id(),
                    self.fixture.payload.id(),
                )
                .unwrap(),
            ],
            crate::CoreMirInitializationCycleThrowerV1::new(
                self.frame_owner(),
                CallableOwner::Function(self.frame_owner()),
            )
            .unwrap(),
        )
        .unwrap()
    }
}
impl MirTypeBridgeSourceSemanticAuthorityV1<&'static str> for Source {
    fn provider(&self) -> ConeIdentity {
        self.provider
    }
    fn required_types(&self) -> Result<&[PersistentExactTypeId], &'static str> {
        Ok(&self.required_types)
    }
    fn required_callables(&self) -> Result<&[StrongCallableDefinitionOwner], &'static str> {
        Ok(&self.required_callables)
    }
    fn required_dispatch(&self) -> Result<&[PersistentExactTypeId], &'static str> {
        Ok(&self.required_dispatch)
    }
    fn required_objects(&self) -> Result<&[PersistentObjectValueId], &'static str> {
        Ok(&self.required_objects)
    }
    fn required_source_roots(&self) -> Result<&[PersistentTypeId], &'static str> {
        Ok(&self.roots)
    }
    fn type_source(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirTypeExportV1, &'static str> {
        self.expected.get(exact).ok_or("source type is absent")
    }
    fn callable_source(
        &self,
        _: StrongCallableDefinitionOwner,
    ) -> Result<&ParamFreeMirCallableBindingV1, &'static str> {
        Err("source has no callable exports")
    }
    fn dispatch_source(
        &self,
        _: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirDispatchSchemaV1, &'static str> {
        Err("source has no dispatch exports")
    }
    fn object_source(
        &self,
        _: PersistentObjectValueId,
    ) -> Result<&ParamFreeMirObjectValueV1, &'static str> {
        Err("source has no object exports")
    }
    fn committed_initialization_uses(
        &self,
    ) -> Result<&CanonicalMirExternalInitializationUsesV1, &'static str> {
        Ok(&self.uses)
    }
}
