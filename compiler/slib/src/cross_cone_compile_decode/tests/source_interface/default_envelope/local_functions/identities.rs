use scoop_identity::{DefinitionOwnerAtom, PersistentGenericFunctionId};

use super::*;

pub(super) fn add(fixture: &mut CallableSourceSurface, arity: u32) -> CallableTemplateOrigin {
    let original = OdrFreeHirFoundation::try_new(fixture.foundation.clone()).unwrap();
    let CallableTemplateOrigin::Function(current) = fixture.owner else {
        panic!("fixture function")
    };
    let template = &fixture.interface.default_templates().records()[0];
    let source = template.definition_origin().origin().source().clone();
    let parameter = if arity == 0 {
        template.result().clone()
    } else {
        SignatureTypeKey::Binder { depth: 0, index: 0 }
    };
    let key = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            fixture.cone.identity(),
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Function(current)]),
            DeclarationScope::LexicalScoped {
                source,
                path: local_path(0, 1),
            },
        )
        .unwrap(),
        CanonicalIdentifier::new("local").unwrap(),
        arity,
        None,
        vec![parameter],
    );
    let (declaration, subject) = if arity == 0 {
        let local = CborIdentityRecord::<PersistentFunctionId, _>::from_key(key).unwrap();
        let current_record = CborIdentityRecord::<PersistentFunctionId, _>::from_key(
            SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    fixture.cone.identity(),
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new("consume").unwrap(),
                0,
                None,
                vec![template.result().clone()],
            ),
        )
        .unwrap();
        assert_eq!(current_record.id(), current);
        let id = local.id();
        fixture
            .foundation
            .set_functions(vec![current_record, local])
            .unwrap();
        (
            CallableTemplateOrigin::Function(id),
            DefinitionOriginSubject::Function(id),
        )
    } else {
        let local = CborIdentityRecord::<PersistentGenericFunctionId, _>::from_key(key).unwrap();
        let id = local.id();
        fixture
            .foundation
            .set_generic_functions(vec![local])
            .unwrap();
        (
            CallableTemplateOrigin::GenericFunction(id),
            DefinitionOriginSubject::GenericFunction(id),
        )
    };
    let mut origins: Vec<_> = fixture
        .interface
        .nominal_interfaces()
        .all_records()
        .map(|record| {
            let subject = match record.declaration() {
                SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
                SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
            };
            original.definition_origin(subject).unwrap().clone()
        })
        .collect();
    origins.push(
        original
            .definition_origin(DefinitionOriginSubject::Function(current))
            .unwrap()
            .clone(),
    );
    origins.push(DefinitionOriginRecord::new(
        subject,
        template.definition_origin().origin().clone(),
    ));
    fixture.foundation.set_definition_origins(origins).unwrap();
    declaration
}
