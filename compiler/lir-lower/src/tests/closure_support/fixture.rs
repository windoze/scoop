use super::*;

pub(super) struct Sample {
    pub module: mir::Module,
    pub class: mir::ClosureClassId,
    pub invoke: mir::FunctionId,
    pub bridge: mir::FunctionId,
    pub caller: mir::FunctionId,
    pub function_type: mir::FunctionTypeId,
}

pub(super) fn sample() -> Sample {
    let mut b = Builder::new();
    let function_type = b.function_types.alloc(mir::FunctionType {
        receiver: None,
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Boolean,
    });
    let bridge_type = b.function_types.alloc(mir::FunctionType {
        receiver: None,
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Any,
    });
    let ty = mir::Type::Function(function_type);
    let mut define = |name, result| {
        let mut locals = Arena::new();
        let environment = locals.alloc(local("environment", ty.clone()));
        b.user_fn_full(
            name,
            vec![param("environment", ty.clone(), environment)],
            result,
            locals,
            Vec::new(),
        )
    };
    let invoke = define("invoke", mir::Type::Boolean);
    let bridge = define("bridge", mir::Type::Any);
    let caller = define("caller", mir::Type::Unit);
    let boxed = b.class(
        "box<Boolean>",
        None,
        &[("value", mir::Type::Boolean)],
        Vec::new(),
        Vec::new(),
    );
    let main = b.main(Arena::new(), Vec::new());
    let mut module = b.finish(main);
    register_boxed_source_nominal(
        &mut module,
        mir::Type::Boolean,
        boxed,
        "TestBoolean",
        SourceNominalKind::Struct,
    );
    register_test_source_exact_type(&mut module, mir::Type::Function(bridge_type));
    let bridge_signature = module
        .meta
        .source_callable_materializations
        .get(bridge)
        .unwrap()
        .signature_record()
        .signature()
        .clone();
    let source = module
        .meta
        .source_callable_materializations
        .get(caller)
        .unwrap();
    let CallableTemplateOwner::Function(parent) = source.materialization().template() else {
        unreachable!();
    };
    let callable = CborIdentityRecord::from_key(GeneratedCallableKey::Lexical {
        parent: LexicalCallableParent::function(parent),
        role: LexicalCallableRole::LambdaBody,
        path: StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
            [],
        ),
    })
    .unwrap();
    let materialization = CallableMaterialization::new(
        CallableTemplateOwner::Generated(callable.id()),
        CallableMaterializationContext::NoSubstitution,
    );
    let environment =
        mir::ClosureEnvironmentIdentity::for_lambda(materialization, Vec::new(), None).unwrap();
    let signature = module
        .meta
        .source_callable_materializations
        .get(invoke)
        .unwrap()
        .signature_record()
        .signature()
        .clone();
    let bridge_identity = mir::FunctionBridgeIdentity::new(
        environment.generated_type_record(),
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            exact_type_record(&module, &mir::Type::Any).id(),
        ),
        None,
    )
    .unwrap();
    let invoke_id = module
        .closure_invoke_functions
        .alloc(mir::ClosureInvokeFunction { function: invoke });
    let class = module.closure_classes.alloc(mir::ClosureClass {
        name: "testClosure".to_string(),
        function_type,
        invoke: invoke_id,
        captures: Vec::new(),
        bridges: vec![mir::FunctionBridge {
            target: bridge_type,
            function: bridge,
        }],
    });
    module.meta.function_bridges.push(
        mir::FunctionBridgeMaterialization::checked(
            class,
            &module.closure_classes[class],
            bridge_type,
            bridge,
            bridge_identity.clone(),
        )
        .unwrap(),
    );
    let mut types = module
        .meta
        .generated_exact_types
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    types.push(
        mir::GeneratedExactTypeIdentity::new(
            mir::GeneratedExactTypeLocation::Closure(class),
            environment.generated_type_record(),
            None,
        )
        .unwrap(),
    );
    module.meta.generated_exact_types = mir::GeneratedExactTypeIdentities::checked(types).unwrap();
    module.meta.closure_environments.push(
        mir::ClosureEnvironment::checked(class, &module.closure_classes[class], environment)
            .unwrap(),
    );
    let sources = module
        .meta
        .source_callable_materializations
        .iter()
        .filter(|source| source.function() != bridge)
        .map(|source| {
            if source.function() == invoke {
                mir::SourceCallableMaterialization::new(
                    invoke,
                    materialization,
                    signature.clone(),
                    None,
                )
                .unwrap()
            } else {
                source.clone()
            }
        })
        .collect();
    module.meta.source_callable_materializations =
        mir::SourceCallableMaterializations::checked(sources).unwrap();
    module.meta.generated_callables =
        mir::MirGeneratedCallableIdentities::checked(vec![mir::MirGeneratedCallableIdentity::new(
            bridge,
            bridge_identity.callable_record(),
            bridge_identity.signature_record().subject(),
        )])
        .unwrap();
    module.meta.callable_signatures = mir::MirCallableSignatures::checked(
        module
            .meta
            .source_callable_materializations
            .iter()
            .map(|source| source.signature_record().clone())
            .chain([mir::CallableSignatureRecord::new(
                bridge_identity.signature_record().subject(),
                bridge_signature,
            )])
            .collect(),
    )
    .unwrap();
    let mut sample = Sample {
        module,
        class,
        invoke,
        bridge,
        caller,
        function_type,
    };
    // A private cycle alone cannot make the support reachable.
    sample.call(invoke, bridge);
    sample.call(bridge, invoke);
    let body = &mut sample.module.functions[invoke].body;
    body.blocks[body.entry].terminator = mir::Terminator::Return {
        value: mir::Expr::bool(true),
    };
    sample
}

impl Sample {
    pub fn call(&mut self, caller: mir::FunctionId, target: mir::FunctionId) {
        let result = self.module.functions[target].return_ty.clone();
        let function = &mut self.module.functions[caller];
        let destination = function
            .body
            .locals
            .alloc(local("callResult", result.clone()));
        function.body.blocks[function.body.entry]
            .statements
            .push(call_value(
                destination,
                mir::Call {
                    target: mir::CallTarget::Direct(mir::Callee::User(target)),
                    args: vec![local_expr(
                        function.params[0].local,
                        function.params[0].ty.clone(),
                    )],
                },
            ));
        if caller == self.bridge {
            function.body.blocks[function.body.entry].terminator = mir::Terminator::Return {
                value: expr(
                    mir::Type::Any,
                    mir::ExprKind::Box(Box::new(local_expr(destination, result))),
                ),
            };
        }
    }

    pub fn allocate(&mut self) {
        let main = self.module.output.executable_entry().unwrap();
        let body = &mut self.module.functions[main].body;
        let ty = mir::Type::Function(self.function_type);
        let value = body.locals.alloc(local("value", ty.clone()));
        body.blocks[body.entry].statements.push(val_decl(
            value,
            expr(
                ty,
                mir::ExprKind::ClosureAlloc {
                    class: self.class,
                    captures: Vec::new(),
                },
            ),
        ));
    }
}
