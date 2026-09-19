use super::*;
use scoop_identity::SignatureTypeKey;

#[test]
fn hidden_constructors_never_become_foreign_source_entries() {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let id = fixture.constructor(owner);
    for visibility in [
        DeclaredVisibilityV1::Private,
        DeclaredVisibilityV1::Internal,
    ] {
        let payload = fixture.payload(
            owner,
            CallableTemplateOrigin::Constructor(id),
            vec![],
            SignatureTypeKey::Nominal(nominal(owner)),
        );
        let source = NominalSupportConstructorInterfaceV1::try_new(
            id,
            fixture.access(owner, visibility),
            (*payload).clone(),
        )
        .unwrap();
        assert!(matches!(
            InheritanceConstructorInterfaceV1::try_new(source.clone()),
            Err(InheritanceInterfaceBuildError::ConstructorAccess)
        ));
        let decoded: DecodedInheritanceConstructorInterfaceV1 =
            decode_canonical(&encode(&source).unwrap(), DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture, &mut meter()),
            Err(InheritanceInterfaceResolutionError::Build(
                InheritanceInterfaceBuildError::ConstructorAccess
            ))
        ));
    }
}
#[test]
fn constructor_reference_cannot_be_duplicated_in_the_member_field() {
    let mut bundle = fixture();
    let id = bundle
        .table
        .get(bundle.base.exact)
        .unwrap()
        .constructors()
        .records()[0]
        .declaration();
    bundle.change(bundle.base, |record| {
        record.protected_members = CanonicalProtectedDeclarationRefsV1::try_new(vec![
            ProtectedDeclarationRefV1::Constructor(id),
        ])
        .unwrap()
    });
    let decoded: DecodedCanonicalNominalInheritanceInterfacesV1 =
        decode_canonical(&encode(&bundle.table).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut bundle.fixture, &mut meter()),
        Err(InheritanceInterfaceResolutionError::Build(
            InheritanceInterfaceBuildError::ConstructorInMembers
        ))
    ));
}

#[test]
fn constructor_table_reader_refuses_duplicate_and_reversed_identity_order() {
    use scoop_wire::{Encoder, WireEncode};
    struct Entries<'a>([&'a InheritanceConstructorInterfaceV1; 2]);
    impl WireEncode for Entries<'_> {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.array(2)?;
            self.0[0].encode(encoder)?;
            self.0[1].encode(encoder)
        }
    }
    let mut bundle = fixture();
    let base = &bundle
        .table
        .get(bundle.base.exact)
        .unwrap()
        .constructors()
        .records()[0];
    let derived = &bundle
        .table
        .get(bundle.derived.exact)
        .unwrap()
        .constructors()
        .records()[0];
    let reversed = if base.declaration() > derived.declaration() {
        [base, derived]
    } else {
        [derived, base]
    };
    for entries in [[base, base], reversed] {
        let decoded: DecodedCanonicalInheritanceConstructorsV1 =
            decode_canonical(&encode(&Entries(entries)).unwrap(), DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut bundle.fixture, &mut meter()),
            Err(InheritanceInterfaceResolutionError::Build(
                InheritanceInterfaceBuildError::ConstructorOrder
            ))
        ));
    }
}
