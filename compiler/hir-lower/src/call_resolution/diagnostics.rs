//! Stable source-oriented diagnostics shared by calls and callable references.

use scoop_hir as hir;

use super::arguments::CandidateArgumentMap;
use super::candidates::{CallableSource, CallableView, ReceiverShape};
use super::constraints::{
    ConstraintFailure, ConstraintFailureKind, ConstraintOrigin, InferenceVariableId, TypeTerm,
};
use crate::Lowerer;

mod nominal;

pub(crate) use nominal::{nominal_source_signature, render_nominal_constraint_failure};

pub(crate) fn callable_layer_name(lowerer: &Lowerer, views: &[CallableView]) -> &'static str {
    if views
        .iter()
        .all(|view| matches!(view.receiver, ReceiverShape::Extension(_)))
    {
        return "extension candidate";
    }
    match views[0].target {
        CallableSource::Local { .. } => "lexical local candidate",
        CallableSource::Method(_) => "member candidate",
        CallableSource::Free(function) => {
            // "Implicit import" is the transitional core-unit surface:
            // a candidate whose declaring file is core while the call
            // site is user code (or vice versa) reads as imported.
            let call_site_is_core = lowerer.intrinsic_sources[lowerer.current_file].core;
            let candidate_is_core =
                lowerer.intrinsic_sources[lowerer.function_files[&function]].core;
            if call_site_is_core == candidate_is_core {
                "current-unit top-level candidate"
            } else {
                "implicit-import candidate"
            }
        }
    }
}

pub(crate) fn callable_source_signature(
    lowerer: &Lowerer,
    name: &str,
    view: &CallableView,
) -> String {
    let mut all_parameters = view.owner_parameters.clone();
    all_parameters.extend(view.callable_parameters.iter().cloned());
    let callable_parameters =
        render_type_parameters(lowerer, &view.callable_parameters, &all_parameters);
    let declared_name = match view.target {
        CallableSource::Method(_) => {
            let function_name = &lowerer.functions[view.function()].name;
            let (owner, _) = function_name
                .rsplit_once('.')
                .unwrap_or((function_name.as_str(), name));
            let owner_parameters =
                render_type_parameters(lowerer, &view.owner_parameters, &all_parameters);
            format!("{owner}{owner_parameters}.{name}{callable_parameters}")
        }
        CallableSource::Free(_) if let ReceiverShape::Extension(receiver) = view.receiver => {
            format!(
                "{}.{name}{callable_parameters}",
                lowerer.type_name_with_params(receiver, &all_parameters)
            )
        }
        CallableSource::Free(_) | CallableSource::Local { .. } => {
            format!("{name}{callable_parameters}")
        }
    };
    let parameters = view
        .value_parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                lowerer.type_name_with_params(parameter.ty, &all_parameters)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = lowerer.type_name_with_params(view.return_type, &all_parameters);
    let suspend = if view.effects.is_suspend {
        "suspend "
    } else {
        ""
    };
    format!("{suspend}fun {declared_name}({parameters}): {return_type}")
}

