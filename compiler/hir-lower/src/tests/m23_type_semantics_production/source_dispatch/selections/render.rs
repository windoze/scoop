use super::*;

pub(super) fn render(
    output: &hir::DependencyHirOutput,
    table: &hir::CanonicalInheritanceSourceSlotSelectionsV1,
) -> String {
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
    let mut result = String::new();
    for (name, owner) in super::super::support::owners(output) {
        result.push_str(&format!("{name}\n"));
        let mut lines = table
            .records()
            .iter()
            .filter(|record| record.owner() == owner)
            .map(|record| {
                let selection = match record.selection() {
                    Selection::Abstract => "abstract".to_owned(),
                    Selection::Concrete(callable) => format!("concrete {}", callables[&callable]),
                    Selection::InterfaceDefault(callable) => {
                        format!("default {}", callables[&callable])
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
