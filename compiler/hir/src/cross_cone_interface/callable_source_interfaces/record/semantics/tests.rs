use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOrigin, DefinitionOwnerChain, Effect, GcEffect, NonEmptyVec,
    NormalizedSourcePath, PackagePath, PersistentFunctionId, PersistentGenericTypeId,
    PersistentTypeId, SignatureTypeKey, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};

use super::*;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableModalityV1, CallableOperatorRoleV1,
    CallableParameterCallingV1, CallableSafetyV1, CallableSourceEffectsV1,
    CallableSourceInterfaceV1, CallableSourceParameterV1, CanonicalBinderListV1,
    CanonicalCallableSourceParametersV1, CanonicalSourceParameterShapesV1,
    ExportDefinitionSourceV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
    SourceParameterShapeV1,
};

#[test]
fn validates_exact_callable_shape_vararg_array_and_origins() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();

    assert_eq!(
        fixture
            .source
            .validate_semantics(&fixture.callable, &mut authority),
        Ok(())
    );
    assert_eq!(authority.validated_origins, vec![0, 1]);
}

#[test]
fn rejects_callable_owner_arity_name_and_type_mismatches() {
    let fixture = Fixture::new();
    let other = Fixture::named("other");
    assert!(matches!(
        fixture
            .source
            .validate_semantics(&other.callable, &mut fixture.authority()),
        Err(CallableSourceInterfaceSemanticValidationError::CallableDeclaration { .. })
    ));

    let short_callable = callable(fixture.owner, vec![("value", fixture.scalar_type())]);
    assert_eq!(
        fixture
            .source
            .validate_semantics(&short_callable, &mut fixture.authority()),
        Err(
            CallableSourceInterfaceSemanticValidationError::ParameterArity {
                expected: 1,
                actual: 2,
            }
        )
    );

    let renamed = callable(
        fixture.owner,
        vec![
            ("renamed", fixture.scalar_type()),
            ("rest", fixture.array_type()),
        ],
    );
    assert!(matches!(
        fixture
            .source
            .validate_semantics(&renamed, &mut fixture.authority()),
        Err(CallableSourceInterfaceSemanticValidationError::ParameterName { index: 0, .. })
    ));

    let changed_type = callable(
        fixture.owner,
        vec![("value", unit_type()), ("rest", fixture.array_type())],
    );
    assert!(matches!(
        fixture
            .source
            .validate_semantics(&changed_type, &mut fixture.authority()),
        Err(CallableSourceInterfaceSemanticValidationError::ParameterType { index: 0, .. })
    ));
}

#[test]
fn rejects_a_template_without_an_intrinsic_array_declaration() {
    let fixture = Fixture::new();
    let impostor = generic_type("ArrayLike").id();
    let impostor_application = SignatureTypeKey::NominalApplication {
        origin: impostor,
        arguments: NonEmptyVec::from_first(fixture.scalar_type(), []),
    };
    let callable = callable(
        fixture.owner,
        vec![
            ("value", fixture.scalar_type()),
            ("rest", impostor_application.clone()),
        ],
    );
    let source = source_interface(
        fixture.owner,
        vec![
            (
                "value",
                fixture.scalar_type(),
                CallableParameterCallingV1::Required,
            ),
            (
                "rest",
                impostor_application.clone(),
                CallableParameterCallingV1::VarargEmpty {
                    element_type: fixture.scalar_type(),
                },
            ),
        ],
        fixture.origin.clone(),
    );

    assert!(matches!(
        source.validate_semantics(&callable, &mut fixture.authority()),
        Err(
            CallableSourceInterfaceSemanticValidationError::ArrayDeclaration {
                index: 1,
                error: AuthorityError::Array,
            }
        )
    ));
}

#[test]
fn rejects_vararg_element_and_application_arity_mismatches() {
    let fixture = Fixture::new();
    for arguments in [
        NonEmptyVec::from_first(unit_type(), []),
        NonEmptyVec::from_first(fixture.scalar_type(), [fixture.scalar_type()]),
    ] {
        let actual = SignatureTypeKey::NominalApplication {
            origin: fixture.array,
            arguments,
        };
        let callable = callable(fixture.owner, vec![("rest", actual.clone())]);
        let source = source_interface(
            fixture.owner,
            vec![(
                "rest",
                actual.clone(),
                CallableParameterCallingV1::VarargEmpty {
                    element_type: fixture.scalar_type(),
                },
            )],
            fixture.origin.clone(),
        );
        assert_eq!(
            source.validate_semantics(&callable, &mut fixture.authority()),
            Err(
                CallableSourceInterfaceSemanticValidationError::VarargArrayType {
                    index: 0,
                    element_type: Box::new(fixture.scalar_type()),
                    actual: Box::new(actual),
                }
            )
        );
    }
}

#[test]
fn rejects_foreign_and_authority_rejected_parameter_origins() {
    let fixture = Fixture::new();
    let foreign = ConeCoordinate::new("example", "foreign", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let foreign_source = source_interface(
        fixture.owner,
        vec![
            (
                "value",
                fixture.scalar_type(),
                CallableParameterCallingV1::Required,
            ),
            (
                "rest",
                fixture.array_type(),
                CallableParameterCallingV1::VarargEmpty {
                    element_type: fixture.scalar_type(),
                },
            ),
        ],
        origin(foreign),
    );
    assert!(matches!(
        foreign_source.validate_semantics(&fixture.callable, &mut fixture.authority()),
        Err(
            CallableSourceInterfaceSemanticValidationError::DefinitionOriginCone {
                index: 0,
                expected: ConeIdentity::CORE,
                actual,
            }
        ) if actual == foreign
    ));

    let mut rejected = fixture.authority();
    rejected.reject_origin = Some(1);
    assert_eq!(
        fixture
            .source
            .validate_semantics(&fixture.callable, &mut rejected),
        Err(
            CallableSourceInterfaceSemanticValidationError::DefinitionOrigin {
                index: 1,
                error: AuthorityError::Origin(1),
            }
        )
    );
}

#[test]
fn reports_a_missing_array_declaration_at_the_vararg() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    authority.reject_array = true;

    assert_eq!(
        fixture
            .source
            .validate_semantics(&fixture.callable, &mut authority),
        Err(
            CallableSourceInterfaceSemanticValidationError::ArrayDeclaration {
                index: 1,
                error: AuthorityError::Array,
            }
        )
    );
}

