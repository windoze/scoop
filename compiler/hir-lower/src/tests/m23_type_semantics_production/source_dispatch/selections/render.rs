use super::*;

pub(super) fn render(
    output: &hir::DependencyHirOutput,
    table: &hir::CanonicalNominalInheritanceInterfacesV1,
) -> String {
    let (callables, slots) = labels(output);
    let mut result = String::new();
    for (name, owner) in super::super::support::owners(output) {
        result.push_str(&format!("{name}\n"));
        let mut lines = table
            .get(owner)
            .unwrap()
            .slots()
            .records()
            .iter()
            .map(|record| {
                let selection = match record.implementation() {
                    hir::InheritanceSlotImplementationV1::Abstract => "abstract".to_owned(),
                    hir::InheritanceSlotImplementationV1::Concrete(callable) => {
                        format!("concrete {}", callables[&callable.declaration()])
                    }
                    hir::InheritanceSlotImplementationV1::InterfaceDefault(callable) => {
                        format!("default {}", callables[&callable.declaration()])
                    }
                };
                format!("  {} -> {selection}\n", slots[&record.slot()])
            })
            .collect::<Vec<_>>();
        lines.sort();
        result.extend(lines);
    }
    result
}

pub(super) fn labels(
    output: &hir::DependencyHirOutput,
) -> (
    BTreeMap<Callable, &str>,
    BTreeMap<PersistentDispatchSlotId, &str>,
) {
    let export = output.output().export.module();
    let mut callables = BTreeMap::new();
    let mut slots = BTreeMap::new();
    for (id, function) in export.functions.iter() {
        let callable = match &export.function_identities[id] {
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                Callable::Function(record.id())
            }
            hir::HirFunctionIdentity::PropertyAccessor(
                hir::HirPropertyAccessorFunction::Getter(id),
            ) => Callable::Getter(export.property_accessor_identities[*id].record().id()),
            hir::HirFunctionIdentity::PropertyAccessor(
                hir::HirPropertyAccessorFunction::Setter(id),
            ) => Callable::Setter(export.property_accessor_identities[*id].record().id()),
            _ => continue,
        };
        callables.insert(callable, function.name.as_str());
        if let Some(method) = function.method {
            match method.dispatch {
                hir::MethodDispatch::Direct => {}
                hir::MethodDispatch::Virtual(family)
                | hir::MethodDispatch::FinalOverride(family) => {
                    let root = export
                        .dispatch_slot_identities
                        .virtual_root(family)
                        .unwrap();
                    slots.insert(
                        export.dispatch_slot_identities[family].id(),
                        export.functions[root].name.as_str(),
                    );
                }
                hir::MethodDispatch::Interface(member) => {
                    slots.insert(
                        export.dispatch_slot_identities[member].id(),
                        function.name.as_str(),
                    );
                }
            }
        }
    }
    (callables, slots)
}
