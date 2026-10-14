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
        let omissions = if let Some(structure) = self.source_struct_id(application.template) {
            let constructor = self
                .struct_primary_constructor(structure)
                .expect("declared struct fields have a primary constructor");
            self.current_coding_omissions(
                crate::call_resolution::candidates::NominalConstructorSource::Struct(constructor),
                span,
            )?
        } else {
            let constructor = self.loaded_struct_definitions[&application.template]
                .declaration
                .interface
                .declaration_details()
                .primary_value_constructor()
                .expect("declared struct fields have a primary constructor");
            self.dependency_coding_omissions(scoop_identity::CallableTemplateOrigin::Constructor(
                constructor,
            ))
        };
        let container = self.encoding_container(encoder, true, span, sink)?;
        let mut names = std::collections::HashSet::new();
        for (index, field) in fields.into_iter().enumerate() {
            let config = if let Some(structure) = self.source_struct_id(application.template) {
                let source = hir::StructFieldRef::checked(&self.structs, structure, index as u32)
                    .expect("the field belongs to this struct");
                self.serialization_config(hir::SourceAnnotationTarget::Field(source))
            } else {
                let source = self.loaded_struct_definitions[&application.template]
                    .declaration
                    .interface
                    .source_shape()
                    .declared_fields()[index]
                    .field();
                self.dependency_serialization_config(hir::AnnotationTargetV1::Field(source))
            };
            let Some(wire) = config.wire_name(&field.name) else {
                continue;
            };
            let omit_none = omissions[index] && !config.required && !config.emit_null;
            self.check_encoding_wire_name(&wire, "field", span, &mut names)?;
            let field = self.coding_expr(
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(value.clone()),
                    field: field.reference,
                },
                field.ty,
                span,
            );
            self.encode_field(
                context,
                field,
                container.clone(),
                Some(&EncodingMember { wire, omit_none }),
                span,
                sink,
            )?;
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
