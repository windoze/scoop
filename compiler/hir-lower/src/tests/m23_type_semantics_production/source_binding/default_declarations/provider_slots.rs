use super::*;
use hir::{DefaultSourceProviderDispatchV1 as Dispatch, OptionalDefaultSourceSlotDomainV1 as Slot};
mod corruption;
mod identities;
pub(super) const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/provider-slot-domains.scoop"
));
pub(super) const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/provider-slot-domain-combinations.scoop"
));

#[test]
fn default_provider_slots_replay_original_declaration_roles_and_reference_free_bodies() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let export = &output.output().export;
        let helper = export
            .functions
            .iter()
            .find(|(_, f)| f.name == "ProviderSlotInterface.helper")
            .unwrap()
            .1;
        assert!(helper.access.slot.is_none());
        assert!(matches!(
            helper.method.unwrap().dispatch,
            hir::MethodDispatch::Direct
        ));
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let bound = parameters
                .bind_default_declarations(&table, &[], &mut meter())
                .unwrap();
            let mut snapshot = String::new();
            for (name, position) in [
                ("ProviderSlotInterface.abstractValue", 0),
                ("ProviderSlotInterface.bodyValue", 0),
                ("ProviderSlotInterface.helper", 0),
                ("ProviderSlotBase.virtualValue", 0),
                ("ProviderSlotBase.otherValue", 0),
                ("ProviderSlotBase.direct", 0),
                ("ProviderSlotBase.generic", 1),
            ] {
                let contract = bound
                    .declaration(key(output, name, position), &mut meter())
                    .unwrap();
                match contract.provider_dispatch() {
                    Dispatch::Direct => {
                        assert!(contract.provider_slot_domain().is_none());
                        snapshot.push_str(&format!("{name}: Direct\n"));
                    }
                    Dispatch::RootSlot(id) => {
                        assert_eq!(contract.provider().slot_relations().slots(), &[id]);
                        let domain = contract.provider_slot_domain().unwrap();
                        assert_eq!(domain, contract.direct_call_domain());
                        snapshot.push_str(&format!(
                            "{name}: RootSlot; generic {}\n",
                            domain.generic_subclasses().values().len()
                        ));
                    }
                }
                for occurrence in contract.references().occurrences() {
                    match occurrence.source().witness().slot_call_domain() {
                        Slot::Absent => assert_eq!(contract.provider_dispatch(), Dispatch::Direct),
                        Slot::Present(domain) => {
                            assert_eq!(Some(domain), contract.provider_slot_domain())
                        }
                    }
                }
            }
            let mut constructors = 0;
            let mut variants = 0;
            let mut empty = 0;
            for contract in bound.declarations() {
                match contract.key().owner() {
                    CallableTemplateOrigin::Constructor(_) => constructors += 1,
                    CallableTemplateOrigin::VariantConstructor(_) => variants += 1,
                    _ => continue,
                }
                assert_eq!(contract.provider_dispatch(), Dispatch::Direct);
                if contract.references().occurrences().is_empty() {
                    empty += 1;
                }
            }
            assert!(constructors >= 2 && variants == 1 && empty == 1);
            snapshot.push_str(
                "constructor and variant defaults: Direct\nreference-free constructor: Direct\n",
            );
            assert_eq!(
                snapshot,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/m23-type-source-defaults/provider-slot-domains.snap"
                ))
            );
        });
    });
}
#[test]
fn default_provider_slots_keep_original_roots_through_override_and_generic_qualifiers() {
    with_sources(COMBINATIONS, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let bound = parameters
                .bind_default_declarations(&table, &[], &mut meter())
                .unwrap();
            for (base, child) in [
                (
                    "ProviderSlotParent.inherited",
                    "ProviderSlotChild.inherited",
                ),
                ("ProviderSlotRoot.chosen", "ProviderSlotImpl.chosen"),
            ] {
                let base = bound
                    .declaration(key(output, base, 0), &mut meter())
                    .unwrap();
                let child = bound
                    .declaration(key(output, child, 0), &mut meter())
                    .unwrap();
                assert_ne!(base.key().owner(), child.key().owner());
                assert!(matches!(base.provider_dispatch(), Dispatch::RootSlot(_)));
                assert_eq!(base.provider_dispatch(), child.provider_dispatch());
                assert_eq!(base.provider_slot_domain(), child.provider_slot_domain());
            }
            for (name, position) in [
                ("ProviderSlotGeneric.own", 1),
                ("ProviderSlotGeneric.Static.nested", 0),
            ] {
                let contract = bound
                    .declaration(key(output, name, position), &mut meter())
                    .unwrap();
                assert!(matches!(
                    contract.provider_dispatch(),
                    Dispatch::RootSlot(_)
                ));
                assert_eq!(
                    contract
                        .provider_slot_domain()
                        .unwrap()
                        .generic_subclasses()
                        .values()
                        .len(),
                    1
                );
                if name.contains(".Static.") {
                    assert_eq!(contract.provider_binders().binder_arity(), 0);
                }
            }
        });
    });
}
