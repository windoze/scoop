use super::*;
use crate::Type;
use crate::call_resolution::arguments::CandidateArgumentMap;
use crate::call_resolution::candidates::{
    ArgumentMode, CallableEffects, CallableSource, NominalConstructorSource, ReceiverShape,
    ValueParameter,
};
use crate::defaults::{DefaultExprTemplateRef, SourceParameterCalling, SourceVarargOmission};
use scoop_ast as ast;

fn parameter(identity: u32, slot: u32) -> hir::TypeParamDecl {
    hir::TypeParamDecl {
        id: hir::TypeParamId::with_substitution_slot(identity, slot),
        name: format!("T{identity}"),
        bounds: hir::TypeParamBounds::Unconstrained,
        span: ast::Span::new(0, 0),
    }
}

fn callable(
    owners: Vec<hir::TypeParamDecl>,
    parameters: Vec<hir::TypeParamDecl>,
    values: Vec<ValueParameter>,
    result: hir::TypeId,
) -> CallableView {
    CallableView {
        target: CallableSource::Free(hir::FunctionId::from_raw(0.into())),
        receiver: ReceiverShape::None,
        owner_parameters: owners,
        callable_parameters: parameters,
        value_parameters: values,
        return_type: result,
        effects: CallableEffects {
            is_suspend: false,
            attributes: hir::FunctionAttributes::default(),
        },
        dispatch: crate::CallableCandidateSource::Direct,
        declaration_span: ast::Span::new(0, 0),
    }
}

fn constructor(
    owners: Vec<hir::TypeParamDecl>,
    values: Vec<ValueParameter>,
    result: hir::TypeId,
) -> NominalConstructorView {
    NominalConstructorView {
        target: NominalConstructorSource::Struct(hir::StructConstructorId::from_raw(0.into())),
        owner_parameters: owners,
        value_parameters: values,
        argument_mode: ArgumentMode::Mixed,
        result_type: result,
    }
}

#[test]
fn cross_kind_forwarding_keeps_source_parameters_rigid_and_state_unchanged() {
    let mut state = Lowerer::new();
    let owner = parameter(1, 0);
    let owner_ty = state.intern_type(Type::Param(owner.id));
    let integer = state.integer_type(hir::IntegerKind::SIGNED_32);
    let generic_constructor = constructor(vec![owner], Vec::new(), state.unit);
    let function = callable(Vec::new(), Vec::new(), Vec::new(), state.unit);
    let source_types = [owner_ty];
    let target_types = [integer];
    let source = DeclarationForwardingView::from(NominalForwardingDeclaration {
        view: &generic_constructor,
        parameter_types: &source_types,
    });
    let target = DeclarationForwardingView::from(ForwardingDeclaration {
        view: &function,
        parameter_types: &target_types,
    });
    let types_before = state.types.len();
    for _ in 0..2 {
        assert!(
            !state.declaration_forwards(source, target),
            "a rigid constructor owner parameter cannot be inferred as Int"
        );
        assert!(
            state.declaration_forwards(target, source),
            "the target constructor parameter is fresh"
        );
    }
    assert_eq!(state.types.len(), types_before);
    assert!(
        state.type_params_in_scope.is_empty(),
        "source skolems stay inside the probe"
    );
    assert!(state.diagnostics.is_empty());
}

