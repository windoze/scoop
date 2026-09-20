use super::*;
use hir::InheritanceSlotSchemaSemanticAuthority as _;
use scoop_identity::{CallableTemplateOrigin, DispatchRole};

#[test]
fn dispatch_join_requires_selected_targets_to_own_the_same_source_slot() {
    with_sources(SOURCE, |fixture, sources, dispatch, core| {
        let foundation = fixture.bind().unwrap();
        let original = dispatch.bind(&foundation, &mut meter()).unwrap();
        let original_slots = original.bind_slot_sources(core, &mut meter()).unwrap();
        let (selection, replacement) = dispatch
            .selections
            .records()
            .iter()
            .find_map(|selection| {
                if original_slots
                    .dispatch_slot_key(selection.slot())
                    .unwrap()
                    .role()
                    != DispatchRole::VirtualMethod
                {
                    return None;
                }
                let hir::InheritanceSourceSlotSelectionV1::Concrete(
                    hir::InheritanceCallableDeclarationV1::Function(id),
                ) = selection.selection()
                else {
                    return None;
                };
                let original = sources
                    .members
                    .callables
                    .get(CallableTemplateOrigin::Function(id))
                    .unwrap();
                dispatch.callables.records().iter().find_map(|record| {
                    let hir::InheritanceCallableDeclarationV1::Function(other) =
                        record.declaration()
                    else {
                        return None;
                    };
                    let candidate = sources
                        .members
                        .callables
                        .get(CallableTemplateOrigin::Function(other))
                        .unwrap();
                    (other != id
                        && candidate.payload().owner() == original.payload().owner()
                        && !candidate
                            .payload()
                            .slot_relations()
                            .slots()
                            .contains(&selection.slot()))
                    .then_some((*selection, record.declaration()))
                })
            })
            .unwrap();
        let mut forged = dispatch.clone();
        forged.selections = hir::CanonicalInheritanceSourceSlotSelectionsV1::try_new(
            dispatch
                .selections
                .records()
                .iter()
                .map(|r| {
                    if r.owner() == selection.owner() && r.slot() == selection.slot() {
                        hir::InheritanceSourceSlotSelectionRecordV1::new(
                            r.owner(),
                            r.slot(),
                            hir::InheritanceSourceSlotSelectionV1::Concrete(replacement),
                        )
                    } else {
                        *r
                    }
                })
                .collect(),
            &mut meter(),
        )
        .unwrap();
        let bound = forged.bind(&foundation, &mut meter()).unwrap();
        let slots = bound.bind_slot_sources(core, &mut meter()).unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
            assert!(matches!(parameters.bind_dispatch_sources(&slots, &mut meter()),
                Err(Error::Callable { declaration, field: "selected slot relation" }) if declaration == replacement));
        });
    });
}
