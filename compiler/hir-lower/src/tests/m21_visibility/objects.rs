use super::support::parse_and_lower;
use super::*;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/object-protected.scoop"
));

#[test]
fn objects_and_companions_use_backing_classes_for_protected_access() {
    let output = parse_and_lower(POSITIVE).unwrap();
    let module = output.export.module();
    let base = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Base")
        .unwrap()
        .0;
    let base = module.nominal_identities[base].declaration_id();
    let mut owners = std::collections::BTreeSet::new();
    for (_, object) in module.objects.iter() {
        let backing = &module.classes[object.backing_class];
        let hir::Type::Class(application) = module.types[backing.base_class.unwrap()] else {
            panic!("object has a typed class base")
        };
        assert_eq!(module.class_applications[application].template, base);
        let owner = match object.owner {
            Some(hir::NominalOwner::Class(owner)) => module.classes[owner].name.as_str(),
            None => "",
            other => panic!("unexpected fixture owner {other:?}"),
        };
        assert!(owners.insert((owner, object.name.as_str())));
    }
    assert_eq!(
        owners,
        std::collections::BTreeSet::from([("Holder", "Companion"), ("", "Singleton")])
    );
    assert_eq!(module.objects.len(), 2);
}

#[test]
fn unrelated_objects_and_private_base_constructors_remain_inaccessible() {
    for (source, expected, occurrence) in [
        (
            "open class Base { protected fun secret(): Int = 1 }\nobject Other { fun probe(base: Base): Int = base.secret() }\nfun main() {}",
            "method `secret` is not accessible here",
            "secret() }",
        ),
        (
            "open class Base private constructor()\nobject Singleton : Base()\nfun main() {}",
            "constructor of type `Base` is not accessible here",
            "Base()",
        ),
    ] {
        let errors = parse_and_lower(source).unwrap_err();
        let diagnostic = errors
            .iter()
            .find(|error| error.message == expected)
            .unwrap();
        assert_eq!(
            diagnostic.span.unwrap().start as usize,
            source.find(occurrence).unwrap()
        );
    }
}
