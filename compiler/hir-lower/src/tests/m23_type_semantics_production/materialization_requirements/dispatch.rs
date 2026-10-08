use super::*;

#[test]
fn selected_nonvirtual_interface_implementations_require_their_complete_callable() {
    source_dispatch::with_hir_source(COMBINED, |output, _| {
        let public = public_interface(output);
        let identities = source_inventory::identity_closure(output);
        let mut found = BTreeSet::new();
        public
            .nominal_interfaces()
            .visit_materialization_requirements::<Error>(
                public.callable_interfaces(),
                |requirement| {
                    if let Requirement::Slot { owner, callable } = requirement {
                        let owner_name = name(owner, &identities);
                        if matches!(owner_name.as_str(), "Value" | "FinalReader") {
                            assert!(callable.slot_relations().is_empty());
                            let source = public
                                .nominal_interfaces()
                                .declaration(hir::SourceNominalId::Concrete(owner))
                                .unwrap();
                            assert!(
                                source
                                    .declaration_details()
                                    .dispatch_selections()
                                    .records()
                                    .iter()
                                    .any(|selection| selection.callable_target()
                                        == Some(callable.declaration()))
                            );
                            assert!(found.insert(owner_name));
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(
            found,
            BTreeSet::from(["FinalReader".to_owned(), "Value".to_owned()])
        );
    });
}
