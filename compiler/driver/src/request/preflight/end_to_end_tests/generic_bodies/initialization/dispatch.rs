use scoop_identity::{CallableDefinitionOwner, LinkageClass};
use scoop_lir::{ExactDispatchRoleV1, StrongTypeDispatchCallableRefV2};
use scoop_mir::{MirTypeOriginV1, MirTypeRepresentationV1};

pub(super) fn check(
    sections: &scoop_slib::PhysicalImportsReplayedCrossConeLayoutSections,
    interface_count: usize,
) {
    let mir = sections.mir_type_bridge().exports();
    let lir = sections.lir_exports();
    let mut classes = 0;
    for ty in mir.types().records().iter().filter(|ty| {
        matches!(ty.origin(), MirTypeOriginV1::NominalApplication(_))
            && matches!(ty.representation(), MirTypeRepresentationV1::Class { .. })
    }) {
        classes += 1;
        let schema = mir.dispatch().get(ty.exact()).unwrap();
        assert_eq!(schema.vtable().len(), 2);
        assert_eq!(schema.itables().len(), interface_count);
        let tables: Vec<_> = lir
            .dispatch()
            .records()
            .iter()
            .filter(|table| table.owner_exact() == ty.exact())
            .collect();
        assert_eq!(tables.len(), 1 + interface_count);
        for table in tables {
            assert_eq!(table.definition().symbol().linkage(), LinkageClass::OdrWeak);
            let expected = match table.role() {
                ExactDispatchRoleV1::Vtable => schema.vtable(),
                ExactDispatchRoleV1::Itable { interface_exact } => {
                    schema.interface_table(interface_exact).unwrap().entries()
                }
            };
            assert_eq!(table.entries().len(), expected.len());
            for (actual, expected) in table.entries().iter().zip(expected) {
                let target = actual.implementation().target();
                assert!(matches!(target, CallableDefinitionOwner::Odr(_)));
                assert_eq!(target, expected.implementation().target());
                assert_eq!(actual.slot(), expected.slot());
                assert_eq!(
                    actual.slot_signature().exact(),
                    expected.signature().exact()
                );
                assert_eq!(
                    actual.slot_signature().gc_effect(),
                    match expected.signature().gc_effect() {
                        scoop_mir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
                        scoop_mir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
                    }
                );
                let binding = mir.callables().get(target).unwrap();
                let abi = lir.callables().get(target).unwrap();
                assert_eq!(
                    abi.canonical_signature().signature(),
                    binding.lowered_signature().exact()
                );
                assert_eq!(abi.definition().symbol().linkage(), LinkageClass::OdrWeak);
                assert_eq!(abi.physical_definition().provider(), lir.provider());
                assert!(
                    matches!(actual.abi(), StrongTypeDispatchCallableRefV2::Local(body) if body == abi.definition().semantic_id())
                );
            }
        }
    }
    assert_eq!(classes, 2);
}
