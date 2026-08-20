use super::support::parse_and_lower;
use super::*;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/private-property-accessors.scoop"
));

#[test]
fn private_accessors_are_final_without_erasing_public_getter_slots() {
    let output = parse_and_lower(POSITIVE).unwrap();
    let export = output.export.module();
    let mut seen = std::collections::BTreeSet::new();
    let mut public_slots = 0;
    for (_, property) in export.properties.iter() {
        let name = match property.owner {
            hir::PropertyOwner::Class(owner) => &export.classes[owner].name,
            hir::PropertyOwner::Interface(owner) => &export.interfaces[owner].name,
            _ => continue,
        };
        if !matches!(name.as_str(), "View" | "Owner") {
            continue;
        }
        if property.access.declared == hir::DeclaredVisibility::Private {
            assert_eq!(property.modifier, hir::MethodModifier::Final);
        }
        let getter = &export.property_getters[property.capability.getter()];
        let setter = property
            .capability
            .setter()
            .map(|id| &export.property_setters[id]);
        for (role, access, implementation) in
            std::iter::once(("getter", &getter.access, getter.implementation))
                .chain(setter.map(|setter| ("setter", &setter.access, setter.implementation)))
        {
            let hir::PropertyAccessorImplementation::Body(function) = implementation else {
                continue;
            };
            let function = &export.functions[function];
            let method = function.method.unwrap();
            if access.declared == hir::DeclaredVisibility::Private {
                assert_eq!(method.modifier, hir::MethodModifier::Final);
                assert_eq!(method.dispatch, hir::MethodDispatch::Direct);
                assert!(
                    function.access.slot.is_none(),
                    "{name}.{}.{role}",
                    property.name
                );
                assert!(access.slot.is_none(), "{name}.{}.{role}", property.name);
                assert!(seen.insert((name.as_str(), property.name.as_str(), role)));
            } else {
                assert!(matches!(
                    method.dispatch,
                    hir::MethodDispatch::Interface(_) | hir::MethodDispatch::Virtual(_)
                ));
                public_slots += 1;
            }
        }
    }
    assert_eq!(public_slots, 2);
    assert_eq!(
        seen,
        std::collections::BTreeSet::from([
            ("Owner", "value", "setter"),
            ("View", "cache", "getter"),
            ("View", "cache", "setter"),
            ("View", "current", "setter"),
            ("View", "secret", "getter"),
        ])
    );
}
