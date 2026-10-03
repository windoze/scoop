use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOrigin, DefinitionOwnerChain, LocalValueSelector, PackagePath,
    PersistentFunctionId, SignatureTypeKey, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceSpan,
};

use super::*;
use crate::{
    CallableParameterCallingV1, CallableSourceInterfaceV1, CallableSourceParameterV1,
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalCallableSourceParametersV1,
    DefaultTemplateProviderShapeV1, ExportDefinitionSourceV1, TemplateLocalDefinitionV1,
    TemplateLocalRecordV1, TemplateValueParameterV1,
};

#[test]
fn validates_the_exact_preceding_parameter_prefix() {
    let owner = function("defaults");
    let owner = CallableTemplateOrigin::Function(owner.id());
    let key = ExportDefaultTemplateKeyV1::new(owner, 2);
    let source = source_interface(owner, &[binder(0), binder(1), binder(2)], Some(2));
    let parameters = value_parameters(2);
    let locals = local_table(&[
        (0, binder(0), CanonicalBooleanV1::False),
        (1, binder(1), CanonicalBooleanV1::False),
    ]);

    assert_eq!(validate(&parameters, key, &source, &locals), Ok(()));
}

#[test]
fn validates_source_owner_parameter_and_template_reference() {
    let owner = CallableTemplateOrigin::Function(function("defaults").id());
    let other = CallableTemplateOrigin::Function(function("other").id());
    let key = ExportDefaultTemplateKeyV1::new(owner, 1);
    let parameters = value_parameters(1);

    assert_eq!(
        validate(
            &parameters,
            key,
            &source_interface(other, &[binder(0), binder(1)], Some(1)),
            &local_table(&[(0, binder(0), CanonicalBooleanV1::False)]),
        ),
        Err(TemplateValueParameterSemanticValidationError::SourceOwner {
            expected: owner,
            actual: other,
        })
    );

    let out_of_range = ExportDefaultTemplateKeyV1::new(owner, 2);
    assert_eq!(
        validate(
            &parameters,
            out_of_range,
            &source_interface(owner, &[binder(0), binder(1)], None),
            &local_table(&[(0, binder(0), CanonicalBooleanV1::False)]),
        ),
        Err(
            TemplateValueParameterSemanticValidationError::ParameterOutOfRange {
                position: 2,
                arity: 2,
            }
        )
    );

    assert_eq!(
        validate(
            &parameters,
            key,
            &source_interface(owner, &[binder(0), binder(1)], None),
            &local_table(&[(0, binder(0), CanonicalBooleanV1::False)]),
        ),
        Err(
            TemplateValueParameterSemanticValidationError::ParameterTemplate {
                position: 1,
                expected: key,
                actual: None,
            }
        )
    );
}

