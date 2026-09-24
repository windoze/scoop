use super::*;

/// Owned intermediate transport. Type identities and finite shape relations
/// have been resolved; source replay and the remaining section are unvalidated.
pub struct TypeResolvedCrossConeMirTypeBridgeSectionV1 {
    types: CanonicalParamFreeMirTypeExportsV1,
    callables: DecodedCanonicalMirCallableBindingsV1,
    dispatch: DecodedCanonicalMirDispatchSchemasV1,
    object_values: DecodedCanonicalMirObjectValuesV1,
    shape_support: CanonicalMirShapeSupportsV1,
    initialization_uses: DecodedCanonicalMirExternalInitializationUsesV1,
    selected: Vec<DecodedMirTypeBridgeDependencyV1>,
}

impl DecodedCrossConeMirTypeBridgeSectionV1 {
    pub fn resolve_types<'a, E>(
        self,
        provider: ConeIdentity,
        foundation: &crate::OdrFreeMirFoundation,
        dependencies: impl ExactSizeIterator<Item = &'a CanonicalParamFreeMirTypeExportsV1>,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<TypeResolvedCrossConeMirTypeBridgeSectionV1, MirTypeBridgeSectionError<E>> {
        let types = self.types.validate(graph, foundation, meter)?;
        let shape_support = {
            let count = dependencies
                .len()
                .checked_add(1)
                .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
            let mut tables = reserve(count, meter)?;
            tables.push(&types);
            for dependency in dependencies {
                tables.push(dependency);
            }
            let index = MirTypeBridgeTypeIndexV1::try_new(&tables, meter)?;
            self.shape_support
                .validate(provider, graph, &index, meter)?
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

    pub fn validate<'a, E>(
        self,
        authority: MirTypeBridgeLocalAuthorityV1<'a>,
        dependencies: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
        source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeMirTypeBridgeSectionV1<'a>, MirTypeBridgeSectionError<E>> {
        let dependencies = dependencies::complete(authority.provider(), dependencies, meter)?;
        let types = self.resolve_types(
            authority.provider(),
            authority.foundation(),
            dependencies.iter().map(|section| section.types()),
            graph,
            meter,
        )?;
        types.complete(authority, dependencies, source, graph, meter)
    }
}

impl TypeResolvedCrossConeMirTypeBridgeSectionV1 {
    pub const fn types(&self) -> &CanonicalParamFreeMirTypeExportsV1 {
        &self.types
    }
    pub const fn shape_support(&self) -> &CanonicalMirShapeSupportsV1 {
        &self.shape_support
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

    fn complete<'a, E>(
        self,
        authority: MirTypeBridgeLocalAuthorityV1<'a>,
        dependencies: Vec<&'a CrossConeMirTypeBridgeSectionV1<'a>>,
        source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeMirTypeBridgeSectionV1<'a>, MirTypeBridgeSectionError<E>> {
        let type_index = dependencies::types(&self.types, &dependencies, meter)?;
        let callables =
            self.callables
                .validate(graph, authority.foundation(), &type_index, meter)?;
        let count = dependencies
            .len()
            .checked_add(1)
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let mut callable_tables = reserve(count, meter)?;
        callable_tables.push(&callables);
        callable_tables.extend(dependencies.iter().map(|section| section.callables()));
        let callable_index = MirTypeBridgeCallableIndexV1::try_new(&callable_tables, meter)?;
        let mut schemas = reserve(dependencies.len(), meter)?;
        schemas.extend(dependencies.iter().map(|section| section.dispatch()));
        let dispatch = self.dispatch.validate_with_dependencies(
            graph,
            &type_index,
            &callable_index,
            &schemas,
            meter,
        )?;
        let object_values =
            self.object_values
                .validate(graph, &type_index, &callable_index, meter)?;
        let initialization_uses =
            self.initialization_uses
                .validate(authority.provider(), graph, meter)?;
        let mut selected = reserve(self.selected.len(), meter)?;
        for decoded in self.selected {
            selected.push(decoded.resolve(graph, meter)?);
        }
        let exports = MirTypeBridgeExportConstituentsV1::new(
            self.types,
            callables,
            dispatch,
            object_values,
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
