use super::*;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain,
    PackagePath, PersistentConstructorId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

#[test]
fn ordinary_class_init_does_not_prove_constructor_delegation() {
    let f = Fixture::new();
    let site = |owners| {
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            owners,
            DeclarationScope::ConeWide,
        )
        .unwrap()
    };
    let class = SourceDeclarationKey::nominal(
        site(DefinitionOwnerChain::top_level()),
        CanonicalIdentifier::new("Constructed").unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let class_id = PersistentTypeId::from_source_declaration(&class).unwrap();
    let key = SourceDeclarationKey::constructor(
        site(DefinitionOwnerChain::from_outer_to_inner(vec![
            DefinitionOwnerAtom::Type(class_id),
        ])),
        vec![],
    );
    let constructor = DefaultConstructorRefV1::Class {
        declaration: DefaultClassConstructorIdV1::Source(
            PersistentConstructorId::from_source_declaration(&key).unwrap(),
        ),
        owner_type: SignatureTypeKey::Nominal(class_id),
    };
    let mut refs = empty_set();
    refs.constructors = vec![reference(
        &f,
        constructor.clone(),
        vec![use_at(0, ProtectedDefaultReceiverUseV1::None)],
    )];
    refs.types = vec![reference(
        &f,
        SignatureTypeKey::Nominal(class_id),
        vec![use_at(0, ProtectedDefaultReceiverUseV1::None)],
    )];
    let mut input = Input {
        body: body(
            &f,
            DefaultExpressionKindV1::ClassInit {
                constructor: constructor.clone(),
                arguments: vec![],
            },
        ),
        f,
        refs,
        locals: empty_locals(),
        receiver: OptionalTemplateReceiverV1::Absent,
    };
    input
        .validate(&mut Authority::default(), &mut meter())
        .unwrap();
    input.refs.constructors[0] = reference(
        &input.f,
        constructor,
        vec![use_at(
            0,
            ProtectedDefaultReceiverUseV1::ConstructorDelegation,
        )],
    );
    assert!(matches!(
        input.validate(&mut Authority::default(), &mut meter()),
        Err(ProtectedDefaultBodyClosureError::MissingUse { .. })
    ));
}
