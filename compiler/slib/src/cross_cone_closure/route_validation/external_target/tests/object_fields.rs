use super::*;
use scoop_identity::GeneratedNominalKey;

#[test]
fn object_backing_fields_follow_their_typed_source_object() {
    let fixture = Fixture::new(Case::Valid);
    let target = ExternalHirTargetV1::Field(fixture.field);
    let interface = empty_interface();
    let mut authority = CanonicalCrossConeRouteAuthority::try_new(
        cone("consumer"),
        &fixture.identities,
        &interface,
        &[],
        &[],
        1,
    )
    .unwrap();
    assert_eq!(
        authority.external_hir_target_origin(target).unwrap(),
        fixture.provider
    );
    assert_eq!(
        authority.external_hir_target_binding_root(target).unwrap(),
        fixture.root
    );
}

#[test]
fn object_backing_fields_reject_a_different_property_owner_or_provider() {
    for case in [Case::WrongOwner, Case::WrongProvider] {
        let fixture = Fixture::new(case);
        let interface = empty_interface();
        let mut authority = CanonicalCrossConeRouteAuthority::try_new(
            cone("consumer"),
            &fixture.identities,
            &interface,
            &[],
            &[],
            1,
        )
        .unwrap();
        assert_eq!(
            authority.external_hir_target_origin(ExternalHirTargetV1::Field(fixture.field)),
            Err(
                CrossConeHirReferenceAuthorityError::ObjectFieldOwnerMismatch(Box::new(
                    ExternalObjectFieldOwnerMismatch {
                        field: fixture.field,
                        property: fixture.property,
                    }
                ))
            ),
        );
    }
}

#[test]
fn object_backing_fields_require_an_object_source_declaration() {
    let fixture = Fixture::new(Case::WrongKind);
    let interface = empty_interface();
    let mut authority = CanonicalCrossConeRouteAuthority::try_new(
        cone("consumer"),
        &fixture.identities,
        &interface,
        &[],
        &[],
        1,
    )
    .unwrap();
    let target = ExternalHirTargetV1::Field(fixture.field);
    assert_eq!(
        authority.external_hir_target_origin(target),
        Err(CrossConeHirReferenceAuthorityError::NoPublicBindingRoot { target })
    );
}

#[derive(Clone, Copy)]
enum Case {
    Valid,
    WrongOwner,
    WrongProvider,
    WrongKind,
}

struct Fixture {
    identities: scoop_identity::ValidatedIdentityGraph,
    provider: ConeIdentity,
    field: PersistentFieldId,
    property: PersistentPropertyId,
    root: BindingTarget,
}

impl Fixture {
    fn new(case: Case) -> Self {
        let provider = cone("provider");
        let source = source_nominal::<PersistentTypeId>(
            provider,
            "Registry",
            if matches!(case, Case::WrongKind) {
                SourceNominalKind::Class
            } else {
                SourceNominalKind::Object
            },
            0,
        );
        let other =
            source_nominal::<PersistentTypeId>(provider, "Other", SourceNominalKind::Object, 0);
        let property = CborIdentityRecord::<PersistentPropertyId, _>::from_key(
            SourceDeclarationKey::property(
                source_site(
                    if matches!(case, Case::WrongProvider) {
                        cone("other")
                    } else {
                        provider
                    },
                    DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                        if matches!(case, Case::WrongOwner) {
                            other.id()
                        } else {
                            source.id()
                        },
                    )]),
                ),
                identifier("value"),
            ),
        )
        .unwrap();
        let backing = CborIdentityRecord::<PersistentTypeId, _>::from_key(
            GeneratedNominalKey::ObjectBackingClass {
                object: source.id(),
            },
        )
        .unwrap();
        let field = CborIdentityRecord::<PersistentFieldId, _>::from_key(
            FieldIdentityKey::object_backing_property(backing.key(), property.id()).unwrap(),
        )
        .unwrap();
        let root = BindingTarget::type_name(source.key()).unwrap();
        let (field_id, property_id) = (field.id(), property.id());
        let mut pending = PendingIdentityValidation::new();
        register(&mut pending, source);
        register(&mut pending, other);
        register(&mut pending, property);
        register(&mut pending, backing);
        register(&mut pending, field);
        Self {
            identities: pending.finish().unwrap(),
            provider,
            field: field_id,
            property: property_id,
            root,
        }
    }
}
