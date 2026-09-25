//! Reject source-only implicit construction before lowering executable MIR.

use scoop_ast::{Diagnostic, Span};
use scoop_hir::{
    self as hir,
    concrete::{ConcreteCoreProtocols, ExecutableExpressionVisitError, ExprKind},
};
use scoop_wire::BudgetMeter;

const SOURCE_ONLY: &str = "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED: runtime cast failure constructor requires a materialized dependency layout; its owner has source-only representation";

pub(super) fn validate(
    output: &hir::DependencyHirOutput,
    world: &hir::ImportedSemanticWorld<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Vec<Diagnostic>> {
    let module = output.output().local.module();
    let ConcreteCoreProtocols::Imported(protocols) = &module.core_protocols else {
        return Ok(());
    };
    let nominal = protocols.exceptions().class_cast_exception();
    let mut checked = false;
    module
        .visit_executable_expressions(meter, |occurrence, meter| {
            let cast = matches!(
                occurrence.expression.kind,
                ExprKind::Cast {
                    optional: false,
                    ..
                }
            );
            if checked || !cast {
                return Ok(());
            }
            let span = occurrence.expression.span;
            let available = world.has_materializable_nominal_source(
                nominal.provider(),
                nominal.persistent(),
                meter,
            );
            let available = available.map_err(|error| {
                Diagnostic::at(
                    span,
                    format!("cannot validate runtime constructor source: {error}"),
                )
            })?;
            if !available {
                return Err(Diagnostic::at(span, SOURCE_ONLY));
            }
            checked = true;
            Ok(())
        })
        .map_err(|error| {
            let diagnostic = match error {
                ExecutableExpressionVisitError::Visitor(diagnostic) => diagnostic,
                ExecutableExpressionVisitError::Structure(error) => Diagnostic::at(
                    Span::new(0, 0),
                    format!("cannot inspect runtime constructor requirements: {error}"),
                ),
            };
            vec![diagnostic]
        })
}
