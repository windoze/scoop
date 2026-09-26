use super::*;
pub(super) use crate::cross_cone_type_bridge::tests::support::Fixture as TypeFixture;
use scoop_identity::{ConeCoordinate, PendingIdentityValidation, SourceNominalKind};

pub(super) struct Fixture {
    pub types: TypeFixture,
    pub provider: ConeIdentity,
    pub exports: MirTypeBridgeExportConstituentsV1,
    pub uses: Vec<MirTypeBridgeDependencyV1>,
    pub units: Vec<MirTypeBridgeInitializationUnitV1>,
    pub production: crate::CoreBootstrapBridgeSectionV1,
    pub ordinary: crate::CrossConeMirBridgeSectionV1,
}
impl Fixture {
    pub fn new(name: &str) -> Self {
        let provider = ConeCoordinate::new("section.tests", name, "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        Self::in_provider(provider)
    }
    pub fn in_provider(provider: ConeIdentity) -> Self {
        let types = TypeFixture::with_source_provider("Empty", SourceNominalKind::Struct, provider);
        let table = CanonicalParamFreeMirTypeExportsV1::try_new(vec![
            types.empty_export(),
            types.boxed_export(),
            types.step_export(),
            types.slot_export(),
        ])
        .unwrap();
        let exports = exports(&types, provider, table);
        let production = production(provider, &types.foundation);
        let ordinary = crate::CrossConeMirBridgeSectionV1::try_new(
            provider,
            &types.foundation,
            vec![],
            vec![],
        )
        .unwrap();
        Self {
            types,
            provider,
            exports,
            uses: Vec::new(),
            units: Vec::new(),
            production,
            ordinary,
        }
    }
    pub fn authority(&self) -> MirTypeBridgeLocalInputV1<'_> {
        MirTypeBridgeLocalInputV1 {
            provider: self.provider,

            production: &self.production,
            ordinary: &self.ordinary,
        }
    }
    pub fn section<'a>(
        &'a self,
        dependencies: &[MirTypeBridgeDependencyViewV1<'a>],
        graph: &ValidatedIdentityGraph,
    ) -> Result<CrossConeMirTypeBridgeSectionV1<'a>, MirTypeBridgeSectionError> {
        CrossConeMirTypeBridgeSectionV1::try_new(
            self.authority(),
            self.exports.clone(),
            self.units.clone(),
            dependencies,
            &self.uses,
            graph,
        )
    }
    pub fn type_use(&self) -> MirTypeBridgeDependencyV1 {
        MirTypeBridgeDependencyV1::new(
            self.provider,
            MirTypeBridgeTargetV1::Type(self.types.payload.id()),
        )
    }
    pub fn shape_use(&self) -> MirTypeBridgeDependencyV1 {
        MirTypeBridgeDependencyV1::new(
            self.provider,
            MirTypeBridgeTargetV1::ShapeSupport(self.types.empty.id()),
        )
    }
    pub fn with_field(
        &mut self,
        field_type: PersistentExactTypeId,
        graph: &ValidatedIdentityGraph,
    ) {
        let value = ParamFreeMirTypeExportV1::try_new(
            MirTypeBridgeAuthority {
                identities: graph,
                foundation: &self.types.foundation,
            },
            self.types.payload.id(),
            MirTypeOriginV1::SourceNominal(self.types.empty.id()),
            self.types.empty_export().facts(),
            MirTypeRepresentationV1::Struct {
                fields: vec![MirRepresentationFieldV1 {
                    field: self.types.fields[0].id(),
                    value: field_type,
                }],
                c_layout: MirTypeCLayoutPolicyV1::Ordinary,
                interior_mutable: false,
            },
            self.types.empty_export().base_and_interfaces().clone(),
        )
        .unwrap();
        let table = CanonicalParamFreeMirTypeExportsV1::try_new(vec![
            value,
            self.types.boxed_export(),
            self.types.step_export(),
            self.types.slot_export(),
        ])
        .unwrap();
        self.exports = exports(&self.types, self.provider, table);
    }
}
pub(super) fn exports(
    fixture: &TypeFixture,
    provider: ConeIdentity,
    types: CanonicalParamFreeMirTypeExportsV1,
) -> MirTypeBridgeExportConstituentsV1 {
    let shape_authority = MirShapeSupportAuthority {
        identities: &fixture.graph,
        types: &types,
    };
    let shape = ParamFreeMirShapeSupportV1::try_new(
        shape_authority,
        fixture.empty.id(),
        fixture.payload.id(),
        MirBoxedShapeSupportV1::Available(fixture.boxed_export().exact()),
        fixture.step_export().exact(),
        fixture.slot_export().exact(),
    )
    .unwrap();
    let shapes =
        CanonicalMirShapeSupportsV1::try_new(provider, shape_authority, vec![shape]).unwrap();
    let callables = CanonicalMirCallableBindingsV1::try_new(vec![]).unwrap();
    let dispatch = CanonicalMirDispatchSchemasV1::try_new(
        MirDispatchSchemaAuthority {
            identities: &fixture.graph,
            types: &types,
            callables: &callables,
        },
        vec![],
    )
    .unwrap();
    MirTypeBridgeExportConstituentsV1::new(
        types,
        callables,
        dispatch,
        CanonicalMirObjectValuesV1::try_new(vec![]).unwrap(),
        shapes,
        CanonicalMirExternalInitializationUsesV1::try_new(vec![]).unwrap(),
    )
}
pub(super) fn graph(fixtures: &[&Fixture]) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    for fixture in fixtures {
        pending
            .register_external_graph_authorities(&fixture.types.graph)
            .unwrap();
    }
    pending.finish().unwrap()
}
pub(super) fn production(
    provider: ConeIdentity,
    foundation: &crate::OdrFreeMirFoundation,
) -> crate::CoreBootstrapBridgeSectionV1 {
    crate::CoreBootstrapBridgeSectionV1::try_new(
        provider,
        crate::EntryMirBridgeBranchV1::Library,
        crate::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(foundation),
    )
    .unwrap()
}
