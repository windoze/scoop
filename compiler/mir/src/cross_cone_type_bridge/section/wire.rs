use super::dependency::DecodedMirTypeBridgeDependencyV1;
use super::*;

/// Untrusted seven-field transport. Resolution alone does not create selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCrossConeMirTypeBridgeSectionV1 {
    types: DecodedCanonicalParamFreeMirTypeExportsV1,
    callables: DecodedCanonicalMirCallableBindingsV1,
    dispatch: DecodedCanonicalMirDispatchSchemasV1,
    object_values: DecodedCanonicalMirObjectValuesV1,
    shape_support: DecodedCanonicalMirShapeSupportsV1,
    initialization_uses: DecodedCanonicalMirExternalInitializationUsesV1,
    selected: Vec<DecodedMirTypeBridgeDependencyV1>,
}
impl DecodedCrossConeMirTypeBridgeSectionV1 {
    pub fn validate<'a, E>(
        self,
        authority: MirTypeBridgeLocalAuthorityV1<'a>,
        dependencies: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
        source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<CrossConeMirTypeBridgeSectionV1<'a>, MirTypeBridgeSectionError<E>> {
        let dependencies = dependencies::complete(authority.provider(), dependencies, meter)?;
        let types = self.types.validate(graph, authority.foundation(), meter)?;
        let type_index = dependencies::types(&types, &dependencies, meter)?;
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
        let shapes =
            self.shape_support
                .validate(authority.provider(), graph, &type_index, meter)?;
        let initialization_uses =
            self.initialization_uses
                .validate(authority.provider(), graph, meter)?;
        let mut selected = reserve(self.selected.len(), meter)?;
        for decoded in self.selected {
            selected.push(decoded.resolve(graph, meter)?);
        }
        let exports = MirTypeBridgeExportConstituentsV1::new(
            types,
            callables,
            dispatch,
            object_values,
            shapes,
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
impl WireDecode for DecodedCrossConeMirTypeBridgeSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            types: decoder.field(1, DecodedCanonicalParamFreeMirTypeExportsV1::decode)?,
            callables: decoder.field(2, DecodedCanonicalMirCallableBindingsV1::decode)?,
            dispatch: decoder.field(3, DecodedCanonicalMirDispatchSchemasV1::decode)?,
            object_values: decoder.field(4, DecodedCanonicalMirObjectValuesV1::decode)?,
            shape_support: decoder.field(5, DecodedCanonicalMirShapeSupportsV1::decode)?,
            initialization_uses: decoder
                .field(6, DecodedCanonicalMirExternalInitializationUsesV1::decode)?,
            selected: decoder.field(7, |decoder| {
                decoder.decode_array(|decoder, _| DecodedMirTypeBridgeDependencyV1::decode(decoder))
            })?,
        })
    }
}
impl WireEncode for DecodedCrossConeMirTypeBridgeSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.types.encode(encoder)?;
        encoder.field(2)?;
        self.callables.encode(encoder)?;
        encoder.field(3)?;
        self.dispatch.encode(encoder)?;
        encoder.field(4)?;
        self.object_values.encode(encoder)?;
        encoder.field(5)?;
        self.shape_support.encode(encoder)?;
        encoder.field(6)?;
        self.initialization_uses.encode(encoder)?;
        encoder.field(7)?;
        sequence(encoder, &self.selected)
    }
}
impl WireEncode for CrossConeMirTypeBridgeSectionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.types().encode(encoder)?;
        encoder.field(2)?;
        self.callables().encode(encoder)?;
        encoder.field(3)?;
        self.dispatch().encode(encoder)?;
        encoder.field(4)?;
        self.object_values().encode(encoder)?;
        encoder.field(5)?;
        self.shape_support().encode(encoder)?;
        encoder.field(6)?;
        self.initialization_uses().encode(encoder)?;
        encoder.field(7)?;
        self.selected().encode(encoder)
    }
}