#[test]
fn validates_prefix_arity() {
    let owner = CallableTemplateOrigin::Function(function("defaults").id());
    let key = ExportDefaultTemplateKeyV1::new(owner, 2);
    let source = source_interface(owner, &[binder(0), binder(1), binder(2)], Some(2));

    assert_eq!(
        validate(
            &value_parameters(1),
            key,
            &source,
            &local_table(&[(0, binder(0), CanonicalBooleanV1::False)]),
        ),
        Err(TemplateValueParameterSemanticValidationError::PrefixArity {
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
fn validates_each_parameter_local_record() {
    let owner = CallableTemplateOrigin::Function(function("defaults").id());
    let key = ExportDefaultTemplateKeyV1::new(owner, 2);
    let source = source_interface(owner, &[binder(0), binder(1), binder(2)], Some(2));
    let parameters = value_parameters(2);

    assert_eq!(
        validate(
            &parameters,
            key,
            &source,
            &local_table(&[(0, binder(0), CanonicalBooleanV1::False)]),
        ),
        Err(
            TemplateValueParameterSemanticValidationError::MissingLocal {
                position: 1,
                selector: parameter_selector(1),
            }
        )
    );
    assert_eq!(
        validate(
            &parameters,
            key,
            &source,
            &local_table(&[
                (0, binder(0), CanonicalBooleanV1::True),
                (1, binder(1), CanonicalBooleanV1::False),
            ]),
        ),
        Err(TemplateValueParameterSemanticValidationError::MutableLocal { position: 0 })
    );
    assert_eq!(
        validate(
            &parameters,
            key,
            &source,
            &local_table(&[
                (0, binder(9), CanonicalBooleanV1::False),
                (1, binder(1), CanonicalBooleanV1::False),
            ]),
        ),
        Err(TemplateValueParameterSemanticValidationError::LocalType {
            position: 0,
            expected: Box::new(binder(0)),
            actual: Box::new(binder(9)),
        })
    );
}

#[test]
fn parameter_types_are_mapped_from_both_provider_frames() {
    let owner = CallableTemplateOrigin::Function(function("inherited").id());
    let key = ExportDefaultTemplateKeyV1::new(owner, 2);
    let mapped_owner = binder(4);
    let mapped_callable = binder(3);
    let source = source_interface(
        owner,
        &[mapped_owner.clone(), mapped_callable.clone(), binder(2)],
        Some(2),
    );
    let parameters = value_parameters(2);
    let locals = local_table(&[
        (
            0,
            SignatureTypeKey::Binder { depth: 1, index: 0 },
            CanonicalBooleanV1::False,
        ),
        (
            1,
            SignatureTypeKey::Binder { depth: 0, index: 0 },
            CanonicalBooleanV1::False,
        ),
    ]);
    let provider = DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap();
    let type_parameters =
        CanonicalBinderUseListV1::try_new(vec![mapped_owner, mapped_callable]).unwrap();

    assert_eq!(
        parameters.validate_semantics(key, &source, &locals, provider, &type_parameters),
        Ok(())
    );
}

#[test]
fn parameter_type_substitution_failure_reports_the_position() {
    let owner = CallableTemplateOrigin::Function(function("invalidMapping").id());
    let key = ExportDefaultTemplateKeyV1::new(owner, 1);
    let source = source_interface(owner, &[binder(0), binder(1)], Some(1));
    let parameters = value_parameters(1);
    let invalid = SignatureTypeKey::Binder { depth: 2, index: 0 };
    let locals = local_table(&[(0, invalid, CanonicalBooleanV1::False)]);
    let provider = DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap();
    let type_parameters = CanonicalBinderUseListV1::try_new(vec![binder(0), binder(1)]).unwrap();

    assert_eq!(
        parameters.validate_semantics(key, &source, &locals, provider, &type_parameters),
        Err(
            TemplateValueParameterSemanticValidationError::TypeSubstitution {
                position: 0,
                error: DefaultTemplateTypeSubstitutionError::ProviderBinder(
                    crate::SignatureBinderScopeError::DepthOutOfRange {
                        depth: 2,
                        available_depths: 2,
                    }
                ),
            }
        )
    );
}

fn validate(
    parameters: &CanonicalTemplateValueParametersV1,
    key: ExportDefaultTemplateKeyV1,
    source: &CallableSourceInterfaceV1,
    locals: &CanonicalTemplateLocalTableV1,
) -> Result<(), TemplateValueParameterSemanticValidationError> {
    parameters.validate_semantics(key, source, locals, provider(), &identity_mapping())
}

fn provider() -> DefaultTemplateProviderShapeV1 {
    DefaultTemplateProviderShapeV1::try_new(0, 10).unwrap()
}

fn identity_mapping() -> CanonicalBinderUseListV1 {
    CanonicalBinderUseListV1::try_new((0..10).map(binder).collect()).unwrap()
}

fn value_parameters(count: u32) -> CanonicalTemplateValueParametersV1 {
    CanonicalTemplateValueParametersV1::try_new(
        (0..count)
            .map(|position| {
                TemplateValueParameterV1::try_new(position, parameter_selector(position)).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn local_table(
    locals: &[(u32, SignatureTypeKey, CanonicalBooleanV1)],
) -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(
        locals
            .iter()
            .map(|(position, value_type, mutable)| {
                TemplateLocalRecordV1::try_new(
                    parameter_selector(*position),
                    value_type.clone(),
                    *mutable,
                    TemplateLocalDefinitionV1::Source(origin()),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn source_interface(
    owner: CallableTemplateOrigin,
    types: &[SignatureTypeKey],
    default_position: Option<u32>,
) -> CallableSourceInterfaceV1 {
    let parameters = types
        .iter()
        .enumerate()
        .map(|(position, value_type)| {
            let position = u32::try_from(position).unwrap();
            let calling = if default_position == Some(position) {
                CallableParameterCallingV1::Default {
                    template: ExportDefaultTemplateKeyV1::new(owner, position),
                }
            } else {
                CallableParameterCallingV1::Required
            };
            let name = format!("p{position}");
            CallableSourceParameterV1::new(
                CanonicalIdentifier::new(&name).unwrap(),
                value_type.clone(),
                calling,
                origin(),
            )
        })
        .collect();
    CallableSourceInterfaceV1::try_new(
        owner,
        CanonicalCallableSourceParametersV1::try_new(parameters).unwrap(),
    )
    .unwrap()
}

fn function(name: &str) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
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
}

fn origin() -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::single_file();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(1, 2).unwrap(), &context).unwrap(),
    )
}

const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

const fn parameter_selector(position: u32) -> LocalValueSelector {
    LocalValueSelector::Parameter {
        declaration_index: position,
    }
}
