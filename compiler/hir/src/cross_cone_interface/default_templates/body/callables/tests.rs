use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope,
    DecodedPersistentId, DefinitionOwnerChain, Effect, GeneratedCallableKey, LexicalCallableParent,
    LexicalCallableRole, OptionalSignatureType, PackagePath, PersistentConstructorId,
    PersistentEnumVariantId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentIdResolver,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId, PropertyAccessorKey,
    PropertyOwner, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn callable_declarations_have_fixed_tags_and_resolve_by_kind() {
    let fixture = Fixture::new();
    let declarations = [
        DefaultCallableDeclarationV1::Function(fixture.function),
        DefaultCallableDeclarationV1::GenericFunction(fixture.generic_function),
        DefaultCallableDeclarationV1::PropertyAccessor(fixture.accessor),
        DefaultCallableDeclarationV1::Generated(fixture.generated),
    ];
    let mut resolver = fixture.resolver();

    for (index, declaration) in declarations.into_iter().enumerate() {
        let bytes = encode(&declaration).unwrap();
        assert_eq!(bytes[2], u8::try_from(index + 1).unwrap());
        let decoded: DecodedDefaultCallableDeclarationV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.resolve(&mut resolver), Ok(declaration));
    }
}

#[test]
fn callable_reference_has_fixed_wire_and_round_trips() {
    let fixture = Fixture::new();
    let callable = fixture.callable();
    let bytes = encode(&callable).unwrap();

    assert_eq!(
        hex(&bytes),
        "a301a2000101582012104f4f6e246d6533e24859a416718ca33e20fa88c78d1285c694aa56c3766402a100010381a3000701000201"
    );

    let decoded: DecodedDefaultCallableRefV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(callable));
}

#[test]
fn method_callee_variants_round_trip_without_application_ids() {
    let fixture = Fixture::new();
    let callable = fixture.callable();
    let cases = [
        DefaultMethodCalleeV1::Callable(callable.clone()),
        DefaultMethodCalleeV1::Bound(DefaultBoundCallableRefV1::new(
            DefaultBinderRefV1::new(1, 2),
            DefaultBoundCallableSourceV1::Class {
                bound: binder(0),
                callable,
            },
            function_type(),
        )),
        DefaultMethodCalleeV1::Bound(DefaultBoundCallableRefV1::new(
            DefaultBinderRefV1::new(2, 3),
            DefaultBoundCallableSourceV1::Interface {
                bound: binder(1),
                member: CallableTemplateOrigin::GenericFunction(fixture.generic_function),
            },
            function_type(),
        )),
        DefaultMethodCalleeV1::DerivedEquality {
            owner_type: binder(2),
        },
    ];

    for (expected_tag, expected) in [1, 2, 2, 3].into_iter().zip(cases) {
        let bytes = encode(&expected).unwrap();
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultMethodCalleeV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(expected));
    }
}

#[test]
fn binder_reference_has_fixed_wire() {
    let reference = DefaultBinderRefV1::new(3, 42);
    let bytes = encode(&reference).unwrap();

    assert_eq!(hex(&bytes), "a2010302182a");
    assert_eq!(
        decode_canonical::<DefaultBinderRefV1>(&bytes).unwrap(),
        reference
    );
    assert_eq!(reference.depth(), 3);
    assert_eq!(reference.index(), 42);
}

#[test]
fn callable_resolution_reports_declaration_and_type_argument_locations() {
    let fixture = Fixture::new();
    let missing_declaration = DefaultCallableRefV1::try_new(
        DefaultCallableDeclarationV1::Function(fixture.other_function),
        OptionalSignatureType::Absent,
        Vec::new(),
    )
    .unwrap();
    let decoded: DecodedDefaultCallableRefV1 =
        decode_canonical(&encode(&missing_declaration).unwrap()).unwrap();
    assert_eq!(
        decoded.resolve(&mut fixture.resolver()),
        Err(DefaultCallableRefResolutionError::Declaration(
            ResolutionError::Unknown("function")
        ))
    );

    let missing_argument = DefaultCallableRefV1::try_new(
        DefaultCallableDeclarationV1::Function(fixture.function),
        OptionalSignatureType::Absent,
        vec![
            SignatureTypeKey::Nominal(fixture.allowed_type),
            SignatureTypeKey::Nominal(fixture.missing_type),
        ],
    )
    .unwrap();
    let decoded: DecodedDefaultCallableRefV1 =
        decode_canonical(&encode(&missing_argument).unwrap()).unwrap();
    assert_eq!(
        decoded.resolve(&mut fixture.resolver()),
        Err(DefaultCallableRefResolutionError::TypeArgument {
            index: 1,
            error: ResolutionError::Unknown("type"),
        })
    );
}

