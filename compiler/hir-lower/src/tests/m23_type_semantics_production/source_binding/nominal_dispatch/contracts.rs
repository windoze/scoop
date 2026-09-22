use super::*;
use hir::InheritanceSlotContractSemanticAuthority as _;
use scoop_identity::ExactCallableSignature;

#[test]
fn dispatch_join_rejects_individually_bound_but_conflicting_effects_and_results() {
    with_sources(SOURCE, |fixture, sources, dispatch, core| {
        let foundation = fixture.bind().unwrap();
        let bound = dispatch.bind(&foundation, &mut meter()).unwrap();
        let slots = bound.bind_slot_sources(&mut meter()).unwrap();
        let unit = slots.unit_exact_type().unwrap();
        let index = dispatch
            .callables
            .records()
            .iter()
            .position(|r| {
                matches!(
                    r.declaration(),
                    hir::InheritanceCallableDeclarationV1::Function(_)
                )
            })
            .unwrap();
        let record = &dispatch.callables.records()[index];
        assert_ne!(record.signature().exact_signature().result(), unit);
        for field in ["effects", "result", "arity", "receiver", "modality"] {
            let exact = record.signature().exact_signature();
            let e = record.signature().effects();
            let effects = if field == "effects" {
                hir::CallableSourceEffectsV1::try_new(
                    e.execution(),
                    hir::CallableSafetyV1::Unsafe,
                    e.gc_effect(),
                    e.implementation(),
                    e.operator_role(),
                    e.infix(),
                )
                .unwrap()
            } else {
                e
            };
            let parameters = if field == "arity" {
                vec![]
            } else {
                exact.parameters().to_vec()
            };
            let signature = ExactCallableSignature::new(
                exact.effect(),
                if field == "receiver" {
                    Some(unit)
                } else {
                    exact.receiver().into_option()
                },
                parameters,
                if field == "result" {
                    unit
                } else {
                    exact.result()
                },
            );
            let changed = hir::InheritanceSourceCallableV1::new(
                record.declaration(),
                hir::InheritanceCallableSignatureV1::try_new(signature, effects).unwrap(),
                if field == "modality" {
                    if record.modality() == hir::CallableModalityV1::Final {
                        hir::CallableModalityV1::Open
                    } else {
                        hir::CallableModalityV1::Final
                    }
                } else {
                    record.modality()
                },
                record.declaration_access().clone(),
            );
            let mut records = dispatch.callables.records().to_vec();
            records[index] = changed;
            let mut forged = dispatch.clone();
            forged.callables =
                hir::CanonicalInheritanceSourceCallablesV1::try_new(records, &mut meter()).unwrap();
            let bound = forged.bind(&foundation, &mut meter()).unwrap();
            let slots = bound.bind_slot_sources(&mut meter()).unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
                let error = parameters.bind_dispatch_sources(&slots, &mut meter()).unwrap_err();
                match field {
                    "effects" => assert!(matches!(error, Error::Source(e) if matches!(*e, hir::NominalNestedBindingError::Contract { field: "effects", .. }))),
                    "result" | "receiver" => assert!(matches!(error, Error::Signature { .. })),
                    "modality" => assert!(matches!(error, Error::Source(e) if matches!(*e, hir::NominalNestedBindingError::Contract { field: "modality", .. }))),
                    "arity" => assert!(matches!(error, Error::Callable { field: "parameters", .. })),
                    _ => unreachable!(),
                }
            });
        }
    });
}
