//! Recursive source-pattern projection.

use crate::{
    CanonicalBooleanV1, CanonicalConstValueV1, DefaultLiteralEqualityV1, DefaultPatternFieldV1,
    DefaultPatternV1, ExprKind, LiteralPatternEquality, Pattern,
};

use super::BodyProjection;

impl BodyProjection<'_, '_> {
    pub(super) fn pattern(
        &mut self,
        pattern: &Pattern,
    ) -> Result<DefaultPatternV1, super::super::DefaultBodyProjectionError> {
        match pattern {
            Pattern::Binding { local } => Ok(DefaultPatternV1::binding(self.local(*local)?)),
            Pattern::Wildcard => Ok(DefaultPatternV1::wildcard()),
            Pattern::Literal {
                value,
                equality,
                subject_ty,
            } => Ok(DefaultPatternV1::literal(
                literal_value(value)?,
                self.literal_equality(*equality)?,
                self.type_key(*subject_ty)?,
            )),
            Pattern::Variant {
                application,
                variant,
                fields,
            } => {
                let variant = crate::AppliedEnumVariantRef::checked_index(
                    &self.entities.export().enums,
                    &self.entities.export().enum_applications,
                    *application,
                    *variant,
                )
                .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                    kind: "enum variant",
                    index: *variant,
                })?;
                DefaultPatternV1::try_variant(
                    self.entities.variant(variant, self.binders)?,
                    self.pattern_fields(fields)?,
                )
                .map_err(super::super::DefaultBodyProjectionError::Pattern)
            }
            Pattern::Tuple(elements) => {
                let elements = elements
                    .iter()
                    .map(|element| self.pattern(element))
                    .collect::<Result<Vec<_>, _>>()?;
                DefaultPatternV1::try_tuple(elements)
                    .map_err(super::super::DefaultBodyProjectionError::Pattern)
            }
            Pattern::Struct {
                application,
                fields,
            } => {
                let application = super::super::arena_get(
                    &self.entities.export().struct_applications,
                    *application,
                )
                .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                    kind: "struct application",
                    index: super::super::raw_index(*application),
                })?;
                DefaultPatternV1::try_struct(
                    self.type_key(application.canonical_type)?,
                    self.pattern_fields(fields)?,
                )
                .map_err(super::super::DefaultBodyProjectionError::Pattern)
            }
        }
    }

    fn pattern_fields(
        &mut self,
        fields: &[(u32, Pattern)],
    ) -> Result<Vec<DefaultPatternFieldV1>, super::super::DefaultBodyProjectionError> {
        fields
            .iter()
            .map(|(index, pattern)| {
                self.pattern(pattern)
                    .map(|pattern| DefaultPatternFieldV1::new(*index, pattern))
            })
            .collect()
    }

    fn literal_equality(
        &self,
        equality: LiteralPatternEquality,
    ) -> Result<DefaultLiteralEqualityV1, super::super::DefaultBodyProjectionError> {
        Ok(match equality {
            LiteralPatternEquality::Integer { kind, target } => DefaultLiteralEqualityV1::Integer {
                kind: kind.into(),
                target: self
                    .entities
                    .callable(crate::Callable::Function(target.function()), self.binders)?,
            },
            LiteralPatternEquality::Ordinary { equals } => DefaultLiteralEqualityV1::Ordinary {
                target: self.entities.callable(equals, self.binders)?,
            },
        })
    }
}

fn literal_value(
    expression: &crate::Expr,
) -> Result<CanonicalConstValueV1, super::super::DefaultBodyProjectionError> {
    match &expression.kind {
        ExprKind::StringLiteral { value, .. } => Ok(CanonicalConstValueV1::String(value.clone())),
        ExprKind::IntegerLiteral(value) => Ok(CanonicalConstValueV1::Integer((*value).into())),
        ExprKind::BoolLiteral(value) => Ok(CanonicalConstValueV1::Boolean(
            CanonicalBooleanV1::from(*value),
        )),
        _ => Err(super::super::DefaultBodyProjectionError::InvalidLiteralPattern),
    }
}
