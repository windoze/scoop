use super::*;

impl Lowerer {
    pub(super) fn encode_struct(
        &mut self,
        context: CodingContext,
        value: hir::Expr,
        encoder: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let application = self
            .nominal_application(context.target)
            .expect("a struct has a nominal application");
        if self.nominal_intrinsic_kind(application.template).is_some() {
            self.error(
                span,
                "an intrinsic type requires an explicit encode implementation".into(),
            );
            return None;
        }
        let fields = self
            .struct_fields(context.target)
            .expect("a struct has its fields")
            .fields;
        let container = self.encoding_container(encoder, true, span, sink)?;
        let mut names = std::collections::HashSet::new();
        for (index, field) in fields.into_iter().enumerate() {
            let wire = if let Some(structure) = self.source_struct_id(application.template) {
                let source = hir::StructFieldRef::checked(&self.structs, structure, index as u32)
                    .expect("the field belongs to this struct");
                self.serialization_wire_name(
                    hir::SourceAnnotationTarget::Field(source),
                    &field.name,
                )
            } else {
                let source = self.loaded_struct_definitions[&application.template]
                    .declaration
                    .interface
                    .source_shape()
                    .declared_fields()[index]
                    .field();
                self.dependency_coding_wire_name(
                    hir::AnnotationTargetV1::Field(source),
                    &field.name,
                )
            };
            let Some(wire) = wire else { continue };
            self.check_encoding_wire_name(&wire, "field", span, &mut names)?;
            let field = self.coding_expr(
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(value.clone()),
                    field: field.reference,
                },
                field.ty,
                span,
            );
            self.encode_field(context, field, container.clone(), Some(&wire), span, sink)?;
        }
        self.end_coding_container(container, span, sink)
    }

    pub(super) fn check_encoding_wire_name(
        &mut self,
        wire: &str,
        kind: &str,
        span: Span,
        names: &mut std::collections::HashSet<String>,
    ) -> Option<()> {
        if !names.insert(wire.into()) {
            self.error(
                span,
                format!("automatic encode has duplicate {kind} name `{wire}`"),
            );
            return None;
        }
        Some(())
    }
}
