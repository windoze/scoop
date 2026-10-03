use crate::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use scoop_identity::{AccessorRole, Effect, SourceNominalKind};

use super::PropertyAccessorClosureValidationError;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableModalityV1, CallableOperatorRoleV1,
    CallableOperatorV1, CanonicalCallableInterfacesV1, CanonicalPropertyInterfacesV1,
    DeclaredVisibilityV1, PropertyInterfaceRecordV1, PropertyPublicAccessV1,
    PropertyRepresentationV1, PropertySetterPublicAccessV1, PublicDeclarationOwnerV1,
    PublicLookupAccessV1,
};

mod source_forms;
mod support;
use source_forms::with_source_forms;

use support::*;

#[test]
fn accepts_complete_public_runtime_accessor_pair() {
    let fixture = Fixture::top_level("state");
    let property = fixture.property(
        Some(PropertySetterPublicAccessV1::Public),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );

    assert_eq!(
        validate(
            vec![property],
            vec![
                fixture.accessor(AccessorRole::Getter).build(),
                fixture.accessor(AccessorRole::Setter).build(),
            ],
        ),
        Ok(())
    );
}

#[test]
fn requires_every_accessor_and_keeps_restricted_setters_out_of_public_lookup() {
    let fixture = Fixture::top_level("required");
    let read_only = fixture.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    assert_eq!(
        validate(vec![read_only], Vec::new()),
        Err(
            PropertyAccessorClosureValidationError::MissingPublicAccessor {
                property: fixture.declaration,
                role: AccessorRole::Getter,
                accessor: fixture.getter,
            }
        )
    );

    let public_setter = fixture.property(
        Some(PropertySetterPublicAccessV1::Public),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    assert_eq!(
        validate(
            vec![public_setter],
            vec![fixture.accessor(AccessorRole::Getter).build()],
        ),
        Err(
            PropertyAccessorClosureValidationError::MissingPublicAccessor {
                property: fixture.declaration,
                role: AccessorRole::Setter,
                accessor: fixture.setter,
            }
        )
    );

    let restricted_setter = fixture.property(
        Some(PropertySetterPublicAccessV1::Restricted),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    assert_eq!(
        validate(
            vec![restricted_setter.clone()],
            vec![fixture.accessor(AccessorRole::Getter).build()],
        ),
        Err(
            PropertyAccessorClosureValidationError::MissingSourceAccessor {
                property: fixture.declaration,
                role: AccessorRole::Setter,
                accessor: fixture.setter,
            }
        )
    );
    assert_eq!(
        validate_support(
            vec![restricted_setter.clone()],
            vec![fixture.accessor(AccessorRole::Getter).build()],
            vec![
                fixture
                    .accessor(AccessorRole::Setter)
                    .build_source(DeclaredVisibilityV1::Private)
            ],
        ),
        Ok(())
    );
    assert_eq!(
        validate(
            vec![restricted_setter],
            vec![
                fixture.accessor(AccessorRole::Getter).build(),
                fixture.accessor(AccessorRole::Setter).build(),
            ],
        ),
        Err(
            PropertyAccessorClosureValidationError::RestrictedSetterExported {
                property: fixture.declaration,
                accessor: fixture.setter,
            }
        )
    );
}

#[test]
fn rejects_duplicate_accessor_claims_and_orphan_callable_records() {
    let first = Fixture::top_level("first");
    let second = Fixture::top_level("second");
    let first_property = first.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    let second_property = PropertyInterfaceRecordV1::try_new(
        second.declaration,
        second.owner,
        empty_binders(),
        None,
        second.value_type.clone(),
        Accessors::read_only(AccessorSource::new(first.getter, AccessorForm::Body)),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
        crate::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    let properties =
        CanonicalPropertyInterfacesV1::try_new(vec![first_property, second_property]).unwrap();
    let first_claim = properties.records()[0].declaration();
    let second_claim = properties.records()[1].declaration();
    let callables = CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap();

    assert_eq!(
        properties.validate_accessor_closure(&callables),
        Err(
            PropertyAccessorClosureValidationError::DuplicateAccessorClaim {
                accessor: first.getter,
                first: first_claim,
                second: second_claim,
            }
        )
    );

    let properties = CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap();
    let callables =
        CanonicalCallableInterfacesV1::try_new(vec![first.accessor(AccessorRole::Getter).build()])
            .unwrap();
    assert_eq!(
        properties.validate_accessor_closure(&callables),
        Err(PropertyAccessorClosureValidationError::OrphanPublicAccessor(first.getter))
    );
}

#[test]
fn checks_accessor_owner_and_extension_receiver() {
    let top_level = Fixture::top_level("owned");
    let nominal = Fixture::nominal("container", SourceNominalKind::Class);
    let property = top_level.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    let mut getter = top_level.accessor(AccessorRole::Getter);
    getter.owner = nominal.owner;
    assert_eq!(
        validate(vec![property], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::Owner {
            accessor: top_level.getter,
            expected: PublicDeclarationOwnerV1::TopLevel,
            actual: nominal.owner,
        })
    );

    let extension = Fixture::extension("extended");
    let property = extension.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    let mut getter = extension.accessor(AccessorRole::Getter);
    getter.receiver = Some(unit_type());
    assert_eq!(
        validate(vec![property], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::Receiver {
            accessor: extension.getter,
            expected: extension.receiver.clone().map(Box::new),
            actual: Some(Box::new(unit_type())),
        })
    );
}

#[test]
fn checks_getter_and_setter_parameter_shapes() {
    let fixture = Fixture::top_level("parameters");
    let read_only = fixture.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.parameters.push(fixture.value_type.clone());
    assert_eq!(
        validate(vec![read_only], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::ParameterArity {
            accessor: fixture.getter,
            expected: 0,
            actual: 1,
        })
    );

    let read_write = fixture.property(
        Some(PropertySetterPublicAccessV1::Public),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    let mut setter = fixture.accessor(AccessorRole::Setter);
    setter.parameters[0] = unit_type();
    assert_eq!(
        validate(
            vec![read_write],
            vec![
                fixture.accessor(AccessorRole::Getter).build(),
                setter.build(),
            ],
        ),
        Err(PropertyAccessorClosureValidationError::ParameterType {
            accessor: fixture.setter,
            expected: Box::new(fixture.value_type.clone()),
            actual: Box::new(unit_type()),
        })
    );
}

#[test]
fn checks_accessor_results_and_public_lookup_access() {
    let fixture = Fixture::top_level("results");
    let read_only = fixture.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.result = unit_type();
    assert_eq!(
        validate(vec![read_only], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::Result {
            accessor: fixture.getter,
            expected: Box::new(fixture.value_type.clone()),
            actual: Box::new(unit_type()),
        })
    );

    let read_write = fixture.property(
        Some(PropertySetterPublicAccessV1::Public),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );
    let mut setter = fixture.accessor(AccessorRole::Setter);
    setter.result = fixture.value_type.clone();
    assert_eq!(
        validate(
            vec![read_write],
            vec![
                fixture.accessor(AccessorRole::Getter).build(),
                setter.build(),
            ],
        ),
        Err(PropertyAccessorClosureValidationError::Result {
            accessor: fixture.setter,
            expected: Box::new(unit_type()),
            actual: Box::new(fixture.value_type.clone()),
        })
    );

    let nominal = Fixture::nominal("lookup", SourceNominalKind::Class);
    let property = nominal.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::PublicSlot,
    );
    assert_eq!(
        validate(
            vec![property],
            vec![nominal.accessor(AccessorRole::Getter).build()],
        ),
        Err(PropertyAccessorClosureValidationError::Access {
            accessor: nominal.getter,
            expected: PublicLookupAccessV1::PublicSlot,
            actual: PublicLookupAccessV1::DirectOnly,
        })
    );
}

#[test]
fn rejects_nonordinary_or_specialized_accessor_effects() {
    let fixture = Fixture::top_level("effects");
    let property = fixture.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
    );

    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.effects = effects(
        Effect::Suspend,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    );
    assert_eq!(
        validate(vec![property.clone()], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::Execution(
            fixture.getter
        ))
    );

    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.effects = effects(
        Effect::Ordinary,
        CallableImplementationV1::Intrinsic(crate::IntrinsicFunctionKind::GcCollect),
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    );
    assert_eq!(
        validate(vec![property.clone()], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::Implementation {
            accessor: fixture.getter,
            actual: CallableImplementationV1::Intrinsic(crate::IntrinsicFunctionKind::GcCollect),
        })
    );

    let operator_role = CallableOperatorRoleV1::Language(CallableOperatorV1::Get);
    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.effects = effects(
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        operator_role,
        CallableInfixV1::Ordinary,
    );
    assert_eq!(
        validate(vec![property.clone()], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::OperatorRole {
            accessor: fixture.getter,
            actual: operator_role,
        })
    );

    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.effects = effects(
        Effect::Ordinary,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Infix,
    );
    assert_eq!(
        validate(vec![property], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::Infix(
            fixture.getter
        ))
    );
}

#[test]
fn enforces_const_and_abstract_representation_modalities() {
    let constant = Fixture::top_level("constant");
    let property = constant.property(
        None,
        PropertyRepresentationV1::Const,
        PropertyPublicAccessV1::DirectOnly,
    );
    assert_eq!(
        validate(
            vec![property],
            vec![constant.accessor(AccessorRole::Getter).build()],
        ),
        Ok(())
    );

    let abstract_property = Fixture::nominal("abstracted", SourceNominalKind::Interface);
    let property = abstract_property.property(
        None,
        PropertyRepresentationV1::AbstractSlot,
        PropertyPublicAccessV1::PublicSlot,
    );
    let mut getter = abstract_property.accessor(AccessorRole::Getter);
    getter.access = PublicLookupAccessV1::PublicSlot;
    assert_eq!(
        validate(vec![property.clone()], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::Modality {
            accessor: abstract_property.getter,
            expected: CallableModalityV1::Abstract,
            actual: CallableModalityV1::Final,
        })
    );

    let mut getter = abstract_property.accessor(AccessorRole::Getter);
    getter.modality = CallableModalityV1::Abstract;
    getter.access = PublicLookupAccessV1::PublicSlot;
    assert_eq!(validate(vec![property], vec![getter.build()]), Ok(()));
}

#[test]
fn runtime_properties_require_a_concrete_accessor_in_either_lookup_partition() {
    let fixture = Fixture::nominal("runtime", SourceNominalKind::Interface);
    let read_only = fixture.property(
        None,
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::PublicSlot,
    );
    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.modality = CallableModalityV1::Abstract;
    getter.access = PublicLookupAccessV1::PublicSlot;
    assert_eq!(
        validate(vec![read_only], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::SourceForm {
            accessor: fixture.getter,
            implementation: AccessorForm::Body,
            modality: CallableModalityV1::Abstract,
        })
    );

    let read_write = fixture.property(
        Some(PropertySetterPublicAccessV1::Public),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::PublicSlot,
    );
    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.modality = CallableModalityV1::Abstract;
    getter.access = PublicLookupAccessV1::PublicSlot;
    let mut setter = fixture.accessor(AccessorRole::Setter);
    setter.modality = CallableModalityV1::InterfaceDefault;
    setter.access = PublicLookupAccessV1::PublicSlot;
    assert_eq!(
        validate(
            vec![with_source_forms(
                read_write,
                AccessorForm::AbstractSlot,
                Some(AccessorForm::Body)
            )],
            vec![getter.build(), setter.build()]
        ),
        Ok(())
    );

    let restricted = fixture.property(
        Some(PropertySetterPublicAccessV1::Restricted),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::PublicSlot,
    );
    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.modality = CallableModalityV1::Abstract;
    getter.access = PublicLookupAccessV1::PublicSlot;
    assert_eq!(
        validate_support(
            vec![with_source_forms(
                restricted,
                AccessorForm::AbstractSlot,
                Some(AccessorForm::Body)
            )],
            vec![getter.build()],
            vec![
                fixture
                    .accessor(AccessorRole::Setter)
                    .build_source(DeclaredVisibilityV1::Private)
            ],
        ),
        Ok(())
    );
}
