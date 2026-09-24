use super::*;

/// Canonical owned type/callable/object transport. Source and dispatch/selection
/// agreement remain required before this can become a complete section.
pub struct CallablesResolvedCrossConeMirTypeBridgeSectionV1 {
    types: CanonicalParamFreeMirTypeExportsV1,
    callables: CanonicalMirCallableBindingsV1,
    dispatch: DecodedCanonicalMirDispatchSchemasV1,
    object_values: CanonicalMirObjectValuesV1,
    shape_support: CanonicalMirShapeSupportsV1,
    initialization_uses: DecodedCanonicalMirExternalInitializationUsesV1,
    selected: Vec<DecodedMirTypeBridgeDependencyV1>,
}

impl TypeResolvedCrossConeMirTypeBridgeSectionV1 {
    pub fn resolve_callables<'a, E>(
        self,
        foundation: &crate::OdrFreeMirFoundation,
        dependencies: impl ExactSizeIterator<Item = &'a CanonicalParamFreeMirTypeExportsV1>,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CallablesResolvedCrossConeMirTypeBridgeSectionV1, MirTypeBridgeSectionError<E>>
    {
        let count = dependencies
            .len()
            .checked_add(1)
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let mut tables = reserve(count, meter)?;
        tables.push(&self.types);
        for dependency in dependencies {
            tables.push(dependency);
        }
        let index = MirTypeBridgeTypeIndexV1::try_new(&tables, meter)?;
        let callables = self.callables.validate(graph, foundation, &index, meter)?;
        let object_values = self
            .object_values
            .validate(graph, &index, &callables, meter)?;
        Ok(CallablesResolvedCrossConeMirTypeBridgeSectionV1 {
            types: self.types,
            callables,
            dispatch: self.dispatch,
            object_values,
            shape_support: self.shape_support,
            initialization_uses: self.initialization_uses,
            selected: self.selected,
        })
    }
}

impl CallablesResolvedCrossConeMirTypeBridgeSectionV1 {
    pub const fn types(&self) -> &CanonicalParamFreeMirTypeExportsV1 {
        &self.types
    }
    pub const fn callables(&self) -> &CanonicalMirCallableBindingsV1 {
        &self.callables
    }
    pub const fn shape_support(&self) -> &CanonicalMirShapeSupportsV1 {
        &self.shape_support
    }
    pub const fn object_values(&self) -> &CanonicalMirObjectValuesV1 {
        &self.object_values
    }

    pub fn validate<'a, E>(
        self,
        authority: MirTypeBridgeLocalAuthorityV1<'a>,
        dependencies: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
        source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeMirTypeBridgeSectionV1<'a>, MirTypeBridgeSectionError<E>> {
        let dependencies = dependencies::complete(authority.provider(), dependencies, meter)?;
        self.complete(authority, dependencies, source, graph, meter)
    }

    pub(super) fn complete<'a, E>(
        self,
        authority: MirTypeBridgeLocalAuthorityV1<'a>,
        dependencies: Vec<&'a CrossConeMirTypeBridgeSectionV1<'a>>,
        source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeMirTypeBridgeSectionV1<'a>, MirTypeBridgeSectionError<E>> {
        let type_index = dependencies::types(&self.types, &dependencies, meter)?;
        let count = dependencies
            .len()
            .checked_add(1)
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let mut tables = reserve(count, meter)?;
        tables.push(&self.callables);
        tables.extend(dependencies.iter().map(|section| section.callables()));
        let callables = MirTypeBridgeCallableIndexV1::try_new(&tables, meter)?;
        let mut schemas = reserve(dependencies.len(), meter)?;
        schemas.extend(dependencies.iter().map(|section| section.dispatch()));
        let dispatch = self.dispatch.validate_with_dependencies(
            graph,
            &type_index,
            &callables,
            &schemas,
            meter,
        )?;
        let initialization_uses =
            self.initialization_uses
                .validate(authority.provider(), graph, meter)?;
        let mut selected = reserve(self.selected.len(), meter)?;
        for decoded in self.selected {
            selected.push(decoded.resolve(graph, meter)?);
        }
        let exports = MirTypeBridgeExportConstituentsV1::new(
            self.types,
            self.callables,
            dispatch,
            self.object_values,
            self.shape_support,
            initialization_uses,
        );
        build::complete(
            build::SectionInput {
                authority,
                exports,
                dependencies,
                selection: build::SelectionInput::Reader(selected),
            },
            source,
            graph,
            meter,
        )
    }
}
