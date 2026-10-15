use super::*;

pub(super) struct InspectionMember {
    pub name: String,
    pub index: usize,
}

impl Lowerer {
    pub(super) fn inspect_struct(
        &mut self,
        context: CodingContext,
        target: TypeId,
        value: hir::Expr,
        reference: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let application = self.nominal_application(target).unwrap();
        if self.nominal_intrinsic_kind(application.template).is_some() {
            self.error(
                span,
                "an intrinsic type requires an explicit inspect implementation".into(),
            );
            return None;
        }
        let fields = self
            .struct_fields(target)?
            .fields
            .into_iter()
            .enumerate()
            .map(|(index, field)| InspectionMember {
                name: field.name,
                index,
            })
            .collect::<Vec<_>>();
        let info = self.inspection_shape_info(reference, "Struct", span, sink)?;
        let metadata = self.inspection_field(info, "fields", span)?;
        self.inspection_record(context, value, metadata, fields, false, span, sink)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn inspection_record(
        &mut self,
        context: CodingContext,
        value: hir::Expr,
        metadata: hir::Expr,
        fields: Vec<InspectionMember>,
        properties: bool,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        self.inspection_bind("__inspection_record", value, span, sink);
        let metadata = self.coding_local(metadata, span, sink);
        let view_ty = self.inspection_type("ValueView", span)?;
        let member_ty = self.inspection_type("MemberView", span)?;
        let mut members = Vec::new();
        for field in fields {
            let reader = self.lower_synthesized_lambda(false, &[], view_ty, span, |state, _| {
                let value = state.inspection_capture("__inspection_record", span)?;
                let value = state
                    .inspection_field(value, &field.name, span)
                    .or_else(|| {
                        state.error(
                            span,
                            format!(
                                "automatic inspect cannot read member `{}` from this inspector",
                                field.name
                            ),
                        );
                        None
                    })?;
                let mut statements = Vec::new();
                let value = state.inspection_read(context, value, span, &mut statements)?;
                Some(crate::stmt::ValueBlock {
                    statements,
                    value: Some(value),
                })
            })?;
            let index = self.inspection_long(field.index, span);
            let info = self.inspection_index(metadata.clone(), index, span);
            let info = self.inspection_variant(
                "ObservedMember",
                if properties { "Property" } else { "Field" },
                vec![info],
                span,
            )?;
            members.push(self.inspection_construct(
                "MemberView",
                vec![info, reader],
                span,
                sink,
            )?);
        }
        let members = self.inspection_array(member_ty, members, span);
        let base_ty = self.option_type(view_ty);
        let base = self.coding_expr(hir::ExprKind::NoneLiteral, base_ty, span);
        let record = self.inspection_construct("RecordView", vec![members, base], span, sink)?;
        self.inspection_variant("ValueShape", "Record", vec![record], span)
    }

    pub(super) fn inspection_read(
        &mut self,
        context: CodingContext,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let selected = self.select_field_codec(context, value.ty, span)?;
        let inspector = self.selected_codec_value(context, &selected, span, sink)?;
        self.coding_call(inspector, "inspect", vec![value], span, sink)
    }
}
