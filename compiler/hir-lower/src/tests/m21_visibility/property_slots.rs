use super::support::parse_and_lower;
use super::*;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/property-setter-slots.scoop"
));
const NEGATIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/errors/property-setter-slot-narrowing.scoop"
));

#[test]
fn explicit_setters_preserve_independent_slot_contracts_and_override_relations() {
    let output = parse_and_lower(POSITIVE).unwrap();
    let module = output.export.module();
    let mut rows = Vec::new();
    for (_, property) in module.properties.iter() {
        let hir::PropertyOwner::Class(owner) = property.owner else {
            continue;
        };
        let owner_name = &module.classes[owner].name;
        if !matches!(owner_name.as_str(), "Base" | "Child") {
            continue;
        }
        let hir::PropertyCapability::ReadWrite { setter, .. } = property.capability else {
            panic!("mutable fixture property");
        };
        let setter = &module.property_setters[setter];
        let hir::PropertyAccessorImplementation::Body(function_id) = setter.implementation else {
            panic!("explicit setter body");
        };
        let function = &module.functions[function_id];
        let slot = function.access.slot.as_ref().unwrap();
        let domain = slot
            .0
            .constraints()
            .iter()
            .map(|constraint| match constraint {
                hir::AccessConstraint::Cone(_) => "Cone".to_owned(),
                hir::AccessConstraint::SubclassesOf(owner) => {
                    let (_, class) = module
                        .classes
                        .iter()
                        .find(|(id, _)| module.nominal_identities[*id].declaration_id() == *owner)
                        .expect("fixture slot retains its original class declaration");
                    format!("SubclassesOf({})", class.name)
                }
                other => panic!("unexpected fixture slot constraint {other:?}"),
            })
            .collect::<Vec<_>>()
            .join(" & ");
        rows.push(format!(
            "{owner_name}.{}: {:?}, slot={}\n",
            property.name,
            function.access.declared,
            if slot.0.is_universal() {
                "Public"
            } else {
                &domain
            }
        ));
        if owner_name == "Child" {
            assert!(!function.access.lookup.0.is_universal());
            let [hir::PropertyReference::Local(inherited_property)] = property.overrides.as_slice()
            else {
                panic!("one overridden property");
            };
            let setter = module.properties[*inherited_property]
                .capability
                .setter()
                .expect("the inherited property is mutable");
            let hir::PropertyAccessorImplementation::Body(inherited) =
                module.property_setters[setter].implementation
            else {
                panic!("the inherited setter has a body");
            };
            let inherited = &module.functions[inherited];
            assert_eq!(inherited.access.slot.as_ref(), Some(slot));
            let hir::MethodDispatch::Virtual(family) = inherited.method.unwrap().dispatch else {
                panic!("base setter owns a virtual family");
            };
            assert_eq!(
                function.method.unwrap().dispatch,
                hir::MethodDispatch::FinalOverride(family)
            );
        }
    }
    rows.sort();
    assert_eq!(
        rows.concat(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m21-visibility/property-setter-slots.contracts.snap"
        ))
    );
}

#[test]
fn explicit_setter_cannot_narrow_the_inherited_public_contract() {
    let errors = parse_and_lower(NEGATIVE).unwrap_err();
    let error = errors
        .iter()
        .find(|error| {
            error.message
                == "visibility of `$set$value` does not cover inherited slot `Base.$set$value`"
        })
        .unwrap_or_else(|| panic!("missing setter coverage diagnostic: {errors:?}"));
    assert_eq!(
        error.span.unwrap().start as usize,
        NEGATIVE.find("override var value").unwrap() + "override var ".len()
    );
}