pub(crate) fn render_callable_constraint_failure(
    lowerer: &Lowerer,
    view: &CallableView,
    argument_map: Option<&CandidateArgumentMap>,
    arguments: &[Option<hir::Expr>],
    failure: &ConstraintFailure,
) -> String {
    let parameter = |variable| inference_parameter(view, variable);
    match &failure.kind {
        ConstraintFailureKind::Kind {
            variable,
            solution,
            required,
        } => {
            let required = match required {
                hir::TypeParamKind::Any => "any",
                hir::TypeParamKind::Value => "value",
                hir::TypeParamKind::Ref => "ref",
            };
            format!(
                "type argument `{}` for `{}` must satisfy `{required}`",
                lowerer.type_name(*solution),
                parameter(*variable).name,
            )
        }
        ConstraintFailureKind::InterfaceBound {
            variable,
            solution,
            required,
        } => format!(
            "type argument `{}` for `{}` must satisfy interface upper bound `{}`",
            lowerer.type_name(*solution),
            parameter(*variable).name,
            lowerer.type_name(*required),
        ),
        ConstraintFailureKind::ClassBound {
            variable,
            solution,
            required,
        } => format!(
            "type argument `{}` for `{}` must satisfy class upper bound `{}`",
            lowerer.type_name(*solution),
            parameter(*variable).name,
            lowerer.type_name(*required),
        ),
        ConstraintFailureKind::ConflictingExactBounds {
            variable,
            first,
            second,
        } => format!(
            "conflicting types for `{}`: {} and {}",
            parameter(*variable).name,
            lowerer.type_name(*first),
            lowerer.type_name(*second),
        ),
        ConstraintFailureKind::NoUniqueSolution {
            variable,
            lower_bounds,
            ..
        } if lower_bounds.len() >= 2 => format!(
            "conflicting types for `{}`: {} and {}",
            parameter(*variable).name,
            lowerer.type_name(lower_bounds[0]),
            lowerer.type_name(lower_bounds[1]),
        ),
        ConstraintFailureKind::NoUniqueSolution { variable, .. } => format!(
            "cannot infer a unique type argument for `{}`",
            parameter(*variable).name
        ),
        ConstraintFailureKind::UnresolvedTerm(TypeTerm::Variable(variable)) => {
            format!("cannot infer type argument `{}`", parameter(*variable).name)
        }
        ConstraintFailureKind::Relation {
            relation,
            left,
            right,
        } if matches!(failure.origin, ConstraintOrigin::Argument(_)) => {
            let ConstraintOrigin::Argument(input) = failure.origin else {
                unreachable!()
            };
            let receiver_offset = usize::from(matches!(view.receiver, ReceiverShape::Extension(_)));
            let source_index = input.index();
            let parameter_index = argument_map.map_or(source_index, |mapping| {
                mapping.source_binding(input).0.index()
            });
            let found = arguments
                .get(receiver_offset + source_index)
                .and_then(Option::as_ref)
                .map(|argument| lowerer.type_name(argument.ty))
                .unwrap_or_else(|| render_type_term(lowerer, view, *left));
            format!(
                "argument for `{}` has type {found}, which {} {}",
                view.value_parameters[parameter_index].name,
                relation_failure_phrase(*relation),
                render_type_term(lowerer, view, *right),
            )
        }
        ConstraintFailureKind::Relation {
            relation,
            left,
            right,
        } => format!(
            "{} {} {}",
            render_type_term(lowerer, view, *left),
            relation_failure_phrase(*relation),
            render_type_term(lowerer, view, *right),
        ),
        ConstraintFailureKind::CallableShape(mismatch) => match mismatch {
            super::constraints::CallableShapeMismatch::ExpectedCallable => {
                "expected a callable type".to_string()
            }
            super::constraints::CallableShapeMismatch::Suspend => {
                "ordinary and suspend callable shapes differ".to_string()
            }
            super::constraints::CallableShapeMismatch::Arity { expected, actual } => format!(
                "callable shape expects {expected} parameter(s), but the actual type has {actual}"
            ),
        },
        ConstraintFailureKind::ForeignVariable(_) => {
            "candidate references an inference variable from another session".to_string()
        }
        ConstraintFailureKind::ForeignTypeParameter(_) => {
            "candidate references a type parameter outside its declaration".to_string()
        }
        ConstraintFailureKind::UnresolvedTerm(term) => format!(
            "cannot resolve type term {}",
            render_type_term(lowerer, view, *term)
        ),
        ConstraintFailureKind::NonConcreteApplication(_) => {
            "candidate result is not a complete concrete type application".to_string()
        }
    }
}

fn relation_failure_phrase(relation: super::constraints::RelationKind) -> &'static str {
    match relation {
        super::constraints::RelationKind::Equal => "is not equal to",
        super::constraints::RelationKind::Subtype => "is not a subtype of",
    }
}

fn render_type_term(lowerer: &Lowerer, view: &CallableView, term: TypeTerm) -> String {
    match term {
        TypeTerm::Type(ty) | TypeTerm::Rigid(ty) => {
            let mut parameters = view.owner_parameters.clone();
            parameters.extend(view.callable_parameters.iter().cloned());
            lowerer.type_name_with_params(ty, &parameters)
        }
        TypeTerm::Variable(variable) => inference_parameter(view, variable).name.clone(),
    }
}

pub(super) fn render_type_parameters(
    lowerer: &Lowerer,
    parameters: &[hir::TypeParamDecl],
    all_parameters: &[hir::TypeParamDecl],
) -> String {
    if parameters.is_empty() {
        return String::new();
    }
    let parameters = parameters
        .iter()
        .map(|parameter| {
            let bound = match &parameter.bounds {
                hir::TypeParamBounds::Unconstrained => String::new(),
                hir::TypeParamBounds::Value { .. } => " : value".to_string(),
                hir::TypeParamBounds::Ref { .. } => " : ref".to_string(),
                hir::TypeParamBounds::Nominal(bounds) => {
                    let mut rendered = Vec::new();
                    if let Some(bound) = &bounds.class {
                        rendered.push((
                            bound.span.start,
                            lowerer.type_name_with_params(
                                lowerer.class_applications[bound.application].canonical_type,
                                all_parameters,
                            ),
                        ));
                    }
                    rendered.extend(bounds.interfaces.iter().map(|bound| {
                        (
                            bound.span.start,
                            lowerer.type_name_with_params(
                                lowerer.interface_applications[bound.application].canonical_type,
                                all_parameters,
                            ),
                        )
                    }));
                    rendered.sort_by_key(|(start, _)| *start);
                    format!(
                        " : {}",
                        rendered
                            .into_iter()
                            .map(|(_, value)| value)
                            .collect::<Vec<_>>()
                            .join(" & ")
                    )
                }
            };
            format!("{}{bound}", parameter.name)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("<{parameters}>")
}

fn inference_parameter(view: &CallableView, variable: InferenceVariableId) -> &hir::TypeParamDecl {
    match variable {
        InferenceVariableId::Owner(_) => &view.owner_parameters[variable.group_index()],
        InferenceVariableId::Callable(_) => &view.callable_parameters[variable.group_index()],
    }
}
