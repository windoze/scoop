use super::support::parse_and_lower;
use super::*;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/private-property-accessors.scoop"
));
const NEGATIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/errors/private-abstract-property.scoop"
));

#[test]
fn private_accessors_are_final_without_erasing_public_getter_slots() {
    let output = parse_and_lower(POSITIVE).unwrap();
    let export = output.export.module();
    let mut rows = Vec::new();
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
                rows.push(format!(
                    "{name}.{}.{role}: Private {:?} {:?}\n",
                    property.name, method.modifier, method.dispatch
                ));
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
    rows.sort();
    assert_eq!(
        rows.concat(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m21-visibility/private-property-accessors.snap"
        ))
    );
}

#[test]
fn private_interface_properties_still_require_accessor_bodies() {
    let errors = parse_and_lower(NEGATIVE).unwrap_err();
    let error = errors
        .iter()
        .find(|error| {
            error.message == "private interface property `missing` must provide every accessor body"
        })
        .unwrap_or_else(|| panic!("{errors:?}"));
    let start = NEGATIVE.find("missing").unwrap();
    assert_eq!(
        error.span.unwrap(),
        ast::Span {
            start: start as u32,
            end: (start + "missing".len()) as u32
        }
    );
}

#[test]
fn bare_interface_property_assignment_checks_the_setter_value_type() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m21-visibility/errors/interface-property-write-type.scoop"
    ));
    let errors = parse_and_lower(source).unwrap_err();
    let error = errors
        .iter()
        .find(|error| {
            error.message == "cannot assign value of type Boolean to property `value` of type Int"
        })
        .unwrap_or_else(|| panic!("{errors:?}"));
    let start = source.find("true").unwrap();
    assert_eq!(
        error.span.unwrap(),
        ast::Span {
            start: start as u32,
            end: (start + 4) as u32
        }
    );
}
