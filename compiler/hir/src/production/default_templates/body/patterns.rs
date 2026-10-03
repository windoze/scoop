//! Recursive source-pattern projection.

use crate::{
    DefaultLiteralEqualityV1, DefaultPatternFieldV1, DefaultPatternV1, LiteralPatternEquality,
    Pattern,
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
            } => DefaultPatternV1::try_literal(
                self.expression(value)?,
                self.literal_equality(*equality)?,
                self.type_key(*subject_ty)?,
            )
            .map_err(super::super::DefaultBodyProjectionError::Pattern),
            Pattern::Variant {
                application,
                fields,
            } => DefaultPatternV1::try_variant(
                self.entities.variant(*application, self.binders)?,
                self.pattern_fields(fields)?,
            )
            .map_err(super::super::DefaultBodyProjectionError::Pattern),
            Pattern::Tuple(elements) => {
                let elements = elements
                    .iter()
                    .map(|element| self.pattern(element))
                    .collect::<Result<Vec<_>, _>>()?;
                DefaultPatternV1::try_tuple(elements)
                    .map_err(super::super::DefaultBodyProjectionError::Pattern)
            }
            Pattern::Struct { owner, fields } => {
                DefaultPatternV1::try_struct(self.type_key(*owner)?, self.pattern_fields(fields)?)
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
            LiteralPatternEquality::Integer { kind } => {
                DefaultLiteralEqualityV1::Integer { kind: kind.into() }
            }
            LiteralPatternEquality::Ordinary { equals } => DefaultLiteralEqualityV1::Ordinary {
                target: self.callable_target(equals)?,
            },
        })
    }
}
