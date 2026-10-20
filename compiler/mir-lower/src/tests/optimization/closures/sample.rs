use super::*;
use crate::tests::function_adapters::function_type;

pub(super) fn sample(box_capture: bool, capture_count: usize) -> mir::Module {
    let mut h = Harness::new();
    let (int, any) = (h.int, h.any());
    let (signature, callable) = function_type(&mut h, Vec::new(), int);
    let mut locals = Arena::new();
    let action = locals.alloc(local("action", callable));
    let helper = h.user_fn_full(
        "invoke",
        Vec::new(),
        vec![param("action", callable, action)],
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: expr(
                    hir::ExprKind::CallableCall {
                        callee: Box::new(local_ref(action, callable)),
                        function_type: signature,
                        args: Vec::new(),
                    },
                    int,
                ),
            })],
        },
    );
    let mut locals = Arena::new();
    let first = locals.alloc(local("first", int));
    let second = locals.alloc(local("second", int));
    let mut statements = Vec::new();
    let (captured, capture_ty) = if box_capture {
        let boxed = locals.alloc(local("boxed", any));
        statements.push(val_decl(
            boxed,
            expr(hir::ExprKind::Box(Box::new(local_ref(first, int))), any),
        ));
        (boxed, any)
    } else {
        (first, int)
    };
    let captures = [(captured, capture_ty), (second, int)]
        .into_iter()
        .take(capture_count)
        .map(|(value, ty)| hir::Capture {
            binding: locals[value].binding,
            name: locals[value].name.clone(),
            ty,
            first_use_span: SPAN,
            source: local_ref(value, ty),
        })
        .collect::<Vec<_>>();
    let returned = if capture_count == 0 {
        int_lit(&h, 7)
    } else {
        let first = expr(hir::ExprKind::Capture(captures[0].binding), capture_ty);
        let first = if box_capture {
            expr(hir::ExprKind::Unbox(Box::new(first)), int)
        } else {
            first
        };
        if capture_count == 1 {
            first
        } else {
            integer_binary(
                &mut h,
                hir::IntegerKind::SIGNED_32,
                hir::IntegerBinaryOperator::Sub,
                first,
                expr(hir::ExprKind::Capture(captures[1].binding), int),
            )
        }
    };
    let worker = h.user_fn_full(
        "scope",
        Vec::new(),
        vec![param("first", int, first), param("second", int, second)],
        int,
        hir::Body { locals, statements },
    );
    let mut lambda_locals = Arena::new();
    let environment = lambda_locals.alloc(local("$closure", callable));
    let lambda_body = h.user_fn_full(
        "lambdaBody",
        Vec::new(),
        vec![param("$closure", callable, environment)],
        int,
        hir::Body {
            locals: lambda_locals.clone(),
            statements: Vec::new(),
        },
    );
    let main = empty_main(&mut h);
    let executable = h.finish(main);
    let entry = executable.entry();
    let mut source = executable.into_module();
    source.top_level.retain(|&function| function != lambda_body);
    let path = scoop_identity::StructuralDefinitionPath::from_first(
        scoop_identity::StructuralPathSegment::new(
            scoop_identity::StructuralDefinitionSiteRole::Lambda,
            0,
        ),
        [],
    );
    let lambda = source.lambdas.alloc(hir::Lambda {
        definition: hir::LexicalFunctionDefinition::Source {
            function: lambda_body,
            root: hir::LexicalDefinitionRoot::Function(worker),
        },
        definition_path: path.clone(),
        function_type: signature,
        owner_type_param_count: 0,
        body_type_arguments: hir::CallableBodyTypeArguments::Lexical,
        captures,
        span: SPAN,
    });
    source.functions[lambda_body].kind = hir::FunctionKind::User(hir::Body {
        locals: lambda_locals,
        statements: vec![stmt(hir::StatementKind::Return { value: returned })],
    });
    let hir::FunctionKind::User(body) = &mut source.functions[worker].kind else {
        panic!("scope body");
    };
    let closure = body.locals.alloc(local("closure", callable));
    body.statements.push(val_decl(
        closure,
        expr(hir::ExprKind::Lambda(lambda), callable),
    ));
    body.statements.push(stmt(hir::StatementKind::Return {
        value: call_typed(helper, vec![local_ref(closure, callable)], int),
    }));
    set_lambda_identity(&mut source, worker, lambda_body, path);
    lower(&executable_output(source, entry))
}

fn set_lambda_identity(
    source: &mut hir::Module,
    worker: hir::FunctionId,
    lambda: hir::FunctionId,
    path: scoop_identity::StructuralDefinitionPath,
) {
    let parent = source.function_identities[worker]
        .source_identity()
        .unwrap()
        .lexical_parent();
    let identity = hir::HirFunctionIdentity::lexical_generated(
        parent,
        scoop_identity::LexicalCallableRole::LambdaBody,
        path,
    )
    .unwrap();
    let identities = source
        .function_identities
        .iter()
        .map(|(id, old)| {
            if id == lambda {
                identity.clone()
            } else {
                old.clone()
            }
        })
        .collect();
    source.function_identities = hir::HirFunctionIdentities::checked(
        hir::HirFunctionIdentityInputs {
            functions: &source.functions,
            lambdas: &source.lambdas,
            anonymous_functions: &source.anonymous_functions,
            local_functions: &source.local_functions,
            property_getters: &source.property_getters,
            property_setters: &source.property_setters,
            property_accessor_identities: &source.property_accessor_identities,
            initialization_units: &source.initialization_units,
            initialization_unit_identities: &source.initialization_unit_identities,
            derived_equality_applications: &source.derived_equality_applications,
            structs: &source.structs,
            enums: &source.enums,
            type_identities: &source.type_identities,
            struct_constructors: &source.struct_constructors,
            class_constructors: &source.class_constructors,
            constructor_identities: &source.constructor_identities,
            enum_member_identities: &source.enum_member_identities,
        },
        identities,
    )
    .unwrap();
}
