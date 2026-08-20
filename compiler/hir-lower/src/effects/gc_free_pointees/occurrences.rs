use scoop_hir as hir;

mod body;
mod declarations;
mod types;

pub(super) use body::{collect_body_type_occurrences, collect_default_type_occurrences};
pub(super) use declarations::{
    collect_class_constructor_type_occurrences, collect_class_constructor_types,
    collect_constructor_arguments_types, collect_struct_constructor_type_occurrences,
};
pub(super) use types::{collect_body_types, collect_expr_types, collect_statement_types};

#[derive(Debug, Clone, Copy)]
pub(super) struct TypeOccurrence {
    pub(super) ty: hir::TypeId,
    pub(super) file: usize,
    pub(super) span: scoop_ast::Span,
}
