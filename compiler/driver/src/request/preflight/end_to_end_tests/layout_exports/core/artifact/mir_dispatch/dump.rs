use super::*;
use scoop_identity::{
    CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticCatalog, PropertyAccessorKey,
    PropertyOwner,
};

pub(super) fn check(replay: &Replay<'_>, name: &str) {
    let identities = replay.source.metadata().identities;
    let coordinates = [ConeCoordinate::reserved_core()];
    let names = ExactTypeDiagnosticCatalog::try_new(identities, &coordinates).unwrap();
    let exact = |id| {
        CanonicalExactTypeDiagnosticName::from_validated_graph(id, &names)
            .unwrap()
            .into_string()
    };
    let mut rows = Vec::new();
    for record in replay.source.section().inheritance().records() {
        let owner = exact(record.owner());
        if !owner.contains("SharedDispatch") {
            continue;
        }
        for table in record.slot_schemas().records() {
            let table_name = match table.role() {
                hir::InheritanceSlotSchemaRoleV1::ClassVtable => "vtable".into(),
                hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => {
                    format!("itable {}", exact(interface_exact))
                }
            };
            for (position, slot) in table.slots().iter().enumerate() {
                let slot = record.slots().get(table.role(), *slot).unwrap();
                let target = match slot.implementation() {
                    hir::InheritanceSlotImplementationV1::Abstract(target) => {
                        format!("abstract {:?}", target.declaration())
                    }
                    hir::InheritanceSlotImplementationV1::Concrete(target) => {
                        format!("direct {:?}", target.declaration())
                    }
                    hir::InheritanceSlotImplementationV1::InterfaceDefault(target) => {
                        format!("default {:?}", target.declaration())
                    }
                };
                rows.push(format!(
                    "hir {owner} {table_name}[{position}]: {:?} -> {target}\n",
                    slot.declaration()
                ));
            }
        }
    }
    for record in replay.section.dispatch().records() {
        let owner = exact(record.owner());
        if !owner.contains("SharedDispatch") {
            continue;
        }
        for slot in record.interface_slots().into_iter().flatten() {
            let signature = slot.signature();
            let parameters = signature
                .exact()
                .parameters()
                .iter()
                .map(|id| exact(*id))
                .collect::<Vec<_>>();
            let receiver = signature.exact().receiver().into_option().map(exact);
            rows.push(format!(
                "mir {owner} interface-slot[{}]: {:?} {:?} ({receiver:?}; {parameters:?}) -> {} {:?}\n",
                slot.position().get(),
                slot.slot(),
                signature.exact().effect(),
                exact(signature.exact().result()),
                signature.gc_effect(),
            ));
        }
        let tables = std::iter::once(("vtable".into(), record.vtable())).chain(
            record.itables().iter().map(|table| {
                (
                    format!("itable {}", exact(table.interface())),
                    table.entries(),
                )
            }),
        );
        for (table, entries) in tables {
            for entry in entries {
                let signature = entry.signature();
                let parameters = signature
                    .exact()
                    .parameters()
                    .iter()
                    .map(|id| exact(*id))
                    .collect::<Vec<_>>();
                let receiver = signature.exact().receiver().into_option().map(exact);
                rows.push(format!(
                    "mir {owner} {table}[{}]: {} {:?} ({receiver:?}; {parameters:?}) -> {} {:?}\n",
                    entry.position().get(),
                    callable(replay, entry.implementation().target()),
                    entry.implementation(),
                    exact(signature.exact().result()),
                    signature.gc_effect()
                ));
            }
        }
    }
    rows.sort();
    let snapshot = crate::workspace_root().join(format!(
        "tests/fixtures/m23-core-layout-exports/{name}.dispatch.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
        std::fs::write(&snapshot, rows.concat()).unwrap();
    }
    assert_eq!(rows.concat(), std::fs::read_to_string(snapshot).unwrap());
}

fn callable(replay: &Replay<'_>, target: scoop_identity::CallableDefinitionOwner) -> String {
    let scoop_identity::CallableDefinitionOwner::Strong(target) = target else {
        return format!("{target:?}");
    };
    let identities = replay.source.metadata().identities;
    match target {
        StrongCallableDefinitionOwner::Function(id) => format!(
            "{:?}",
            identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap()
                .name()
        ),
        StrongCallableDefinitionOwner::PropertyAccessor(id) => {
            let accessor = identities
                .canonical_key::<_, PropertyAccessorKey>(id)
                .unwrap();
            let key = match accessor.owner() {
                PropertyOwner::Property(id) => identities
                    .canonical_key::<_, SourceDeclarationKey>(id)
                    .unwrap(),
                PropertyOwner::ExtensionProperty(id) => identities
                    .canonical_key::<_, SourceDeclarationKey>(id)
                    .unwrap(),
            };
            format!("{:?} {:?}", accessor.role(), key.name())
        }
        StrongCallableDefinitionOwner::GeneratedCallable(id) => format!(
            "{:?}",
            identities
                .canonical_key::<_, GeneratedCallableKey>(id)
                .unwrap()
        ),
        StrongCallableDefinitionOwner::Constructor(_) => {
            panic!("a constructor cannot implement a dispatch slot")
        }
    }
}
