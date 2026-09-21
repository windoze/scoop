use super::*;

/// All export constituents are mandatory even when their inventory is empty.
/// This transport bundle is not a completed section or a selected provider.
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

/// Implemented by a stage from its independent checked source and MIR inputs.
/// Returning the candidate records as their own expected source is invalid.
pub trait MirTypeBridgeSourceSemanticAuthorityV1<E> {
    fn provider(&self) -> ConeIdentity;
    fn required_types(&self) -> Result<&[PersistentExactTypeId], E>;
    fn required_callables(&self) -> Result<&[StrongCallableDefinitionOwner], E>;
    fn required_dispatch(&self) -> Result<&[PersistentExactTypeId], E>;
    fn required_objects(&self) -> Result<&[PersistentObjectValueId], E>;
    fn required_source_roots(&self) -> Result<&[PersistentTypeId], E>;
    fn type_source(&self, exact: PersistentExactTypeId) -> Result<&ParamFreeMirTypeExportV1, E>;
    fn callable_source(
        &self,
        target: StrongCallableDefinitionOwner,
    ) -> Result<&ParamFreeMirCallableBindingV1, E>;
    fn dispatch_source(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&ParamFreeMirDispatchSchemaV1, E>;
    fn object_source(
        &self,
        value: PersistentObjectValueId,
    ) -> Result<&ParamFreeMirObjectValueV1, E>;
    fn committed_initialization_uses(&self)
    -> Result<&CanonicalMirExternalInitializationUsesV1, E>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirTypeBridgeSourceInventoryV1 {
    Types,
    Callables,
    Dispatch,
    Objects,
    SourceRoots,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirTypeBridgeSourceRecordV1 {
    Type(PersistentExactTypeId),
    Callable(StrongCallableDefinitionOwner),
    Dispatch(PersistentExactTypeId),
    Object(PersistentObjectValueId),
    InitializationUses,
}

/// A checked local source join. It grants no terminal-provider eligibility.
#[derive(Clone, Copy)]
pub struct CheckedMirTypeBridgeSourceJoinV1<'a> {
    pub(super) provider: ConeIdentity,
    pub(super) exports: &'a MirTypeBridgeExportConstituentsV1,
}
impl CheckedMirTypeBridgeSourceJoinV1<'_> {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
    pub const fn exports(&self) -> &MirTypeBridgeExportConstituentsV1 {
        self.exports
    }
}
