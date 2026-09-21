use super::*;
use hir::concrete as local;

pub(super) fn verify(
    output: &hir::DependencyHirOutput,
    mir: &scoop_mir::Module,
    table: &hir::CanonicalInheritanceSourceSlotSelectionsV1,
) {
    let local = output.output().local.module();
    let inventory = project(output);
    let mut checked = 0;
    let mut interfaces = |owner, implementations: &[local::InterfaceImplementation]| {
        for implementation in implementations {
            for method in &implementation.methods {
                let slot = local
                    .dispatch_slot_identities
                    .interface_slot(implementation.interface, method.slot)
                    .id();
                let selection = table.get(owner, slot).unwrap();
                match method.target {
                    local::InterfaceImplementationTarget::Abstract { .. } => {
                        assert_eq!(selection, Selection::Abstract)
                    }
                    local::InterfaceImplementationTarget::Method(function) => {
                        let target = &local.functions[function];
                        let local::MethodDispatch::Interface { .. } =
                            target.receiver.method().unwrap().dispatch
                        else {
                            let Selection::Concrete(callable) = selection else {
                                panic!("class/value method must remain concrete")
                            };
                            assert_eq!(template(callable), target.materialization.template());
                            checked += 1;
                            continue;
                        };
                        let Selection::InterfaceDefault(callable) = selection else {
                            panic!("interface body must remain a default")
                        };
                        assert_eq!(template(callable), target.materialization.template());
                    }
                }
                checked += 1;
            }
        }
    };
    for (id, class) in local.classes.iter() {
        let owner_type = local
            .objects
            .iter()
            .find(|(_, object)| object.backing_class == id)
            .map(|(_, object)| local.object_types[object.object_type].canonical_type)
            .unwrap_or(class.canonical_type);
        let owner = local.exact_type_identities[owner_type].id();
        let Some(source) = inventory.get(owner) else {
            continue;
        };
        interfaces(owner, &class.interface_implementations);
        let schema = source
            .slot_schemas()
            .get(hir::InheritanceSlotSchemaRoleV1::ClassVtable)
            .unwrap();
        let mir_class = &mir
            .classes
            .iter()
            .find(|(_, entry)| entry.name == class.name)
            .unwrap()
            .1;
        assert_eq!(schema.slots().len(), mir_class.vtable.len());
        for (slot, target) in schema.slots().iter().zip(&mir_class.vtable) {
            let scoop_mir::TableSlot::Function(function) = target else {
                panic!("source fixture selects an ordinary function")
            };
            let actual = mir
                .meta
                .source_callable_materializations
                .get(*function)
                .unwrap()
                .materialization();
            match table.get(owner, *slot).unwrap() {
                Selection::Concrete(callable) => assert_eq!(template(callable), actual.template()),
                Selection::Abstract => {
                    let function = local
                        .functions
                        .iter()
                        .find(|(_, function)| function.materialization == actual)
                        .unwrap()
                        .1;
                    assert_eq!(
                        function.receiver.method().unwrap().modifier,
                        local::MethodModifier::Abstract
                    );
                }
                Selection::InterfaceDefault(_) => {
                    panic!("a class vtable cannot select an interface declaration")
                }
            }
        }
    }
    for (_, value) in local.structs.iter() {
        let owner = local.exact_type_identities[value.canonical_type].id();
        interfaces(owner, &value.interface_implementations);
    }
    for (_, value) in local.enums.iter() {
        let owner = local.exact_type_identities[value.canonical_type].id();
        interfaces(owner, &value.interface_implementations);
    }
    assert!(checked != 0 || !table.records().is_empty());
}
