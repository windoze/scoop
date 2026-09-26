use super::*;

#[test]
fn nested_source_replay_rejects_changed_nominal_modality_and_omitted_child_closure() {
    with_candidates(SOURCE, |fixture, sources, candidates, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let mut authority = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            for record in candidates.records.iter().filter(|r| {
                !r.payload()
                    .source_interface()
                    .children()
                    .values()
                    .is_empty()
            }) {
                let s = record.payload().source_interface();
                for omit_child in [false, true] {
                    let child = s.children().values()[0];
                    let children = if omit_child {
                        hir::CanonicalNestedNominalRefsV1::try_new(
                            s.children()
                                .values()
                                .iter()
                                .copied()
                                .filter(|id| *id != child)
                                .collect(),
                        )
                        .unwrap()
                    } else {
                        s.children().clone()
                    };
                    let entries = s
                        .source_support()
                        .records()
                        .iter()
                        .filter(|r| {
                            !(omit_child
                                && matches!(r, Entry::NestedNominal(r) if r.declaration() == child))
                        })
                        .cloned()
                        .collect();
                    let interface = hir::ProtectedNestedSourceInterfaceV1::try_new(
                        s.kind(),
                        if omit_child {
                            s.modality()
                        } else {
                            hir::NominalInheritanceModalityV1::Open
                        },
                        s.type_parameters().clone(),
                        s.supertypes().clone(),
                        s.constructors().clone(),
                        s.members().clone(),
                        children,
                        s.source_shape().clone(),
                        hir::CanonicalNestedSourceSupportV1::try_new(entries).unwrap(),
                    )
                    .unwrap();
                    let forged = rebuild(record, interface);
                    let Error::Contract { field, .. } = authority
                        .validate_nested_source(&forged, &candidates.protocols)
                        .unwrap_err()
                    else {
                        panic!("independent nominal contract");
                    };
                    assert_eq!(field, if omit_child { "children" } else { "modality" });
                }
            }
        });
    });
}
