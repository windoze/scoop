use scoop_hir as hir;

use crate::Lowerer;

use super::TypeOccurrence;
use super::body::{
    collect_body_type_occurrences, collect_expr_type_occurrences,
    collect_statement_type_occurrences,
};
use super::types::{collect_body_types, collect_expr_types, collect_statement_types};

pub(in super::super) fn collect_class_constructor_types(
    lowerer: &Lowerer,
    constructor: &hir::ClassConstructor,
    out: &mut Vec<hir::TypeId>,
) {
    match &constructor.kind {
        hir::ClassConstructorKind::Primary {
            base,
            common_initialization,
            ..
        } => {
            collect_base_initialization_types(lowerer, base, out);
            collect_class_initialization_types(lowerer, common_initialization, out);
        }
        hir::ClassConstructorKind::Secondary { delegation, body } => {
            match delegation {
                hir::ClassSecondaryDelegation::This { arguments, .. } => {
                    collect_constructor_arguments_types(lowerer, arguments, out)
                }
                hir::ClassSecondaryDelegation::Terminal {
                    base,
                    common_initialization,
                } => {
                    collect_base_initialization_types(lowerer, base, out);
                    collect_class_initialization_types(lowerer, common_initialization, out);
                }
            }
            collect_body_types(lowerer, body, out);
        }
    }
}

fn collect_base_initialization_types(
    lowerer: &Lowerer,
    base: &hir::BaseInitialization,
    out: &mut Vec<hir::TypeId>,
) {
    if let hir::BaseInitialization::Super { arguments, .. } = base {
        collect_constructor_arguments_types(lowerer, arguments, out);
    }
}

fn collect_class_initialization_types(
    lowerer: &Lowerer,
    initialization: &[hir::ClassInitializationStep],
    out: &mut Vec<hir::TypeId>,
) {
    for step in initialization {
        match step {
            hir::ClassInitializationStep::Field { initializer, .. } => {
                collect_constructor_expression_types(lowerer, initializer, out)
            }
            hir::ClassInitializationStep::InitBlock { body, .. } => {
                collect_body_types(lowerer, body, out)
            }
        }
    }
}

pub(in super::super) fn collect_constructor_arguments_types(
    lowerer: &Lowerer,
    arguments: &hir::ConstructorArguments,
    out: &mut Vec<hir::TypeId>,
) {
    out.extend(arguments.locals.iter().map(|(_, local)| local.ty));
    collect_statement_types(lowerer, &arguments.statements, out);
    for argument in &arguments.args {
        collect_expr_types(lowerer, argument, out);
    }
}

fn collect_constructor_expression_types(
    lowerer: &Lowerer,
    expression: &hir::ConstructorExpression,
    out: &mut Vec<hir::TypeId>,
) {
    out.extend(expression.locals.iter().map(|(_, local)| local.ty));
    collect_statement_types(lowerer, &expression.statements, out);
    collect_expr_types(lowerer, &expression.value, out);
}

pub(in super::super) fn collect_struct_constructor_type_occurrences(
    lowerer: &Lowerer,
    constructor: &hir::StructConstructor,
    out: &mut Vec<TypeOccurrence>,
) {
    let hir::StructConstructorKind::Secondary {
        delegation, body, ..
    } = &constructor.kind
    else {
        return;
    };
    let file = constructor.origin.file as usize;
    collect_constructor_arguments_type_occurrences(lowerer, &delegation.arguments, file, out);
    collect_body_type_occurrences(lowerer, body, file, out);
}

pub(in super::super) fn collect_class_constructor_type_occurrences(
    lowerer: &Lowerer,
    constructor: &hir::ClassConstructor,
    out: &mut Vec<TypeOccurrence>,
) {
    let file = constructor.origin.file as usize;
    match &constructor.kind {
        hir::ClassConstructorKind::Primary {
            base,
            common_initialization,
            ..
        } => {
            collect_base_initialization_type_occurrences(lowerer, base, file, out);
            collect_class_initialization_type_occurrences(
                lowerer,
                common_initialization,
                file,
                out,
            );
        }
        hir::ClassConstructorKind::Secondary { delegation, body } => {
            match delegation {
                hir::ClassSecondaryDelegation::This { arguments, .. } => {
                    collect_constructor_arguments_type_occurrences(lowerer, arguments, file, out);
                }
                hir::ClassSecondaryDelegation::Terminal {
                    base,
                    common_initialization,
                } => {
                    collect_base_initialization_type_occurrences(lowerer, base, file, out);
                    collect_class_initialization_type_occurrences(
                        lowerer,
                        common_initialization,
                        file,
                        out,
                    );
                }
            }
            collect_body_type_occurrences(lowerer, body, file, out);
        }
    }
}

fn collect_base_initialization_type_occurrences(
    lowerer: &Lowerer,
    base: &hir::BaseInitialization,
    file: usize,
    out: &mut Vec<TypeOccurrence>,
) {
    if let hir::BaseInitialization::Super { arguments, .. } = base {
        collect_constructor_arguments_type_occurrences(lowerer, arguments, file, out);
    }
}

fn collect_class_initialization_type_occurrences(
    lowerer: &Lowerer,
    initialization: &[hir::ClassInitializationStep],
    file: usize,
    out: &mut Vec<TypeOccurrence>,
) {
    for step in initialization {
        match step {
            hir::ClassInitializationStep::Field { initializer, .. } => {
                collect_statement_type_occurrences(lowerer, &initializer.statements, file, out);
                collect_expr_type_occurrences(lowerer, &initializer.value, out);
            }
            hir::ClassInitializationStep::InitBlock { body, .. } => {
                collect_body_type_occurrences(lowerer, body, file, out);
            }
        }
    }
}

fn collect_constructor_arguments_type_occurrences(
    lowerer: &Lowerer,
    arguments: &hir::ConstructorArguments,
    file: usize,
    out: &mut Vec<TypeOccurrence>,
) {
    collect_statement_type_occurrences(lowerer, &arguments.statements, file, out);
    for argument in &arguments.args {
        collect_expr_type_occurrences(lowerer, argument, out);
    }
}
