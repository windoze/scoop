use scoop_identity::{CallableTemplateOrigin, StructuralDefinitionSiteRole};
use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::super::test_support::*;
use super::*;

#[test]
fn local_function_round_trips_through_indexed_captures() {
    let fixture = Fixture::new();
    let expected = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        path(StructuralDefinitionSiteRole::LocalDeclaration, 0),
        function_type(),
        vec![fixture.capture(0)],
        2,
    )
    .unwrap();
    let mut locals = LocalResolver::new(vec![parameter(0)]);
    let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();

    assert_eq!(bytes[0], 0xa5);
    let decoded: DecodedDefaultLocalFunctionV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut locals),
        Ok(expected.clone())
    );
    assert_eq!(
        expected.declaration(),
        CallableTemplateOrigin::Function(fixture.function)
    );
    assert_eq!(expected.capture_count(), 1);
    assert_eq!(expected.owner_type_parameter_count(), 2);
}

#[test]
fn local_function_rejects_non_function_declarations_on_both_sides() {
    let fixture = Fixture::new();
    let path = path(StructuralDefinitionSiteRole::LocalDeclaration, 0);
    assert_eq!(
        DefaultLocalFunctionV1::try_new(
            CallableTemplateOrigin::Constructor(fixture.constructor),
            path.clone(),
            function_type(),
            Vec::new(),
            0,
        ),
        Err(DefaultLocalFunctionBuildError::UnsupportedDeclaration(
            CallableTemplateOrigin::Constructor(fixture.constructor)
        ))
    );

    let raw = RawLocalFunction {
        declaration: CallableTemplateOrigin::Constructor(fixture.constructor),
        definition_path: path,
    };
    let decoded: DecodedDefaultLocalFunctionV1 = decode_canonical(&encode(&raw).unwrap()).unwrap();
    let mut locals = LocalResolver::new(Vec::new());
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut locals),
        Err(DefaultLocalFunctionResolutionError::Record(
            DefaultLocalFunctionBuildError::UnsupportedDeclaration(
                CallableTemplateOrigin::Constructor(fixture.constructor)
            )
        ))
    );
}

#[test]
fn local_function_index_error_identifies_capture_position() {
    let fixture = Fixture::new();
    let function = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        path(StructuralDefinitionSiteRole::LocalDeclaration, 0),
        function_type(),
        vec![fixture.capture(0), fixture.capture(2)],
        0,
    )
    .unwrap();
    let mut locals = LocalResolver::new(vec![parameter(0)]);

    assert_eq!(
        function.index_locals(&mut locals).unwrap_err(),
        DefaultLocalFunctionIndexError::Capture {
            index: 1,
            error: DefaultCaptureIndexError::Source(LocalError::MissingSelector(parameter(2))),
        }
    );
}

#[test]
fn local_function_decoder_requires_all_five_fields() {
    let error = decode_canonical::<DecodedDefaultLocalFunctionV1>(&[0xa1, 0x01, 0xa1, 0x00, 0x01])
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 5,
            actual: 1,
        }
    );
}

struct RawLocalFunction {
    declaration: CallableTemplateOrigin,
    definition_path: scoop_identity::StructuralDefinitionPath,
}

impl WireEncode for RawLocalFunction {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.definition_path.encode(encoder)?;
        encoder.field(3)?;
        function_type().encode(encoder)?;
        encoder.field(4)?;
        encoder.array(0)?;
        encoder.field(5)?;
        encoder.unsigned(0)
    }
}
