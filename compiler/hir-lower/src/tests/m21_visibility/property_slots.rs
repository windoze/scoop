use super::support::parse_and_lower;
use super::*;

const POSITIVE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m21-visibility/property-setter-slots.scoop"
));

#[test]
fn explicit_setters_preserve_independent_slot_contracts_and_override_relations() {
    let output = parse_and_lower(POSITIVE).unwrap();
    let module = output.export.module();
    let base = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Base")
        .unwrap()
        .0;
    let base = module.nominal_identities[base].declaration_id();
    let mut seen = std::collections::BTreeSet::new();
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
        let (declared, constraints) = match property.name.as_str() {
            "internalWriter" => (
                hir::DeclaredVisibility::Internal,
                vec![hir::AccessConstraint::Cone(module.cone)],
            ),
            "protectedWriter" => (
                hir::DeclaredVisibility::Protected,
                vec![hir::AccessConstraint::SubclassesOf(base)],
            ),
            "visible" => (hir::DeclaredVisibility::Public, Vec::new()),
            other => panic!("unexpected fixture property {other}"),
        };
        assert_eq!(function.access.declared, declared);
        assert_eq!(slot.0.constraints(), constraints);
        assert!(seen.insert((owner_name.as_str(), property.name.as_str())));
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
    assert_eq!(
        seen,
        std::collections::BTreeSet::from([
            ("Base", "internalWriter"),
            ("Base", "protectedWriter"),
            ("Base", "visible"),
            ("Child", "internalWriter"),
            ("Child", "protectedWriter"),
            ("Child", "visible"),
        ])
    );
}
