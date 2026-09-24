use crate::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use super::*;
use scoop_identity::AccessorRole;

#[test]
fn restricted_setter_overlap_does_not_grant_getter_or_owner_access() {
    let mut fixture = SourceFixture::default();
    let owner = fixture.class("PropertyOwner");
    let getter = fixture.accessor(owner, AccessorRole::Getter, unit());
    let setter = fixture.accessor(owner, AccessorRole::Setter, unit());
    let (CallableTemplateOrigin::Accessor(getter_id), CallableTemplateOrigin::Accessor(setter_id)) =
        (getter, setter)
    else {
        unreachable!()
    };
    let property = *fixture.property_types.keys().next().unwrap();
    let getter_payload = fixture.payload(owner, getter, vec![], unit());
    let setter_payload = fixture.payload(owner, setter, vec![unit()], unit());
    let property = PropertyInterfaceRecordV1::try_new(
        PropertyDeclarationId::Property(property),
        PublicDeclarationOwnerV1::Nominal(owner.source),
        CanonicalBinderListV1::try_new(vec![]).unwrap(),
        None,
        unit(),
        Accessors::try_read_write(
            AccessorSource::new(getter_id, AccessorForm::Body),
            AccessorSource::new(setter_id, AccessorForm::Body),
        )
        .unwrap(),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
        crate::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    let public = public_with_properties(
        vec![
            callable(getter, &getter_payload),
            callable(setter, &setter_payload),
        ],
        vec![property],
    );
    let access = fixture.access(owner, DeclaredVisibilityV1::Protected);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.graph.records.values(),
        &fixture.graph,
        &mut meter(),
    )
    .unwrap();
    callables::validate::<&str>(
        setter,
        &access,
        &setter_payload,
        &public,
        &graph,
        &mut meter(),
        &path(),
    )
    .unwrap();
    assert!(
        callables::validate::<&str>(
            getter,
            &access,
            &getter_payload,
            &public,
            &graph,
            &mut meter(),
            &path()
        )
        .is_err()
    );
    let original = fixture.graph.access[&owner.source].clone();
    fixture.graph.access.insert(
        owner.source,
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Internal,
            vec![],
            original.definition_origin().clone(),
        )
        .unwrap(),
    );
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.graph.records.values(),
        &fixture.graph,
        &mut meter(),
    )
    .unwrap();
    assert!(
        callables::validate::<&str>(
            setter,
            &access,
            &setter_payload,
            &public,
            &graph,
            &mut meter(),
            &path()
        )
        .is_err()
    );
}
