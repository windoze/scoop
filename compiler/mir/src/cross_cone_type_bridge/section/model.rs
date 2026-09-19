use super::*;

/// Complete MIR semantic proof. Artifact and machine-use eligibility still
/// require the containing LIR/Strong/profile closure.
pub struct CrossConeMirTypeBridgeSectionV1<'a> {
    pub(super) authority: MirTypeBridgeLocalAuthorityV1<'a>,
    pub(super) exports: MirTypeBridgeExportConstituentsV1,
    pub(super) units: Vec<MirTypeBridgeInitializationUnitV1>,
    pub(super) core_shapes: Vec<ParamFreeMirShapeSupportV1>,
    pub(super) legacy_callables: Vec<StrongCallableDefinitionOwner>,
    pub(super) dependencies: Vec<&'a CrossConeMirTypeBridgeSectionV1<'a>>,
    pub(super) selected: SelectedDependencyMirTypeSetV1<'a>,
}
impl<'a> CrossConeMirTypeBridgeSectionV1<'a> {
    pub const fn local_authority(&self) -> MirTypeBridgeLocalAuthorityV1<'a> {
        self.authority
    }
    pub const fn provider(&self) -> ConeIdentity {
        self.authority.provider()
    }
    pub const fn types(&self) -> &CanonicalParamFreeMirTypeExportsV1 {
        self.exports.types()
    }
    pub const fn callables(&self) -> &CanonicalMirCallableBindingsV1 {
        self.exports.callables()
    }
    pub const fn dispatch(&self) -> &CanonicalMirDispatchSchemasV1 {
        self.exports.dispatch()
    }
    pub const fn object_values(&self) -> &CanonicalMirObjectValuesV1 {
        self.exports.objects()
    }
    pub const fn shape_support(&self) -> &CanonicalMirShapeSupportsV1 {
        self.exports.shapes()
    }
    pub const fn initialization_uses(&self) -> &CanonicalMirExternalInitializationUsesV1 {
        self.exports.initialization_uses()
    }
    pub fn initialization_units(&self) -> &[MirTypeBridgeInitializationUnitV1] {
        &self.units
    }
    pub const fn selected(&self) -> &SelectedDependencyMirTypeSetV1<'a> {
        &self.selected
    }

    pub(super) fn record(
        &self,
        target: MirTypeBridgeTargetV1,
    ) -> Option<MirTypeBridgeSemanticRecordV1<'_>> {
        self.view().record(target)
    }
}

/// Read-only records reached through a checked semantic selection.
#[derive(Clone, Copy)]
pub enum MirTypeBridgeSemanticRecordV1<'a> {
    Type(&'a ParamFreeMirTypeExportV1),
    Callable(&'a ParamFreeMirCallableBindingV1),
    Dispatch(&'a ParamFreeMirDispatchSchemaV1),
    Object(&'a ParamFreeMirObjectValueV1),
    ShapeSupport(&'a ParamFreeMirShapeSupportV1),
    InitializationUnit(&'a MirTypeBridgeInitializationUnitV1),
}
impl MirTypeBridgeSemanticRecordV1<'_> {
    pub(super) fn references(
        self,
        graph: &ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        meter: &mut BudgetMeter,
    ) -> Result<MirTypeBridgeSemanticReferencesV1, MirTypeBridgeReferenceError> {
        match self {
            Self::Type(record) => MirTypeBridgeSemanticReferencesV1::of_type(record, graph, meter),
            Self::Callable(record) => {
                MirTypeBridgeSemanticReferencesV1::of_callable(record, graph, meter)
            }
            Self::Dispatch(record) => {
                MirTypeBridgeSemanticReferencesV1::of_dispatch(record, graph, types, meter)
            }
            Self::Object(record) => {
                MirTypeBridgeSemanticReferencesV1::of_object(record, graph, meter)
            }
            Self::ShapeSupport(record) => {
                MirTypeBridgeSemanticReferencesV1::of_shape(record, graph, meter)
            }
            Self::InitializationUnit(record) => {
                MirTypeBridgeSemanticReferencesV1::of_initialization_contract(
                    record.unit(),
                    record.signature(),
                    graph,
                    meter,
                )
            }
        }
    }
}
