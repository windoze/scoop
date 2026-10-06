//! Missing encode requirements become ordinary methods on the selected codec.

use super::coding::{CodingContext, CodingDirection, SelectedCodec};
use super::interfaces::InterfaceMemberInstance;
use super::*;
use ast::Span;

mod classes;
mod declarations;
mod fields;
mod values;
mod variants;

impl Lowerer {
    pub(crate) fn lower_derived_encoding_bodies(&mut self) {
        for (function, interface) in std::mem::take(&mut self.derived_encoding_methods) {
            self.current_file = self.function_files[&function];
            let context = CodingContext {
                owner: self.function_owner[&function],
                target: self.signatures[&function].params[0].ty,
                interface,
                direction: CodingDirection::Encode,
            };
            let span = self.functions[function].span;
            let body = self.lower_synthesized_body(function, |state| {
                let parameter = |index: usize| {
                    let parameter = &state.functions[function].params[index];
                    state.coding_expr(hir::ExprKind::Local(parameter.local), parameter.ty, span)
                };
                let value = parameter(1);
                let encoder = parameter(2);
                let before = state.diagnostics.len();
                let mut statements = Vec::new();
                if state.encode_target(context, value, encoder, span, &mut statements).is_some() {
                    statements.push(hir::Statement {
                        kind: hir::StatementKind::Return { value: None },
                        span,
                    });
                } else if state.diagnostics.len() == before {
                    state.error(span, "automatic encode could not resolve the required core codec or field access".into());
                }
                statements
            });
            self.functions[function].kind = hir::FunctionKind::User(body);
        }
    }

    fn encode_target(
        &mut self,
        context: CodingContext,
        value: hir::Expr,
        encoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        match self.types[context.target].clone() {
            Type::Struct(_) => self.encode_struct(context, value, encoder, span, sink),
            Type::Class(_) => self.encode_class(context, value, encoder, span, sink),
            Type::Enum(_) => self.encode_enum(context, value, encoder, span, sink),
            Type::Tuple(fields) => {
                let codecs = fields
                    .into_iter()
                    .map(|ty| self.select_field_codec(context, ty, span))
                    .collect::<Option<Vec<_>>>()?;
                self.encode_tuple(context, &codecs, value, encoder, span, sink)
            }
            _ => {
                self.error(span, format!("automatic encode requires a struct, enum, tuple, or final class without a class base; target {} requires an explicit implementation", self.type_name(context.target)));
                None
            }
        }
    }
}