#[test]
fn sum_decoders_reject_unknown_tags_and_non_exact_maps() {
    let mut unknown_declaration = vec![0xa2, 0x00, 0x05, 0x01, 0x58, 0x20];
    unknown_declaration.extend([0; 32]);
    let error =
        decode_canonical::<DecodedDefaultCallableDeclarationV1>(&unknown_declaration).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let error =
        decode_canonical::<DecodedDefaultCallableDeclarationV1>(&[0xa1, 0x00, 0x01]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );

    let error = decode_canonical::<DecodedDefaultMethodCalleeV1>(&[0xa1, 0x00, 0x03]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

struct Fixture {
    function: PersistentFunctionId,
    other_function: PersistentFunctionId,
    generic_function: PersistentGenericFunctionId,
    accessor: PersistentPropertyAccessorId,
    generated: PersistentGeneratedCallableId,
    allowed_type: PersistentTypeId,
    missing_type: PersistentTypeId,
}

impl Fixture {
    fn new() -> Self {
        let function = source_function("collect", 0, Vec::new());
        let other_function = source_function("other", 0, Vec::new());
        let generic_function =
            PersistentGenericFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                top_level_site(),
                identifier("generic"),
                1,
                None,
                vec![binder(0)],
            ))
            .unwrap();
        let property = PersistentPropertyId::from_source_declaration(
            &SourceDeclarationKey::property(top_level_site(), identifier("value")),
        )
        .unwrap();
        let accessor = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            PropertyOwner::Property(property),
            AccessorRole::Getter,
        ))
        .unwrap();
        let generated = PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(function),
            role: LexicalCallableRole::LambdaBody,
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
                [],
            ),
        })
        .unwrap();
        Self {
            function,
            other_function,
            generic_function,
            accessor,
            generated,
            allowed_type: nominal("Allowed"),
            missing_type: nominal("Missing"),
        }
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            function: self.function,
            generic_function: self.generic_function,
            accessor: self.accessor,
            generated: self.generated,
            allowed_type: self.allowed_type,
        }
    }

    fn callable(&self) -> DefaultCallableRefV1 {
        DefaultCallableRefV1::try_new(
            DefaultCallableDeclarationV1::Function(self.function),
            OptionalSignatureType::Absent,
            vec![binder(1)],
        )
        .unwrap()
    }
}

struct Resolver {
    function: PersistentFunctionId,
    generic_function: PersistentGenericFunctionId,
    accessor: PersistentPropertyAccessorId,
    generated: PersistentGeneratedCallableId,
    allowed_type: PersistentTypeId,
}

macro_rules! resolve_fixture_identity {
    ($identity:ty, $field:ident, $name:literal) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                id.verify(self.$field)
                    .map_err(|_| ResolutionError::Unknown($name))
            }
        }
    };
}

macro_rules! reject_identity {
    ($identity:ty, $name:literal) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                _id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                Err(ResolutionError::Unknown($name))
            }
        }
    };
}

resolve_fixture_identity!(PersistentFunctionId, function, "function");
resolve_fixture_identity!(
    PersistentGenericFunctionId,
    generic_function,
    "generic function"
);
resolve_fixture_identity!(PersistentPropertyAccessorId, accessor, "property accessor");
resolve_fixture_identity!(
    PersistentGeneratedCallableId,
    generated,
    "generated callable"
);
resolve_fixture_identity!(PersistentTypeId, allowed_type, "type");
reject_identity!(PersistentGenericTypeId, "generic type");
reject_identity!(PersistentConstructorId, "constructor");
reject_identity!(PersistentEnumVariantId, "enum variant");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolutionError {
    Unknown(&'static str),
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(kind) => write!(formatter, "unknown {kind}"),
        }
    }
}

impl std::error::Error for ResolutionError {}

fn source_function(
    name: &str,
    type_parameter_count: u32,
    parameters: Vec<SignatureTypeKey>,
) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        top_level_site(),
        identifier(name),
        type_parameter_count,
        None,
        parameters,
    ))
    .unwrap()
}

fn nominal(name: &str) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        top_level_site(),
        identifier(name),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

fn function_type() -> SignatureTypeKey {
    SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![binder(0)],
        result: Box::new(binder(1)),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
