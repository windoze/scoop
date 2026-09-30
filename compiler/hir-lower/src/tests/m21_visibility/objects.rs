use super::support::parse_and_lower;
use super::*;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/object-protected.scoop"
));
const NEGATIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/errors/object-protected-receiver.scoop"
));

#[test]
fn objects_and_companions_use_backing_classes_for_protected_access() {
    let output = parse_and_lower(POSITIVE).unwrap();
    let module = output.export.module();
    let mut owners = module
        .objects
        .iter()
        .map(|(_, object)| {
            let backing = &module.classes[object.backing_class];
            let hir::Type::Class(base) = module.types[backing.base_class.unwrap()] else {
                panic!("object has a typed class base")
            };
            let name = match object.owner {
                Some(hir::NominalOwner::Class(owner)) => {
                    format!("{}.{}", module.classes[owner].name, object.name)
                }
                None => object.name.clone(),
                other => panic!("unexpected fixture owner {other:?}"),
            };
            format!(
                "{name} : {}\n",
                module.classes[module
                    .nominal_identities
                    .class_id(module.class_applications[base].template)
                    .expect("an application retains its declaration")]
                .name
            )
        })
        .collect::<Vec<_>>();
    owners.sort();
    assert_eq!(
        owners.concat(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m21-visibility/object-protected.owners.snap"
        ))
    );
    assert_eq!(module.objects.len(), 2);
}

#[test]
fn object_protected_access_still_rejects_base_and_sibling_receivers() {
    let errors = parse_and_lower(NEGATIVE).unwrap_err();
    for (receiver, occurrence) in [("Base", "other.secret()"), ("Sibling", "peer.secret()")] {
        let message = format!(
            "protected method `secret` cannot be accessed through receiver of static type `{receiver}`; receiver must be `Singleton` or one of its subclasses"
        );
        let diagnostic = errors
            .iter()
            .find(|error| error.message == message)
            .unwrap();
        assert_eq!(
            diagnostic.span.unwrap().start as usize,
            NEGATIVE.find(occurrence).unwrap() + occurrence.find('.').unwrap() + 1
        );
    }
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
