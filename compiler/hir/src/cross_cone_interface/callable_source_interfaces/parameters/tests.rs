use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DefinitionOrigin,
    NormalizedSourcePath, PersistentFunctionId, SignatureTypeKey, SourceContextKey, SourceIdentity,
    SourceSpan,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn calling_variants_have_fixed_indexed_wire() {
    let expected = [
        ("a10001", CallableVariant::Required),
        ("a200020107", CallableVariant::Default),
        ("a2000301a3000701000202", CallableVariant::VarargEmpty),
        ("a3000401a30007010002020209", CallableVariant::VarargDefault),
    ];

    for (wire, expected) in expected {
        let decoded: DecodedCallableParameterCallingV1 =
            decode_canonical(&hex_bytes(wire)).unwrap();
        assert_eq!(variant(&decoded), expected);
        assert_eq!(hex(&encode(&decoded).unwrap()), wire);
    }
}

#[test]
fn calling_decoder_rejects_unknown_tags_wrong_shapes_and_wide_indices() {
    let unknown =
        decode_canonical::<DecodedCallableParameterCallingV1>(&hex_bytes("a10005")).unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let wrong_shape =
        decode_canonical::<DecodedCallableParameterCallingV1>(&hex_bytes("a200010100"))
            .unwrap_err();
    assert_eq!(
        wrong_shape.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );

    let wide = decode_canonical::<DecodedCallableParameterCallingV1>(&hex_bytes(
        "a20002011b0000000100000000",
    ))
    .unwrap_err();
    assert_eq!(wide.kind(), &WireErrorKind::IntegerOutOfRange);
}

#[test]
fn parameter_list_rejects_duplicate_names_and_multiple_varargs() {
    let owner = CallableTemplateOrigin::Function(function_id("collect"));
    let duplicate = CanonicalCallableSourceParametersV1::try_new(vec![
        parameter("item", CallableParameterCallingV1::Required),
        parameter(
            "item",
            CallableParameterCallingV1::Default {
                template: ExportDefaultTemplateKeyV1::new(owner, 1),
            },
        ),
    ]);
    assert!(matches!(
        duplicate,
        Err(CallableSourceParameterListBuildError::DuplicateName { position: 1, .. })
    ));

    let multiple = CanonicalCallableSourceParametersV1::try_new(vec![
        parameter(
            "head",
            CallableParameterCallingV1::VarargEmpty {
                element_type: binder(0),
            },
        ),
        parameter(
            "tail",
            CallableParameterCallingV1::VarargDefault {
                element_type: binder(1),
                template: ExportDefaultTemplateKeyV1::new(owner, 1),
            },
        ),
    ]);
    assert_eq!(
        multiple,
        Err(CallableSourceParameterListBuildError::MultipleVarargs {
            first: 0,
            duplicate: 1,
        })
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallableVariant {
    Required,
    Default,
    VarargEmpty,
    VarargDefault,
}

fn variant(value: &DecodedCallableParameterCallingV1) -> CallableVariant {
    match value {
        DecodedCallableParameterCallingV1::Required => CallableVariant::Required,
        DecodedCallableParameterCallingV1::Default { .. } => CallableVariant::Default,
        DecodedCallableParameterCallingV1::VarargEmpty { .. } => CallableVariant::VarargEmpty,
        DecodedCallableParameterCallingV1::VarargDefault { .. } => CallableVariant::VarargDefault,
    }
}

fn parameter(name: &str, calling: CallableParameterCallingV1) -> CallableSourceParameterV1 {
    CallableSourceParameterV1::new(
        CanonicalIdentifier::new(name).unwrap(),
        binder(0),
        calling,
        origin(),
    )
}

fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

fn function_id(name: &str) -> PersistentFunctionId {
    use scoop_identity::{
        CborIdentityRecord, DeclarationScope, DefinitionOwnerChain, PackagePath,
        SourceDeclarationKey, SourceDeclarationSite,
    };

    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
    .id()
}

fn origin() -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/Parameters.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(0, 1).unwrap(), &context).unwrap(),
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_bytes(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let digits = std::str::from_utf8(digits).unwrap();
            u8::from_str_radix(digits, 16).unwrap()
        })
        .collect()
}
