use super::*;

pub(super) enum DecodedInput {
    Value(hir::Expr),
    Optional {
        value: hir::Expr,
        default: DecodeDefault,
    },
    Default(DecodeDefault),
}

impl Lowerer {
    pub(super) fn read_decoding_record(
        &mut self,
        context: CodingContext,
        record: &DecodeRecord,
        decoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<Vec<DecodedInput>> {
        let mut names = std::collections::HashSet::new();
        for parameter in &record.parameters {
            if let Some(name) = &parameter.wire {
                if record.keyed && !names.insert(name) {
                    self.error(
                        span,
                        format!("automatic decode has duplicate field name `{name}`"),
                    );
                    return None;
                }
            } else if parameter.default.is_none() {
                self.error(span, format!("automatic decode requires a declared default for omitted constructor parameter `{}`", parameter.name));
                return None;
            }
        }
        let container = self.coding_call(
            decoder,
            if record.keyed { "keyed" } else { "unkeyed" },
            Vec::new(),
            span,
            sink,
        )?;
        let container = self.coding_local(container, span, sink);
        let mut inputs = Vec::with_capacity(record.parameters.len());
        for parameter in &record.parameters {
            let Some(name) = &parameter.wire else {
                inputs.push(DecodedInput::Default(
                    parameter
                        .default
                        .expect("an omitted parameter has a checked default"),
                ));
                continue;
            };
            let codec = self.select_field_codec(context, parameter.ty, span)?;
            let input = if record.keyed {
                let name = self.coding_string(name, span);
                self.coding_call(
                    container.clone(),
                    if parameter.default.is_some() {
                        "optional"
                    } else {
                        "required"
                    },
                    vec![name],
                    span,
                    sink,
                )?
            } else {
                self.coding_call(container.clone(), "element", Vec::new(), span, sink)?
            };
            if record.keyed
                && let Some(default) = parameter.default
            {
                let child = self.coding_local(input, span, sink);
                let decoder_type = self
                    .as_option(child.ty)
                    .expect("optional returns Option<Decoder>");
                let condition = self.coding_expr(
                    hir::ExprKind::IsSome(Box::new(child.clone())),
                    self.boolean,
                    span,
                );
                let provided = self.coding_expr(
                    hir::ExprKind::Unwrap {
                        operand: Box::new(child),
                        trap_on_none: false,
                    },
                    decoder_type,
                    span,
                );
                let mut yes = Vec::new();
                let value =
                    self.decode_selected_value(context, &codec, provided, span, &mut yes)?;
                let option = self.option_type(parameter.ty);
                let value =
                    self.coding_expr(hir::ExprKind::SomeWrap(Box::new(value)), option, span);
                let missing = self.coding_expr(hir::ExprKind::NoneLiteral, option, span);
                let value = self.decoding_branch(
                    condition,
                    option,
                    (yes, value),
                    (Vec::new(), missing),
                    span,
                    sink,
                );
                inputs.push(DecodedInput::Optional { value, default });
            } else {
                inputs.push(DecodedInput::Value(
                    self.decode_selected_value(context, &codec, input, span, sink)?,
                ));
            }
        }
        self.end_coding_container(container, span, sink)?;
        Some(inputs)
    }

    pub(super) fn finish_decoding_record(
        &mut self,
        result: TypeId,
        record: &DecodeRecord,
        inputs: Vec<DecodedInput>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let mut arguments = Vec::with_capacity(inputs.len());
        for (parameter, input) in record.parameters.iter().zip(inputs) {
            let value = match input {
                DecodedInput::Value(value) => value,
                DecodedInput::Default(default) => {
                    self.decoding_default(&record.constructor, default, &arguments, span, sink)?
                }
                DecodedInput::Optional { value, default } => {
                    let condition = self.coding_expr(
                        hir::ExprKind::IsSome(Box::new(value.clone())),
                        self.boolean,
                        span,
                    );
                    let provided = self.coding_expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(value),
                            trap_on_none: false,
                        },
                        parameter.ty,
                        span,
                    );
                    let mut missing = Vec::new();
                    let default = self.decoding_default(
                        &record.constructor,
                        default,
                        &arguments,
                        span,
                        &mut missing,
                    )?;
                    let default = self.adapt_to(default, parameter.ty);
                    self.decoding_branch(
                        condition,
                        parameter.ty,
                        (Vec::new(), provided),
                        (missing, default),
                        span,
                        sink,
                    )
                }
            };
            let value = self.adapt_to(value, parameter.ty);
            arguments.push(self.coding_local(value, span, sink));
        }
        self.call_coding_constructor(result, record, arguments, span, sink)
    }
}
