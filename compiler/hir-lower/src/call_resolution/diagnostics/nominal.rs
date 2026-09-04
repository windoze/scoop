//! Source signatures and constraint failures for nominal constructor candidates.

use scoop_hir as hir;

use super::render_type_parameters;
use crate::Lowerer;
use crate::call_resolution::arguments::CandidateArgumentMap;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::call_resolution::constraints::{
    CallableShapeMismatch, ConstraintFailure, ConstraintFailureKind, ConstraintOrigin,
    InferenceVariableId, RelationKind, TypeTerm,
};

pub(crate) fn nominal_source_signature(lowerer: &Lowerer, view: &NominalConstructorView) -> String {
    let parameters =
        render_type_parameters(lowerer, &view.owner_parameters, &view.owner_parameters);
    let fields = view
        .value_parameters
        .iter()
        .map(|field| {
            format!(
                "{}: {}",
                field.name,
                lowerer.type_name_with_params(field.ty, &view.owner_parameters)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    match view.target {
        NominalConstructorSource::Struct(constructor) => {
            let structure = lowerer.struct_constructors[constructor].owner;
            format!(
                "struct {}{parameters}({fields})",
                lowerer.structs[structure].name
            )
        }
        NominalConstructorSource::Class(constructor) => {
            let class = lowerer.class_constructors[constructor].owner;
            format!(
                "class {}{parameters}({fields})",
                lowerer.classes[class].name
            )
        }
        NominalConstructorSource::IntrinsicClass(class) => {
            format!(
                "class {}{parameters}({fields})",
                lowerer.classes[class].name
            )
        }
        NominalConstructorSource::Variant {
            enumeration,
            variant,
        } => format!(
            "variant {}{parameters}.{}({fields})",
            lowerer.enums[enumeration].name,
            lowerer.enums[enumeration].variants[variant as usize].name,
        ),
    }
}

pub(crate) fn render_nominal_constraint_failure(
    lowerer: &Lowerer,
    view: &NominalConstructorView,
    argument_map: &CandidateArgumentMap,
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
            let source_index = input.index();
            let parameter_index = argument_map.source_binding(input).0.index();
            let found = arguments
                .get(source_index)
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
            CallableShapeMismatch::ExpectedCallable => "expected a callable type".to_string(),
            CallableShapeMismatch::Suspend => {
                "ordinary and suspend callable shapes differ".to_string()
            }
            CallableShapeMismatch::Arity { expected, actual } => format!(
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

fn relation_failure_phrase(relation: RelationKind) -> &'static str {
    match relation {
        RelationKind::Equal => "is not equal to",
        RelationKind::Subtype => "is not a subtype of",
    }
}

fn render_type_term(lowerer: &Lowerer, view: &NominalConstructorView, term: TypeTerm) -> String {
    match term {
        TypeTerm::Type(ty) | TypeTerm::Rigid(ty) => {
            lowerer.type_name_with_params(ty, &view.owner_parameters)
        }
        TypeTerm::Variable(variable) => inference_parameter(view, variable).name.clone(),
    }
}

fn inference_parameter(
    view: &NominalConstructorView,
    variable: InferenceVariableId,
) -> &hir::TypeParamDecl {
    match variable {
        InferenceVariableId::Owner(_) => &view.owner_parameters[variable.group_index()],
        InferenceVariableId::Callable(_) => {
            unreachable!("nominal constructors have no callable type parameters")
        }
    }
}