#[test]
fn owner_and_callable_binders_do_not_collapse_across_constructor_comparison() {
    let mut state = Lowerer::new();
    let owner = parameter(10, 0);
    let method = parameter(11, 1);
    let target_owner = parameter(12, 0);
    let mut pointer = |parameter: &hir::TypeParamDecl| {
        let parameter = state.intern_type(Type::Param(parameter.id));
        state.intern_type(Type::Ptr(parameter))
    };
    let source_types = [pointer(&owner), pointer(&method)];
    let target_ty = pointer(&target_owner);
    let target_types = [target_ty, target_ty];
    let function = callable(vec![owner], vec![method], Vec::new(), state.unit);
    let nominal = constructor(vec![target_owner], Vec::new(), state.unit);
    let source = DeclarationForwardingView::from(ForwardingDeclaration {
        view: &function,
        parameter_types: &source_types,
    });
    let target = DeclarationForwardingView::from(NominalForwardingDeclaration {
        view: &nominal,
        parameter_types: &target_types,
    });
    assert_eq!(source.owner_parameters.len(), 1);
    assert_eq!(source.callable_parameters.len(), 1);
    assert_eq!(target.owner_parameters.len(), 1);
    assert!(target.callable_parameters.is_empty());
    assert!(
        !state.declaration_forwards(source, target),
        "invariant inputs cannot identify distinct rigid owner/callable parameters"
    );
    assert!(state.declaration_forwards(target, source));
}

#[test]
fn cross_kind_forwarding_checks_both_source_and_target_kind_bounds() {
    let mut state = Lowerer::new();
    let mut source_parameter = parameter(20, 0);
    source_parameter.bounds = hir::TypeParamBounds::Value {
        span: ast::Span::new(0, 0),
    };
    let mut target_parameter = parameter(21, 0);
    target_parameter.bounds = hir::TypeParamBounds::Ref {
        span: ast::Span::new(0, 0),
    };
    let source_types = [state.intern_type(Type::Param(source_parameter.id))];
    let target_types = [state.intern_type(Type::Param(target_parameter.id))];
    let source = constructor(vec![source_parameter], Vec::new(), state.unit);
    let mut target = callable(Vec::new(), vec![target_parameter], Vec::new(), state.unit);
    assert!(
        !state.declaration_forwards(
            NominalForwardingDeclaration {
                view: &source,
                parameter_types: &source_types
            }
            .into(),
            ForwardingDeclaration {
                view: &target,
                parameter_types: &target_types
            }
            .into()
        )
    );
    target.callable_parameters[0].bounds = hir::TypeParamBounds::Value {
        span: ast::Span::new(0, 0),
    };
    assert!(
        state.declaration_forwards(
            NominalForwardingDeclaration {
                view: &source,
                parameter_types: &source_types
            }
            .into(),
            ForwardingDeclaration {
                view: &target,
                parameter_types: &target_types
            }
            .into()
        )
    );
}

#[test]
fn fixed_application_parameter_types_are_not_replaced_by_generic_owner_binders() {
    let mut state = Lowerer::new();
    let owner = parameter(30, 0);
    let template_type = state.intern_type(Type::Param(owner.id));
    let integer = state.integer_type(hir::IntegerKind::SIGNED_32);
    let generic = constructor(vec![owner], Vec::new(), state.unit);
    let fixed = constructor(Vec::new(), Vec::new(), state.unit);
    let function = callable(Vec::new(), Vec::new(), Vec::new(), state.unit);
    let generic_types = [template_type];
    let fixed_types = [integer];
    let function = ForwardingDeclaration {
        view: &function,
        parameter_types: &fixed_types,
    };
    let generic = NominalForwardingDeclaration {
        view: &generic,
        parameter_types: &generic_types,
    };
    let fixed = NominalForwardingDeclaration {
        view: &fixed,
        parameter_types: &fixed_types,
    };
    assert!(!state.declaration_forwards(generic.into(), function.into()));
    assert!(state.declaration_forwards(fixed.into(), function.into()));
    assert!(state.declaration_forwards(function.into(), fixed.into()));
    assert!(
        !state.declaration_forwards(
            DeclarationForwardingView {
                parameter_types: &[],
                ..fixed.into()
            },
            function.into()
        ),
        "source arity is checked before constraint solving"
    );
}

