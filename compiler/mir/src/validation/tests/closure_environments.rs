use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    GeneratedCallableKey, LexicalCallableParent, LexicalCallableRole, LocalValueKey,
    LocalValueSelector, PackagePath, PersistentFunctionId, SourceDeclarationKey,
    SourceDeclarationSite, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use super::*;

fn source_function(name: &str) -> PersistentFunctionId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn lambda_materialization() -> CallableMaterialization {
    let key = GeneratedCallableKey::Lexical {
        parent: LexicalCallableParent::function(source_function("closureValidationOwner")),
        role: LexicalCallableRole::LambdaBody,
        path: StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
            [],
        ),
    };
    let callable = CborIdentityRecord::from_key(key).unwrap().id();
    CallableMaterialization::new(
        CallableTemplateOwner::Generated(callable),
        CallableMaterializationContext::NoSubstitution,
    )
}

fn module_with_source_closure() -> Module {
    module_with_source_closure_fields(0).0
}

pub(super) fn module_with_source_closure_fields(field_count: u32) -> (Module, ClosureClassId) {
    module_with_source_closure_result(field_count, Type::Any)
}

pub(super) fn module_with_source_closure_result(
    field_count: u32,
    result: Type,
) -> (Module, ClosureClassId) {
    let (mut module, _) = module_with_variants(Vec::new());
    register_test_exact_type(&mut module, &Type::Unit);
    let callable = lambda_materialization();
    register_test_exact_type(&mut module, &Type::Any);
    let function_type = module.function_types.alloc(FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: result.clone(),
    });
    register_test_function_type(&mut module, function_type);
    let mut locals = Arena::new();
    let receiver = locals.alloc(Local {
        name: "$closure".to_string(),
        ty: Type::Function(function_type),
        mutable: false,
    });
    let invoke_function = module.functions.alloc(Function {
        gc_effect: GcEffect::Managed,
        name: "$closure.invoke".to_string(),
        params: vec![Param {
            name: "$closure".to_string(),
            ty: Type::Function(function_type),
            local: receiver,
        }],
        return_ty: result.clone(),
        body: Body::unreachable(locals),
    });
    module.top_level.push(invoke_function);
    let mut sources = module
        .meta
        .source_callable_materializations
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    sources.push(
        SourceCallableMaterialization::new(
            invoke_function,
            callable,
            scoop_identity::ExactCallableSignature::new(
                scoop_identity::Effect::Ordinary,
                None,
                Vec::new(),
                test_exact_type(&result).id(),
            ),
            None,
        )
        .unwrap(),
    );
    module.meta.source_callable_materializations =
        SourceCallableMaterializations::checked(sources).unwrap();
    let invoke = module
        .closure_invoke_functions
        .alloc(ClosureInvokeFunction {
            function: invoke_function,
        });
    let inputs = (0..field_count)
        .map(|declaration_index| {
            let value = CborIdentityRecord::from_key(LocalValueKey::new(
                CallableMaterialization::new(
                    CallableTemplateOwner::Function(source_function("capturedValueOwner")),
                    callable.context(),
                ),
                LocalValueSelector::Parameter { declaration_index },
            ))
            .unwrap();
            (ClosureFieldSource::Capture { declaration_index }, value)
        })
        .collect();
    let identity = ClosureEnvironmentIdentity::for_lambda(callable, inputs, None).unwrap();
    let captures = identity
        .fields()
        .iter()
        .map(|field| Field {
            name: format!("capture{:?}", field.source()),
            ty: Type::Unit,
        })
        .collect();
    let class = module.closure_classes.alloc(ClosureClass {
        name: "$Closure$validation".to_string(),
        function_type,
        invoke,
        captures,
        bridges: vec![FunctionBridge {
            target: function_type,
            function: invoke_function,
        }],
    });
    module.meta.closure_environments.push(
        ClosureEnvironment::checked(class, &module.closure_classes[class], identity).unwrap(),
    );
    install_generated_exact_types(&mut module);
    install_callable_signatures(&mut module);
    (module, class)
}

fn install_closure_allocation(
    module: &mut Module,
    class: ClosureClassId,
    physical_fields_in_semantic_order: impl IntoIterator<Item = u32>,
) -> BlockId {
    let allocation = Expr::new(
        Type::Function(module.closure_classes[class].function_type),
        ExprKind::ClosureAlloc {
            class,
            captures: physical_fields_in_semantic_order
                .into_iter()
                .map(|field| ClosureCaptureInit::new(field, Expr::unit()))
                .collect(),
        },
    );
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        statements: vec![Statement {
            kind: StatementKind::Expr(allocation),
            span: SourceSpan::new(0, 0).unwrap(),
        }],
        terminator: Terminator::Return { value: None },
        unwind: None,
    });
    module.functions[module
        .output
        .executable_entry()
        .expect("test module is executable")]
    .body = Body {
        locals: Arena::new(),
        blocks,
        entry,
        loop_header_polls: Vec::new(),
    };
    entry
}

#[test]
fn source_closure_environment_validates_with_its_invoke_materialization() {
    module_with_source_closure().validate().unwrap();
}

#[test]
fn every_non_adapter_closure_class_requires_an_environment_identity() {
    let mut module = module_with_source_closure();
    module.meta.closure_environments.clear();

    assert_eq!(
        module.validate().unwrap_err(),
        MirValidationError {
            location: MirValidationLocation::ClosureEnvironment { environment: 0 },
            kind: MirValidationErrorKind::InvalidClosureEnvironment {
                reason: "a source closure class has no persistent environment identity",
            },
        }
    );
}

#[test]
fn closure_allocation_retains_semantic_order_while_naming_physical_fields() {
    let (mut module, class) = module_with_source_closure_fields(2);
    install_closure_allocation(&mut module, class, [1, 0]);

    module.validate().unwrap();
    let dump = dump(&module);
    let second_field = dump.find("CaptureInit field=1").unwrap();
    let first_field = dump.find("CaptureInit field=0").unwrap();
    assert!(second_field < first_field);
}

#[test]
fn closure_allocation_rejects_a_duplicate_physical_field() {
    let (mut module, class) = module_with_source_closure_fields(2);
    let block = install_closure_allocation(&mut module, class, [0, 0]);

    assert_eq!(
        module.validate().unwrap_err(),
        MirValidationError {
            location: MirValidationLocation::FunctionBlock {
                function: module
                    .output
                    .executable_entry()
                    .expect("test module is executable"),
                block,
            },
            kind: MirValidationErrorKind::InvalidClosureExpression {
                reason: "allocation initializes a physical capture field more than once",
            },
        }
    );
}

#[test]
fn closures_require_one_fixed_dynamic_invoke() {
    let (mut module, class) = module_with_source_closure_fields(0);
    let target = module.closure_classes[class].bridges[0].target;
    let function = module.closure_classes[class].bridges[0].function;
    module.closure_classes[class].bridges.clear();
    assert!(matches!(
        module.validate().unwrap_err().kind,
        MirValidationErrorKind::InvalidFunctionBridge {
            reason: "a closure requires exactly one fixed dynamic invoke"
        }
    ));
    module.closure_classes[class].bridges = vec![
        FunctionBridge { target, function },
        FunctionBridge { target, function },
    ];
    assert!(matches!(
        module.validate().unwrap_err().kind,
        MirValidationErrorKind::InvalidFunctionBridge {
            reason: "a closure requires exactly one fixed dynamic invoke"
        }
    ));
}
