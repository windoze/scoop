use super::*;

/// Canonical owned type/callable/object/dispatch transport. Source and selection
/// agreement remain required before this can become a complete section.
pub struct CallablesResolvedCrossConeMirTypeBridgeSectionV1 {
    types: CanonicalParamFreeMirTypeExportsV1,
    callables: CanonicalMirCallableBindingsV1,
    dispatch: CanonicalMirDispatchSchemasV1,
    object_values: CanonicalMirObjectValuesV1,
    shape_support: CanonicalMirShapeSupportsV1,
    initialization_uses: DecodedCanonicalMirExternalInitializationUsesV1,
    selected: Vec<DecodedMirTypeBridgeDependencyV1>,
}

impl TypeResolvedCrossConeMirTypeBridgeSectionV1 {
    pub fn resolve_callables<'a, E>(
        self,
        foundation: &crate::OdrFreeMirFoundation,
        dependencies: impl ExactSizeIterator<
            Item = (
                &'a CanonicalParamFreeMirTypeExportsV1,
                &'a CanonicalMirCallableBindingsV1,
                &'a CanonicalMirDispatchSchemasV1,
            ),
        >,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CallablesResolvedCrossConeMirTypeBridgeSectionV1, MirTypeBridgeSectionError<E>>
    {
        let count = dependencies
            .len()
            .checked_add(1)
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let mut tables = reserve(count, meter)?;
        let mut callable_tables = reserve(count, meter)?;
        let mut schemas = reserve(count - 1, meter)?;
        tables.push(&self.types);
        for (types, callables, dispatch) in dependencies {
            tables.push(types);
            callable_tables.push(callables);
            schemas.push(dispatch);
        }
        let index = MirTypeBridgeTypeIndexV1::try_new(&tables, meter)?;
        let callables = self.callables.validate(graph, foundation, &index, meter)?;
        let object_values = self
            .object_values
            .validate(graph, &index, &callables, meter)?;
        callable_tables.push(&callables);
        let callable_index = MirTypeBridgeCallableIndexV1::try_new(&callable_tables, meter)?;
        let dispatch = self.dispatch.validate_with_dependencies(
            graph,
            &index,
            &callable_index,
            &schemas,
            meter,
        )?;
        Ok(CallablesResolvedCrossConeMirTypeBridgeSectionV1 {
            types: self.types,
            callables,
            dispatch,
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
    pub const fn dispatch(&self) -> &CanonicalMirDispatchSchemasV1 {
        &self.dispatch
    }

    pub fn resolve_dependencies<E>(
        self,
        authority: MirTypeBridgeLocalAuthorityV1<'_>,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<DependencyResolvedCrossConeMirTypeBridgeSectionV1, MirTypeBridgeSectionError<E>>
    {
        authority.validate(meter)?;
        let provider = authority.provider();
        let initialization_uses = self.initialization_uses.validate(provider, graph, meter)?;
        let mut selected = reserve(self.selected.len(), meter)?;
        for decoded in self.selected {
            selected.push(decoded.resolve(graph, meter)?);
        }
        build::validate_selected_records(provider, &selected, meter)?;
        Ok(DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
            provider,
            exports: MirTypeBridgeExportConstituentsV1::new(
                self.types,
                self.callables,
                self.dispatch,
                self.object_values,
                self.shape_support,
                initialization_uses,
            ),
            legacy: authority.legacy_callables(meter)?,
            selected,
        })
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
        let DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
            exports, selected, ..
        } = self.resolve_dependencies(authority, graph, meter)?;
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
