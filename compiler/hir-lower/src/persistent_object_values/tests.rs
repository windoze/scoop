use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{PersistentObjectValueId, PersistentTypeId};

use crate::tests::{core_file, core_source_identity, file, ident, sp, test_source_identity};

fn object(name: &str) -> ast::Decl {
    ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    })
}

fn identities(
    path: &str,
    display_locator: &str,
    name: &str,
) -> (PersistentTypeId, PersistentObjectValueId) {
    let source = file(vec![object(name)]);
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(test_source_identity(path), source),
        Vec::new(),
    ))
    .unwrap();
    let core = core_file();
    let input = crate::DefinedTestSources::try_new(
        vec![crate::ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::CurrentSourceDetails {
            display_locator,
            source_text: "",
        },
    )
    .unwrap();
    let output = crate::lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("the object-value fixture lowers");
    let (object_id, declaration) = output
        .export
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == name)
        .unwrap();
    let type_id = output.export.nominal_identities[object_id]
        .source()
        .expect("a source object has a source nominal identity")
        .concrete_id()
        .expect("an object source identity is non-generic");
    let value_identity = &output.export.object_value_identities[declaration.singleton_value];
    assert_eq!(value_identity.declaration(), object_id);
    assert_eq!(
        value_identity.record().key(),
        output.export.nominal_identities[object_id]
            .source()
            .unwrap()
            .declaration()
    );
    (type_id, value_identity.id())
}

#[test]
fn object_value_identity_uses_the_source_object_but_not_its_locator() {
    let first = identities("src/first.scoop", "/old/tree/first.scoop", "Registry");
    let moved = identities("src/moved.scoop", "/new/tree/moved.scoop", "Registry");
    let renamed = identities("src/first.scoop", "/old/tree/first.scoop", "Other");

    assert_eq!(first, moved);
    assert_ne!(first, renamed);
    assert_ne!(first.0.as_array(), first.1.as_array());
}