struct Fixture {
    owner: CallableTemplateOrigin,
    scalar: PersistentTypeId,
    array: PersistentGenericTypeId,
    origin: ExportDefinitionSourceV1,
    callable: CallableInterfaceRecordV1,
    source: CallableSourceInterfaceV1,
}

impl Fixture {
    fn new() -> Self {
        Self::named("collect")
    }

    fn named(name: &str) -> Self {
        let scalar = concrete_type("Int").id();
        let array = generic_type("Array").id();
        let owner = CallableTemplateOrigin::Function(function(name).id());
        let origin = origin(ConeIdentity::CORE);
        let scalar_type = SignatureTypeKey::Nominal(scalar);
        let array_type = SignatureTypeKey::NominalApplication {
            origin: array,
            arguments: NonEmptyVec::from_first(scalar_type.clone(), []),
        };
        let callable = callable(
            owner,
            vec![("value", scalar_type.clone()), ("rest", array_type.clone())],
        );
        let source = source_interface(
            owner,
            vec![
                (
                    "value",
                    scalar_type.clone(),
                    CallableParameterCallingV1::Required,
                ),
                (
                    "rest",
                    array_type,
                    CallableParameterCallingV1::VarargEmpty {
                        element_type: scalar_type,
                    },
                ),
            ],
            origin.clone(),
        );
        Self {
            owner,
            scalar,
            array,
            origin,
            callable,
            source,
        }
    }

    fn scalar_type(&self) -> SignatureTypeKey {
        SignatureTypeKey::Nominal(self.scalar)
    }

    fn array_type(&self) -> SignatureTypeKey {
        SignatureTypeKey::NominalApplication {
            origin: self.array,
            arguments: NonEmptyVec::from_first(self.scalar_type(), []),
        }
    }

    fn authority(&self) -> Authority {
        Authority {
            current: ConeIdentity::CORE,
            array: self.array,
            reject_array: false,
            reject_origin: None,
            validated_origins: Vec::new(),
        }
    }
}

fn callable(
    owner: CallableTemplateOrigin,
    parameters: Vec<(&str, SignatureTypeKey)>,
) -> CallableInterfaceRecordV1 {
    CallableInterfaceRecordV1::try_new(
        owner,
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(
            parameters
                .into_iter()
                .map(|(name, value_type)| SourceParameterShapeV1::new(identifier(name), value_type))
                .collect(),
        )
        .unwrap(),
        unit_type(),
        CallableSourceEffectsV1::try_new(
            Effect::Ordinary,
            CallableSafetyV1::Safe,
            GcEffect::Managed,
            CallableImplementationV1::Scoop,
            CallableOperatorRoleV1::None,
            CallableInfixV1::Ordinary,
        )
        .unwrap(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap()
}

fn source_interface(
    owner: CallableTemplateOrigin,
    parameters: Vec<(&str, SignatureTypeKey, CallableParameterCallingV1)>,
    origin: ExportDefinitionSourceV1,
) -> CallableSourceInterfaceV1 {
    CallableSourceInterfaceV1::try_new(
        owner,
        CanonicalCallableSourceParametersV1::try_new(
            parameters
                .into_iter()
                .map(|(name, value_type, calling)| {
                    CallableSourceParameterV1::new(
                        identifier(name),
                        value_type,
                        calling,
                        origin.clone(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    )
    .unwrap()
}

struct Authority {
    current: ConeIdentity,
    array: PersistentGenericTypeId,
    reject_array: bool,
    reject_origin: Option<u32>,
    validated_origins: Vec<u32>,
}

impl CallableSourceInterfaceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn validate_array_type(
        &mut self,
        array: PersistentGenericTypeId,
    ) -> Result<(), AuthorityError> {
        if self.reject_array || array != self.array {
            Err(AuthorityError::Array)
        } else {
            Ok(())
        }
    }

    fn validate_source_parameter_origin(
        &mut self,
        _owner: crate::CallableDeclarationId,
        position: u32,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        if self.reject_origin == Some(position) {
            Err(AuthorityError::Origin(position))
        } else {
            self.validated_origins.push(position);
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityError {
    Array,
    Origin(u32),
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AuthorityError {}

fn function(name: &str) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        site(ConeIdentity::CORE),
        identifier(name),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn concrete_type(name: &str) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(ConeIdentity::CORE),
        identifier(name),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn generic_type(name: &str) -> CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(ConeIdentity::CORE),
        identifier(name),
        SourceNominalKind::Class,
        1,
    ))
    .unwrap()
}

fn unit_type() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    )
}

fn origin(cone: ConeIdentity) -> ExportDefinitionSourceV1 {
    let path = if cone == ConeIdentity::SINGLE_FILE {
        NormalizedSourcePath::single_file()
    } else {
        NormalizedSourcePath::new("src/Callable.scoop").unwrap()
    };
    let source = SourceIdentity::new(cone, path).unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(0, 1).unwrap(), &context).unwrap(),
    )
}

fn site(cone: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
