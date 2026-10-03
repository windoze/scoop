use super::*;

impl Lowerer {
    pub(super) fn lower_struct_binding(
        &mut self,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        span: Span,
        subject: BindingSubject,
        mutable: bool,
        statements: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let structure = self
            .struct_fields(subject.ty)
            .expect("a struct pattern has struct fields");
        let owner = format!("struct `{}`", structure.name);
        let indices =
            self.positional_pattern_indices(elements, rest, structure.fields.len(), &owner, span)?;
        for (element, index) in elements.iter().zip(indices) {
            let field = &structure.fields[index];
            self.lower_projected_binding(
                element,
                subject,
                (field.reference, field.ty),
                pattern_span(element),
                mutable,
                statements,
            )?;
        }
        Some(())
    }

    pub(super) fn lower_named_struct_binding(
        &mut self,
        fields: &[ast::FieldPattern],
        rest: Option<Span>,
        span: Span,
        subject: BindingSubject,
        mutable: bool,
        statements: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let structure = self
            .struct_fields(subject.ty)
            .expect("a struct pattern has struct fields");
        let owner = format!("struct `{}`", structure.name);
        let names = structure
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>();
        let indices = self.named_pattern_indices(fields, rest, &names, &owner, span)?;
        for (pattern, index) in fields.iter().zip(indices) {
            let field = &structure.fields[index];
            self.lower_projected_binding(
                &pattern.subpattern,
                subject,
                (field.reference, field.ty),
                pattern.field.span,
                mutable,
                statements,
            )?;
        }
        Some(())
    }

    pub(super) fn lower_component_binding(
        &mut self,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        subject: BindingSubject,
        mutable: bool,
        statements: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        if let Some(rest) = rest {
            self.error(rest, "a class component pattern cannot contain `..`".into());
            return None;
        }
        for (position, element) in elements.iter().enumerate() {
            let index = std::num::NonZeroU32::new(
                u32::try_from(position)
                    .expect("source pattern positions fit u32")
                    .checked_add(1)
                    .expect("component indices fit u32"),
            )
            .expect("component indices start at one");
            let span = pattern_span(element);
            let before = self.diagnostics.len();
            let call = self.lower_component_call(
                subject.expression(span, self.expression_origin(span)),
                index,
                span,
                statements,
            )?;
            if self.diagnostics.len() != before {
                return None;
            }
            let result = BindingSubject {
                local: self.alloc_hidden("binding.component", call.ty),
                ty: call.ty,
            };
            statements.push(binding_statement(result.local, call, span));
            self.lower_binding_pattern(element, result, mutable, statements)?;
        }
        Some(())
    }
}
