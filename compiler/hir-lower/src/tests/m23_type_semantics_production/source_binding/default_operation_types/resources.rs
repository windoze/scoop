use super::*;

#[test]
fn operation_application_copy_preserves_all_signature_forms_and_accounts_for_output() {
    use scoop_identity::{CallingConvention, Effect};
    with_protocols(COMBINED, |_, protocols| {
        let path = WirePath::root();
        let unit = protocols.default_operation_type(Role::Unit).unwrap();
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

        let result = protocols
            .classify_default_operation_application(&application, &path)
            .unwrap()
            .unwrap();
        assert_eq!(classified(&result).1, &source);

        protocols
            .classify_default_operation_application(&application, &path)
            .unwrap();
    });
}
