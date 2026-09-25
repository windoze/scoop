use super::*;
use scoop_wire::{Encoder, WireEncode};
type EncodeResult = Result<(), scoop_wire::cbor::EncodeError>;

pub(super) fn resolve(
    exports: &LayoutAbiExportConstituentsV1,
    semantic: &[LayoutAbiDependencyV1],
    rows: &[&dyn WireEncode],
    identities: &mut ValidatedIdentityGraph,
) -> DependencyResolvedCrossConeLayoutAbiSectionV1 {
    let bytes = encode(&Selection {
        exports,
        semantic,
        rows,
    })
    .unwrap();
    let decoded: DecodedCrossConeLayoutAbiSectionV1 = decode_canonical(&bytes).unwrap();
    decoded
        .validate_layouts(exports.layouts())
        .unwrap()
        .validate_callables(exports.callables())
        .unwrap()
        .validate_dispatch(exports.dispatch())
        .unwrap()
        .validate_descriptors(exports.descriptors())
        .unwrap()
        .validate_shape_support::<Infallible>(exports.shape_support())
        .unwrap()
        .resolve_dependencies::<Infallible>(identities)
        .unwrap()
}

struct Selection<'a> {
    exports: &'a LayoutAbiExportConstituentsV1,
    semantic: &'a [LayoutAbiDependencyV1],
    rows: &'a [&'a dyn WireEncode],
}
impl WireEncode for Selection<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(6)?;
        field(encoder, 1, self.exports.layouts())?;
        field(encoder, 2, self.exports.descriptors())?;
        field(encoder, 3, self.exports.dispatch())?;
        field(encoder, 4, self.exports.callables())?;
        field(encoder, 5, self.exports.shape_support())?;
        encoder.field(6)?;
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.semantic.len() as u64)?;
        for relation in self.semantic {
            relation.encode(encoder)?;
        }
        encoder.field(2)?;
        Rows(self.rows).encode(encoder)
    }
}

pub(super) struct Rows<'a>(pub &'a [&'a dyn WireEncode]);
impl WireEncode for Rows<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.array(self.0.len() as u64)?;
        for row in self.0 {
            row.encode(encoder)?;
        }
        Ok(())
    }
}

pub(super) struct ReplacedField<'a> {
    pub original: &'a ExternalShapeLinkImportV1<'a>,
    pub index: u32,
    pub replacement: &'a dyn WireEncode,
}
impl WireEncode for ReplacedField<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(5)?;
        let provider = self.original.provider();
        let subject = self.original.subject();
        let symbol = self.original.expected_symbol();
        let definition = self.original.required_definition();
        let fields: [&dyn WireEncode; 5] = [
            &provider,
            &subject,
            &symbol,
            &definition,
            self.original.contract(),
        ];
        for (index, original) in fields.into_iter().enumerate() {
            let index = index as u32 + 1;
            encoder.field(index)?;
            if index == self.index {
                self.replacement.encode(encoder)?;
            } else {
                original.encode(encoder)?;
            }
        }
        Ok(())
    }
}

fn field(encoder: &mut Encoder, index: u32, value: &impl WireEncode) -> EncodeResult {
    encoder.field(index)?;
    value.encode(encoder)
}
