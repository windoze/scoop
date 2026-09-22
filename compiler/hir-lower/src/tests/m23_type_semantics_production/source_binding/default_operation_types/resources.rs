use super::*;
use hir::DefaultOperationProtocolTypeError as Error;

#[test]
fn operation_type_queries_charge_even_unclassified_values_and_share_the_caller_budget() {
    with_protocols(SOURCE, |_, protocols| {
        let path = WirePath::root();
        let unit = protocols
            .default_operation_type(Role::Unit, &mut meter(), &path)
            .unwrap();
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                protocols
                    .default_operation_type(Role::Unit, &mut BudgetMeter::new(limits), &path)
                    .is_err()
            );
            assert!(matches!(
                protocols.classify_default_operation_application(
                    &unit,
                    &mut BudgetMeter::new(limits),
                    &path
                ),
                Err(Error::Resource(_))
            ));
        }
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: 1,
            ..DecodeLimits::default()
        });
        protocols
            .default_operation_type(Role::Unit, &mut shared, &path)
            .unwrap();
        assert!(matches!(
            protocols.classify_default_operation_application(&unit, &mut shared, &path),
            Err(Error::Resource(_))
        ));
    });
}

#[test]
fn operation_application_copy_preserves_all_signature_forms_and_accounts_for_output() {
    use scoop_identity::{CallingConvention, Effect};
    with_protocols(COMBINED, |_, protocols| {
        let path = WirePath::root();
        let unit = protocols
            .default_operation_type(Role::Unit, &mut meter(), &path)
            .unwrap();
        let binder = Type::Binder { depth: 3, index: 7 };
        let function = Type::Function {
            effect: Effect::Suspend,
            parameters: vec![binder.clone()],
            result: Box::new(unit.clone()),
        };
        let array = protocols.fundamental_types().array().persistent();
        let source = Type::Tuple(
            NonEmptyVec::new(vec![
                unit.clone(),
                binder.clone(),
                function,
                Type::NativeFunctionPointer {
                    calling_convention: CallingConvention::C,
                    parameters: vec![unit.clone()],
                    result: Box::new(binder.clone()),
                },
                Type::RawPointer(Box::new(binder.clone())),
                Type::NominalApplication {
                    origin: array,
                    arguments: NonEmptyVec::from_first(binder, []),
                },
            ])
            .unwrap(),
        );
        let application = Type::NominalApplication {
            origin: array,
            arguments: NonEmptyVec::from_first(source.clone(), []),
        };
        let mut measured = meter();
        let result = protocols
            .classify_default_operation_application(&application, &mut measured, &path)
            .unwrap()
            .unwrap();
        assert_eq!(classified(&result).1, &source);
        for limits in [
            DecodeLimits {
                validation_work_units: measured.usage().validation_work_units - 1,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                owned_bytes: measured.usage().owned_bytes - 1,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: measured.usage().decoded_nodes - 1,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 1,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                protocols.classify_default_operation_application(
                    &application,
                    &mut BudgetMeter::new(limits),
                    &path
                ),
                Err(Error::Resource(_))
            ));
        }
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units * 2 - 1,
            ..DecodeLimits::default()
        });
        protocols
            .classify_default_operation_application(&application, &mut shared, &path)
            .unwrap();
        assert!(matches!(
            protocols.classify_default_operation_application(&application, &mut shared, &path),
            Err(Error::Resource(_))
        ));
    });
}
