use super::*;
use crate::{
    CanonicalNestedMemberRefsV1, NominalDeclarationDetailsV1, NominalInheritanceModalityV1,
};

#[test]
fn declaration_details_reject_modality_incompatible_with_nominal_kind() {
    let record = Fixture::new().record();
    let details = record.declaration_details();
    let corrupt = NominalDeclarationDetailsV1::new(
        NominalInheritanceModalityV1::Open,
        details.declared_visibility(),
        details.constructors().clone(),
        details.members().clone(),
        details.children().clone(),
        details.dispatch_order().clone(),
        details.dispatch_selections().clone(),
        details.primary_value_constructor(),
        details.instantiation_conditions().clone(),
    );
    assert_eq!(
        rebuild(&record, corrupt),
        Err(NominalInterfaceRecordBuildError::Modality {
            kind: PublicNominalKindV1::Struct,
            modality: NominalInheritanceModalityV1::Open
        })
    );
}

#[test]
fn public_constructor_must_belong_to_the_complete_declaration() {
    let record = Fixture::new().record();
    let details = record.declaration_details();
    let corrupt = NominalDeclarationDetailsV1::new(
        details.modality(),
        details.declared_visibility(),
        CanonicalPersistentIdsV1::empty(),
        details.members().clone(),
        details.children().clone(),
        details.dispatch_order().clone(),
        details.dispatch_selections().clone(),
        details.primary_value_constructor(),
        details.instantiation_conditions().clone(),
    );
    assert_eq!(
        rebuild(&record, corrupt),
        Err(NominalInterfaceRecordBuildError::UndeclaredConstructor(
            record.constructors().values()[0]
        ))
    );
}

#[test]
fn public_member_must_belong_to_the_complete_declaration() {
    let record = Fixture::new().record();
    let details = record.declaration_details();
    let corrupt = NominalDeclarationDetailsV1::new(
        details.modality(),
        details.declared_visibility(),
        details.constructors().clone(),
        CanonicalNestedMemberRefsV1::try_new(vec![]).unwrap(),
        details.children().clone(),
        details.dispatch_order().clone(),
        details.dispatch_selections().clone(),
        details.primary_value_constructor(),
        details.instantiation_conditions().clone(),
    );
    assert_eq!(
        rebuild(&record, corrupt),
        Err(NominalInterfaceRecordBuildError::UndeclaredMember(
            record.members().members()[0]
        ))
    );
}

#[test]
fn support_partition_cannot_carry_public_lookup_relationships() {
    let record = Fixture::new().record();
    let owner = record.declaration();
    assert_eq!(
        CanonicalNominalInterfacesV1::with_support(vec![], vec![record]),
        Err(NominalInterfaceSetBuildError::SupportLookup(owner))
    );
}

fn rebuild(
    record: &NominalInterfaceRecordV1,
    details: NominalDeclarationDetailsV1,
) -> Result<NominalInterfaceRecordV1, NominalInterfaceRecordBuildError> {
    NominalInterfaceRecordV1::try_new(
        record.declaration(),
        record.kind(),
        record.type_parameters().clone(),
        record.exact_supertypes().clone(),
        record.constructors().clone(),
        record.members().clone(),
        record.nested_bindings().clone(),
        record.source_shape().clone(),
        details,
    )
}

#[test]
fn struct_constructor_set_requires_the_primary_declaration_reference() {
    let record = Fixture::new().record();
    let details = record.declaration_details();
    let missing = NominalDeclarationDetailsV1::new(
        details.modality(),
        details.declared_visibility(),
        details.constructors().clone(),
        details.members().clone(),
        details.children().clone(),
        details.dispatch_order().clone(),
        details.dispatch_selections().clone(),
        None,
        details.instantiation_conditions().clone(),
    );
    assert_eq!(
        rebuild(&record, missing),
        Err(NominalInterfaceRecordBuildError::MissingPrimaryValueConstructor)
    );
}
