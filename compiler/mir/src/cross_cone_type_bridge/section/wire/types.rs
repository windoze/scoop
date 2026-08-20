use super::*;

/// Owned intermediate transport. Type identities and finite shape relations
/// have been resolved; source replay and the remaining section are unvalidated.
pub struct TypeResolvedCrossConeMirTypeBridgeSectionV1 {
    pub(super) types: CanonicalParamFreeMirTypeExportsV1,
    pub(super) callables: DecodedCanonicalMirCallableBindingsV1,
    pub(super) dispatch: DecodedCanonicalMirDispatchSchemasV1,
    pub(super) object_values: DecodedCanonicalMirObjectValuesV1,
    pub(super) shape_support: CanonicalMirShapeSupportsV1,
    pub(super) initialization_uses: DecodedCanonicalMirExternalInitializationUsesV1,
    pub(super) selected: Vec<DecodedMirTypeBridgeDependencyV1>,
}

impl DecodedCrossConeMirTypeBridgeSectionV1 {
    pub fn resolve_types<'a>(
        self,
        provider: ConeIdentity,
        foundation: &crate::CanonicalMirFoundation,
        dependencies: impl ExactSizeIterator<Item = &'a CanonicalParamFreeMirTypeExportsV1>,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<TypeResolvedCrossConeMirTypeBridgeSectionV1, MirTypeBridgeSectionError> {
        let types = self.types.validate(graph, foundation)?;
        let shape_support = {
            let count = dependencies
                .len()
                .checked_add(1)
                .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
            let mut tables = reserve(count)?;
            tables.push(&types);
            for dependency in dependencies {
                tables.push(dependency);
            }
            let index = MirTypeBridgeTypeIndexV1::try_new(&tables)?;
            self.shape_support.validate(provider, graph, &index)?
        };
        Ok(TypeResolvedCrossConeMirTypeBridgeSectionV1 {
            types,
            callables: self.callables,
            dispatch: self.dispatch,
            object_values: self.object_values,
            shape_support,
            initialization_uses: self.initialization_uses,
            selected: self.selected,
        })
    }
}

impl TypeResolvedCrossConeMirTypeBridgeSectionV1 {
    pub const fn types(&self) -> &CanonicalParamFreeMirTypeExportsV1 {
        &self.types
    }
    pub const fn shape_support(&self) -> &CanonicalMirShapeSupportsV1 {
        &self.shape_support
    }
}
