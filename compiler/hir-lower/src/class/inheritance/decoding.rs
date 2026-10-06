//! Missing decode requirements become ordinary methods over selected declarations.

use super::coding::{CodingContext, CodingDirection, SelectedCodec};
use super::*;
use crate::call_resolution::candidates::{
    NominalConstructorSource, ValueParameterCalling, VarargOmission,
};
use crate::imported_core::ImportedTypeBindings;
use ast::Span;
use hir::ImportedCallableSource;

mod classes;
mod codecs;
mod constructors;
mod declarations;
mod expressions;
mod records;
mod shapes;
mod variants;

#[derive(Clone)]
enum DecodeConstructor {
    Current {
        source: NominalConstructorSource,
        bindings: Vec<(hir::TypeParamId, TypeId)>,
    },
    Dependency {
        declaration: Box<hir::ImportedCallableDeclaration>,
        bindings: ImportedTypeBindings,
    },
}

#[derive(Clone, Copy)]
enum DecodeDefault {
    Current(crate::defaults::DefaultArgumentSource),
    Dependency(hir::ExportDefaultTemplateKeyV1),
}

struct DecodeParameter {
    name: String,
    ty: TypeId,
    wire: Option<String>,
    default: Option<DecodeDefault>,
}

pub(super) struct DecodeRecord {
    constructor: DecodeConstructor,
    parameters: Vec<DecodeParameter>,
    keyed: bool,
}

impl Lowerer {
    pub(crate) fn lower_derived_decoding_bodies(&mut self) {
        for (function, decodable) in std::mem::take(&mut self.derived_decoding_methods) {
            self.current_file = self.function_files[&function];
            let context = CodingContext {
                owner: self.function_owner[&function],
                target: self.signatures[&function].return_ty,
                interface: decodable,
                direction: CodingDirection::Decode,
            };
            let span = self.functions[function].span;
            let body = self.lower_synthesized_body(function, |state| {
                let decoder = state.functions[function].params[1].clone();
                let decoder =
                    state.coding_expr(hir::ExprKind::Local(decoder.local), decoder.ty, span);
                let mut statements = Vec::new();
                let before = state.diagnostics.len();
                if let Some(value) = state.decode_result(context, decoder, span, &mut statements) {
                    statements.push(hir::Statement {
                        kind: hir::StatementKind::Return { value: Some(value) },
                        span,
                    });
                } else if state.diagnostics.len() == before {
                    state.error(span, "automatic decode could not resolve the required core decoder or constructor".into());
                }
                statements
            });
            self.functions[function].kind = hir::FunctionKind::User(body);
        }
    }

    fn decode_result(
        &mut self,
        context: CodingContext,
        decoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        match self.types[context.target].clone() {
            Type::Struct(_) | Type::Class(_) => {
                let record = self.decode_record_shape(context.target, span)?;
                let inputs = self.read_decoding_record(context, &record, decoder, span, sink)?;
                self.finish_decoding_record(context.target, &record, inputs, span, sink)
            }
            Type::Enum(_) => self.decode_variants(context, decoder, span, sink),
            Type::Tuple(fields) => {
                let codecs = fields
                    .into_iter()
                    .map(|ty| self.select_field_codec(context, ty, span))
                    .collect::<Option<Vec<_>>>()?;
                self.decode_tuple(context, context.target, &codecs, decoder, span, sink)
            }
            _ => {
                self.error(span, format!("automatic decode requires a struct, enum, tuple, or final class with a primary constructor; result {} requires an explicit implementation", self.type_name(context.target)));
                None
            }
        }
    }
}
