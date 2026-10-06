//! Recover checked patterns without repeating member selection.

use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_default_pattern(
        &mut self,
        pattern: &hir::DefaultPatternV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::Pattern, ImportedDefaultMaterializationError> {
        match pattern.view() {
            hir::DefaultPatternViewV1::Binding { local } => self
                .materialized_imported_default_local(local, context)
                .map(|local| hir::Pattern::Binding { local }),
            hir::DefaultPatternViewV1::Wildcard => Ok(hir::Pattern::Wildcard),
            hir::DefaultPatternViewV1::Variant { variant, fields } => {
                let owner =
                    self.materialize_imported_default_type(variant.owner_type(), context)?;
                let fields = fields
                    .iter()
                    .map(|field| {
                        Ok((
                            field.declaration_index(),
                            self.materialize_imported_default_pattern(field.pattern(), context)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, ImportedDefaultMaterializationError>>()?;
                Ok(hir::Pattern::Variant {
                    application: hir::EnumVariantApplication {
                        owner,
                        variant: variant.declaration(),
                    },
                    fields,
                })
            }
            hir::DefaultPatternViewV1::Tuple { elements } => elements
                .iter()
                .map(|element| self.materialize_imported_default_pattern(element, context))
                .collect::<Result<Vec<_>, _>>()
                .map(hir::Pattern::Tuple),
            hir::DefaultPatternViewV1::Struct { owner_type, fields } => {
                let owner = self.materialize_imported_default_type(owner_type, context)?;
                let fields = fields
                    .iter()
                    .map(|field| {
                        Ok((
                            field.declaration_index(),
                            self.materialize_imported_default_pattern(field.pattern(), context)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, ImportedDefaultMaterializationError>>()?;
                Ok(hir::Pattern::Struct { owner, fields })
            }
            hir::DefaultPatternViewV1::Literal {
                value,
                equality,
                subject_type,
            } => {
                let subject_ty = self.materialize_imported_default_type(subject_type, context)?;
                let equality = match equality {
                    hir::DefaultLiteralEqualityV1::Char => hir::LiteralPatternEquality::Char,
                    hir::DefaultLiteralEqualityV1::Float { kind } => {
                        hir::LiteralPatternEquality::Float { kind: *kind }
                    }
                    hir::DefaultLiteralEqualityV1::Integer { kind } => {
                        hir::LiteralPatternEquality::Integer {
                            kind: (*kind).into(),
                        }
                    }
                    hir::DefaultLiteralEqualityV1::Ordinary { target } => {
                        hir::LiteralPatternEquality::Ordinary {
                            equals: self.materialize_imported_callable_target(
                                target,
                                MemberCallKind::Ordinary,
                                context,
                            )?,
                        }
                    }
                };
                let value = self.materialize_imported_default_expression(value, context)?;
                Ok(hir::Pattern::Literal {
                    value,
                    equality,
                    subject_ty,
                })
            }
        }
    }
}
