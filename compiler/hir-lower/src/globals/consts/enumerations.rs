use super::*;

impl Lowerer {
    pub(crate) fn resolve_local_integral_enum_constants(
        &mut self,
        enums: &[(hir::EnumId, ast::EnumSource<'_>, usize)],
    ) {
        self.resolve_integral_enum_constants(enums, &[], &[], &mut []);
    }

    pub(super) fn resolve_integral_enum_constants(
        &mut self,
        enums: &[(hir::EnumId, ast::EnumSource<'_>, usize)],
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
    ) {
        for &(id, declaration, file) in enums {
            let Some(kind) = self.enums[id].representation.integer_kind() else {
                continue;
            };
            self.current_file = file;
            self.current_owner = Some(crate::Owner::Enum(id));
            self.type_params_in_scope = self.enums[id].type_params.clone();
            let expected = self.intern_type(hir::Type::Integer(kind));
            let mut values = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for variant in declaration.variants {
                let ast::VariantDeclKind::Constant(expression) = &variant.kind else {
                    continue;
                };
                let Some(value) = self.evaluate_const_expression(
                    expression,
                    Some(expected),
                    file,
                    declarations,
                    ordinary,
                    states,
                    &mut Vec::new(),
                ) else {
                    continue;
                };
                let hir::ConstPropertyValue::Integer(value) = value.value else {
                    self.error(
                        expression.span(),
                        "integral enum values must be integer constants".into(),
                    );
                    continue;
                };
                if value.kind() != kind {
                    self.error(
                        expression.span(),
                        format!(
                            "integral enum value must have type {}, found {}",
                            kind.canonical_name(),
                            value.kind().canonical_name()
                        ),
                    );
                    continue;
                }
                if !seen.insert(value.raw_bits()) {
                    self.error(expression.span(), "duplicate integral enum value".into());
                }
                values.push(value.raw_bits());
            }
            self.enums[id].representation = hir::EnumRepresentation::Integral { kind, values };
            self.type_params_in_scope.clear();
        }
    }
}
