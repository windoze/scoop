use scoop_identity::{CborIdentityRecord, Effect, ExactCallableSignature, GeneratedNominalKey};

use super::closure_environments::module_with_source_closure_fields;
use super::*;

fn module_with_function_bridge() -> Module {
    let (mut module, class) = module_with_source_closure_fields(0);
    register_test_exact_type(&mut module, &Type::Unit);
    let target = module.function_types.alloc(FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: Type::Unit,
    });
    let function = module.functions.alloc(Function {
        gc_effect: GcEffect::Managed,
        name: "$function.bridge".to_string(),
        symbol: "scoop.$function.bridge".to_string(),
        params: Vec::new(),
        return_ty: Type::Unit,
        body: Body::unreachable(Arena::new()),
    });
    module.closure_classes[class]
        .bridges
        .push(FunctionBridge { target, function });
    let environment = module.meta.closure_environments[0]
        .identity()
        .generated_type_record();
    let identity = FunctionBridgeIdentity::new(
        environment,
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            test_exact_type(&Type::Unit).id(),
        ),
        None,
    )
    .unwrap();
    module.meta.function_bridges.push(
        FunctionBridgeMaterialization::checked(
            class,
            &module.closure_classes[class],
            target,
            function,
            identity,
        )
        .unwrap(),
    );
    install_generated_callables(&mut module);
    module
}

#[test]
fn generated_function_bridge_has_one_canonical_identity() {
    module_with_function_bridge().validate().unwrap();
}

#[test]
fn signature_changing_dispatch_entry_requires_bridge_metadata() {
    let mut module = module_with_function_bridge();
    module.meta.function_bridges.clear();

    assert_eq!(
        module.validate().unwrap_err(),
        MirValidationError {
            location: MirValidationLocation::FunctionBridge { bridge: 0 },
            kind: MirValidationErrorKind::InvalidFunctionBridge {
                reason: "a signature-changing dispatch entry has no persistent bridge identity",
            },
        }
    );
}

#[test]
fn one_physical_bridge_cannot_have_duplicate_identity_metadata() {
    let mut module = module_with_function_bridge();
    module
        .meta
        .function_bridges
        .push(module.meta.function_bridges[0].clone());

    assert_eq!(
        module.validate().unwrap_err(),
        MirValidationError {
            location: MirValidationLocation::FunctionBridge { bridge: 1 },
            kind: MirValidationErrorKind::InvalidFunctionBridge {
                reason: "the same closure and target have more than one generated bridge",
            },
        }
    );
}

#[test]
fn bridge_identity_must_name_the_exact_closure_environment() {
    let mut module = module_with_function_bridge();
    let bridge = module.meta.function_bridges.remove(0);
    let expected = module.meta.closure_environments[0].identity();
    let GeneratedNominalKey::ClosureEnvironment { callable, .. } =
        expected.generated_type_record().key()
    else {
        panic!("fixture uses a source closure environment")
    };
    let other_environment = CborIdentityRecord::from_key(GeneratedNominalKey::ClosureEnvironment {
        callable: *callable,
        role: scoop_identity::ClosureEnvironmentRole::AnonymousFunction,
    })
    .unwrap();
    let identity = FunctionBridgeIdentity::new(
        &other_environment,
        bridge.identity().signature_record().signature().clone(),
        None,
    )
    .unwrap();
    module.meta.function_bridges.push(
        FunctionBridgeMaterialization::checked(
            bridge.class(),
            &module.closure_classes[bridge.class()],
            bridge.target(),
            bridge.function(),
            identity,
        )
        .unwrap(),
    );

    assert_eq!(
        module.validate().unwrap_err(),
        MirValidationError {
            location: MirValidationLocation::FunctionBridge { bridge: 0 },
            kind: MirValidationErrorKind::InvalidFunctionBridge {
                reason: "the generated bridge identifies a different closure environment",
            },
        }
    );
}
