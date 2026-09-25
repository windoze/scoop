use super::*;

#[test]
fn nominal_member_binding_does_not_lose_a_setter_when_both_source_tables_omit_it() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let record = property(sources, output, "localValue");
        let hir::ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } =
            runtime(&record).mutability()
        else {
            panic!("setter");
        };
        let mut forged = sources.clone();
        replace_property(
            &mut forged,
            with_mutability(&record, hir::ProtectedPropertyMutabilityV1::ReadOnly),
        );
        forged.callables = Callables::try_new(
            forged
                .callables
                .records()
                .iter()
                .filter(|r| r.declaration() != CallableTemplateOrigin::Accessor(*setter))
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            check(fixture, &forged, core),
            Err(Error::Inventory("property accessor keys"))
        ));
    });
}

#[test]
fn nominal_property_and_setter_must_agree_on_access_and_replay_effective_domains() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let record = property(sources, output, "localValue");
        let hir::ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } = runtime(&record).mutability()
        else {
            panic!("setter");
        };
        let callable = sources
            .callables
            .get(CallableTemplateOrigin::Accessor(*setter))
            .unwrap();
        // Internal is within the property's domain, but disagrees with the
        // still-private accessor source. Public is wider even after both agree.
        for (visibility, change_callable) in [
            (hir::DeclaredVisibilityV1::Internal, false),
            (hir::DeclaredVisibilityV1::Public, true),
        ] {
            let mut forged = sources.clone();
            replace_property(
                &mut forged,
                with_mutability(
                    &record,
                    hir::ProtectedPropertyMutabilityV1::ReadWrite {
                        setter: *setter,
                        setter_access: access(setter_access, visibility),
                    },
                ),
            );
            if change_callable {
                replace_callable(
                    &mut forged,
                    hir::NominalSupportCallableInterfaceV1::try_new(
                        callable.declaration(),
                        access(callable.declaration_access(), visibility),
                        callable.payload().clone(),
                    )
                    .unwrap(),
                );
            }
            assert!(check(fixture, &forged, core).is_err());
        }
    });
}

#[test]
fn nominal_property_slot_relations_equal_the_complete_accessor_union() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let slotted = property(sources, output, "count");
        let direct = property(sources, output, "localValue");
        assert!(!runtime(&slotted).slot_relations().is_empty());
        for (record, slots) in [
            (&slotted, runtime(&direct).slot_relations()),
            (&direct, runtime(&slotted).slot_relations()),
        ] {
            let p = runtime(record);
            let changed = hir::NominalSupportPropertyInterfaceV1::try_new(
                record.declaration(),
                record.declaration_access().clone(),
                hir::NominalSupportPropertyPayloadV1::Runtime {
                    interface: hir::NominalSourcePropertyPayloadV1::try_new(
                        p.owner(),
                        p.value_type().clone(),
                        p.getter(),
                        p.mutability().clone(),
                        p.representation(),
                        slots.clone(),
                    )
                    .unwrap(),
                },
            )
            .unwrap();
            let mut forged = sources.clone();
            replace_property(&mut forged, changed);
            assert!(matches!(
                check(fixture, &forged, core),
                Err(Error::Accessor(
                    hir::ProtectedPropertyAccessorClosureError::Slot
                )) | Err(Error::Inventory("property accessor slots"))
            ));
        }
    });
}
