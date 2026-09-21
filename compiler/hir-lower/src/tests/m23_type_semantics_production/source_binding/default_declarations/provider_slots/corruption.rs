use super::*;
use hir::{DefaultSourceAccessDomainV1 as Domain, ExportDefaultReferenceKindV1 as Kind};

#[test]
fn default_provider_slots_reject_forged_presence_and_domains_in_every_reference_kind() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/reference-closure.scoop"
    ));
    for has_slot in [false, true] {
        let source = if has_slot {
            source
                .replace(
                    "public class ReferenceClosureHost",
                    "public open class ReferenceClosureHost",
                )
                .replace("internal fun all", "protected open fun all")
        } else {
            source.to_owned()
        };
        with_sources(&source, |output, fixture, sources, core| {
            let table = templates(output);
            let original = table
                .get(key(output, "ReferenceClosureHost.all", 0))
                .unwrap();
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
                parameters.bind_default_declarations(&table, &[], &mut meter()).unwrap();
                let replacements = if has_slot {
                    vec![Slot::Absent, Slot::Present(Domain::empty()), Slot::Present(Domain::universal())]
                } else { vec![Slot::Present(Domain::universal())] };
                for kind in [Kind::Callable, Kind::Constructor, Kind::Type, Kind::Global, Kind::Singleton, Kind::Field] {
                    for slot in &replacements {
                        let changed = super::super::reference_witness::change(original, kind, |w| {
                            hir::DefaultSourceAccessWitnessV1::try_new(w.owner(), w.direct_call_domain().clone(), slot.clone(), w.target_domain().clone()).unwrap()
                        }, output);
                        let changed = replace(&table, changed);
                        let error = parameters.bind_default_declarations(&changed, &[], &mut meter()).unwrap_err();
                        let Error::Record { key, error } = error else { panic!("record error") };
                        assert_eq!(key, original.key());
                        assert!(matches!(*error, Error::ProviderSlot(error) if matches!(*error, hir::DefaultSourceProviderSlotError::Witness { kind: actual, index: 0 } if actual == kind)));
                    }
                }
            });
        });
    }
}
