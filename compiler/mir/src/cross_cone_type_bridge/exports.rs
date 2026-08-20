use super::*;

/// All export constituents are mandatory even when their inventory is empty.
#[derive(Clone)]
pub struct MirTypeBridgeExportConstituentsV1 {
    pub(super) types: CanonicalParamFreeMirTypeExportsV1,
    pub(super) callables: CanonicalMirCallableBindingsV1,
    pub(super) dispatch: CanonicalMirDispatchSchemasV1,
    pub(super) objects: CanonicalMirObjectValuesV1,
    pub(super) shapes: CanonicalMirShapeSupportsV1,
    pub(super) initialization_uses: CanonicalMirExternalInitializationUsesV1,
}
impl MirTypeBridgeExportConstituentsV1 {
    pub fn new(
        types: CanonicalParamFreeMirTypeExportsV1,
        callables: CanonicalMirCallableBindingsV1,
        dispatch: CanonicalMirDispatchSchemasV1,
        objects: CanonicalMirObjectValuesV1,
        shapes: CanonicalMirShapeSupportsV1,
        initialization_uses: CanonicalMirExternalInitializationUsesV1,
    ) -> Self {
        Self {
            types,
            callables,
            dispatch,
            objects,
            shapes,
            initialization_uses,
        }
    }
    pub const fn types(&self) -> &CanonicalParamFreeMirTypeExportsV1 {
        &self.types
    }
    pub const fn callables(&self) -> &CanonicalMirCallableBindingsV1 {
        &self.callables
    }
    pub const fn dispatch(&self) -> &CanonicalMirDispatchSchemasV1 {
        &self.dispatch
    }
    pub const fn objects(&self) -> &CanonicalMirObjectValuesV1 {
        &self.objects
    }
    pub const fn shapes(&self) -> &CanonicalMirShapeSupportsV1 {
        &self.shapes
    }
    pub const fn initialization_uses(&self) -> &CanonicalMirExternalInitializationUsesV1 {
        &self.initialization_uses
    }
}