fn argument(name: Option<&str>, spread: bool) -> ast::CallArgument {
    ast::CallArgument {
        name: name.map_or(ast::CallArgumentName::Positional, |name| {
            ast::CallArgumentName::Named(ast::Ident {
                text: name.to_string(),
                span: ast::Span::new(0, 0),
            })
        }),
        spread: if spread {
            ast::SpreadSyntax::Spread(ast::Span::new(0, 0))
        } else {
            ast::SpreadSyntax::Plain
        },
        expression: ast::Expr::UnitLiteral {
            span: ast::Span::new(0, 0),
        },
        span: ast::Span::new(0, 0),
    }
}

#[test]
fn callable_and_constructor_mappings_share_default_and_named_source_order() {
    let state = Lowerer::new();
    let integer = state.integer_type(hir::IntegerKind::SIGNED_32);
    let default = crate::defaults::DefaultArgumentSource::Ready(DefaultExprTemplateRef::Export(
        hir::ExportDefaultSourceId::from_raw(0.into()),
    ));
    let parameters = vec![
        ValueParameter {
            name: "first".to_string(),
            calling: SourceParameterCalling::Default(default),
            ty: integer,
        },
        ValueParameter {
            name: "second".to_string(),
            calling: SourceParameterCalling::Required,
            ty: state.boolean,
        },
    ];
    let function = callable(Vec::new(), Vec::new(), parameters.clone(), state.unit);
    let nominal = constructor(Vec::new(), parameters, state.unit);
    for (args, expected, defaults) in [
        (
            vec![argument(Some("second"), false)],
            vec![state.boolean],
            1,
        ),
        (
            vec![
                argument(Some("second"), false),
                argument(Some("first"), false),
            ],
            vec![state.boolean, integer],
            0,
        ),
    ] {
        let function_map = CandidateArgumentMap::source(&function, &args).unwrap();
        let nominal_map = CandidateArgumentMap::source_nominal(&nominal, &args).unwrap();
        let function_types = function_map.forwarding_parameter_types(&function.value_parameters);
        let nominal_types = nominal_map.forwarding_parameter_types(&nominal.value_parameters);
        assert_eq!(function_types, expected);
        assert_eq!(nominal_types, expected);
        assert_eq!(function_map.explicit_default_count(), defaults);
        assert_eq!(nominal_map.explicit_default_count(), defaults);
        assert!(
            state.declaration_forwards(
                ForwardingDeclaration {
                    view: &function,
                    parameter_types: &function_types
                }
                .into(),
                NominalForwardingDeclaration {
                    view: &nominal,
                    parameter_types: &nominal_types
                }
                .into()
            )
        );
    }
}

#[test]
fn callable_and_constructor_vararg_mappings_preserve_element_and_array_inputs() {
    // Mapping is a pure projection over opaque typed parameter handles; it
    // does not inspect an array declaration or evaluate source expressions.
    let element = hir::TypeId::from_raw(1.into());
    let array = hir::TypeId::from_raw(2.into());
    let parameters = vec![ValueParameter {
        name: "values".to_string(),
        calling: SourceParameterCalling::Vararg {
            element_type: element,
            array_type: array,
            omission: SourceVarargOmission::EmptyArray,
        },
        ty: array,
    }];
    let function = callable(Vec::new(), Vec::new(), parameters.clone(), element);
    let nominal = constructor(Vec::new(), parameters, element);
    for (args, expected) in [
        (Vec::new(), Vec::new()),
        (
            vec![argument(None, false), argument(None, false)],
            vec![element, element],
        ),
        (
            vec![argument(None, false), argument(None, true)],
            vec![element, array],
        ),
        (vec![argument(Some("values"), false)], vec![array]),
    ] {
        let function_map = CandidateArgumentMap::source(&function, &args).unwrap();
        let nominal_map = CandidateArgumentMap::source_nominal(&nominal, &args).unwrap();
        assert_eq!(
            function_map.forwarding_parameter_types(&function.value_parameters),
            expected
        );
        assert_eq!(
            nominal_map.forwarding_parameter_types(&nominal.value_parameters),
            expected
        );
    }
}
