use super::*;
use crate::call_resolution::candidates::{
    ArgumentMode, CallableEffects, CallableSource, CallableView, NominalConstructorSource,
    NominalConstructorView, ReceiverShape, SourceDispatch, ValueParameter,
};
use crate::defaults::{DefaultArgumentSource, SourceParameterCalling, SourceVarargOmission};
use scoop_ast::{self as ast, Span};
use scoop_hir as hir;

fn argument(name: Option<&str>, spread: bool) -> ast::CallArgument {
    let span = Span::new(0, 0);
    ast::CallArgument {
        name: name.map_or(ast::CallArgumentName::Positional, |name| {
            ast::CallArgumentName::Named(ast::Ident {
                text: name.to_string(),
                span,
            })
        }),
        spread: if spread {
            ast::SpreadSyntax::Spread(span)
        } else {
            ast::SpreadSyntax::Plain
        },
        expression: ast::Expr::UnitLiteral { span },
        span,
    }
}

fn view(parameter_count: usize, receiver: ReceiverShape) -> CallableView {
    CallableView {
        target: CallableSource::Free(hir::FunctionId::from_raw(0_u32.into())),
        receiver,
        owner_parameters: Vec::new(),
        callable_parameters: Vec::new(),
        value_parameters: (0..parameter_count)
            .map(|index| ValueParameter {
                name: format!("p{index}"),
                calling: SourceParameterCalling::Required,
                ty: hir::TypeId::from_raw((index as u32).into()),
            })
            .collect(),
        return_type: hir::TypeId::from_raw(0_u32.into()),
        effects: CallableEffects {
            is_suspend: false,
            attributes: hir::FunctionAttributes::default(),
        },
        dispatch: SourceDispatch::Direct,
        declaration_span: Span::new(0, 0),
    }
}

fn template(index: u32) -> DefaultArgumentSource {
    DefaultArgumentSource::Ready(crate::defaults::DefaultExprTemplateRef::Export(
        hir::ExportDefaultSourceId::from_raw(index.into()),
    ))
}

#[test]
fn exact_mapping_is_candidate_owned_and_preserves_source_order() {
    let map = CandidateArgumentMap::exact_lowered(
        &view(
            2,
            ReceiverShape::Extension(hir::TypeId::from_raw(3_u32.into())),
        ),
        2,
    )
    .expect("matching positional arguments");
    assert_eq!(map.receiver, ReceiverInput::Present);
    assert_eq!(map.parameters[0].parameter.index(), 0);
    assert_eq!(
        map.parameters[1].input,
        ResolvedParameterInput::Explicit(SourceInputId::from_test_index(1))
    );
    assert_eq!(
        map.source_order
            .iter()
            .map(|input| input.index())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn exact_mapping_reports_candidate_arity() {
    assert_eq!(
        CandidateArgumentMap::exact_lowered(&view(2, ReceiverShape::None), 1),
        Err(ArgumentShapeFailure::Arity {
            expected: 2,
            supplied: 1,
        })
    );
}

#[test]
fn exact_nominal_mapping_uses_constructor_fields() {
    let view = NominalConstructorView {
        target: NominalConstructorSource::Struct(hir::StructConstructorId::from_raw(0_u32.into())),
        owner_parameters: Vec::new(),
        value_parameters: vec![
            ValueParameter {
                name: "left".to_string(),
                calling: SourceParameterCalling::Required,
                ty: hir::TypeId::from_raw(0_u32.into()),
            },
            ValueParameter {
                name: "right".to_string(),
                calling: SourceParameterCalling::Required,
                ty: hir::TypeId::from_raw(0_u32.into()),
            },
        ],
        argument_mode: ArgumentMode::Mixed,
        result_type: hir::TypeId::from_raw(0_u32.into()),
    };

    let args = [argument(None, false), argument(None, false)];
    let mapping = CandidateArgumentMap::source_nominal(&view, &args).expect("matching arity");
    assert_eq!(
        mapping.parameters[0].input,
        ResolvedParameterInput::Explicit(SourceInputId::from_test_index(0))
    );
    assert_eq!(mapping.parameters[1].parameter.index(), 1);
    assert_eq!(
        CandidateArgumentMap::source_nominal(&view, &[argument(None, false)]),
        Err(ArgumentShapeFailure::Arity {
            expected: 2,
            supplied: 1,
        })
    );
}

#[test]
fn source_mapping_supports_ordered_named_defaults_and_varargs() {
    let ty = hir::TypeId::from_raw(0_u32.into());
    let element = hir::TypeId::from_raw(1_u32.into());
    let mut view = view(0, ReceiverShape::None);
    view.value_parameters = vec![
        ValueParameter {
            name: "first".to_string(),
            calling: SourceParameterCalling::Required,
            ty,
        },
        ValueParameter {
            name: "middle".to_string(),
            calling: SourceParameterCalling::Default(template(0)),
            ty,
        },
        ValueParameter {
            name: "values".to_string(),
            calling: SourceParameterCalling::Vararg {
                element_type: element,
                array_type: ty,
                omission: SourceVarargOmission::EmptyArray,
            },
            ty,
        },
        ValueParameter {
            name: "last".to_string(),
            calling: SourceParameterCalling::Default(template(1)),
            ty,
        },
    ];

    let args = [
        argument(Some("first"), false),
        argument(Some("middle"), false),
        argument(None, false),
        argument(None, true),
        argument(Some("last"), false),
    ];
    let mapping = CandidateArgumentMap::source(&view, &args).expect("candidate shape");
    assert!(matches!(
        &mapping.parameters[2].input,
        ResolvedParameterInput::Vararg(ResolvedVarargInput::Parts(parts))
            if parts.len() == 2
                && parts[0].kind == VarargPartKind::Element
                && parts[1].kind == VarargPartKind::CopyArray
    ));
}

#[test]
fn jumping_named_argument_makes_the_remaining_tail_named_only() {
    let view = view(3, ReceiverShape::None);
    let failure =
        CandidateArgumentMap::source(&view, &[argument(Some("p2"), false), argument(None, false)])
            .expect_err("positional input after a jump is illegal");
    assert_eq!(failure, ArgumentShapeFailure::PositionalAfterNamed);
}
