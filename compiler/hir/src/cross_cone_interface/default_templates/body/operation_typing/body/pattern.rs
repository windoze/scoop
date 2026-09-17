use crate::{
    CanonicalBooleanV1, CanonicalConstValueV1, DefaultLiteralEqualityV1,
    DefaultOperationCoreTypeV1, DefaultOperationEntityV1, DefaultOperationExpectedTypeShapeV1,
    DefaultOperationIntrinsicV1, DefaultOperationTypingSemanticAuthority,
    DefaultOperationValueRoleV1, DefaultPatternV1, DefaultPatternViewV1,
};

use super::{
    BodyNode, BodyValidator, BodyWork, DefaultBodyOperationTypingProblemV1, DefaultBodyOperationV1,
    DefaultPatternOperationV1, ExportDefaultBodyOperationTypingValidationError,
};

impl<A, E> BodyValidator<'_, A, E>
where
    A: DefaultOperationTypingSemanticAuthority<E>,
{
    pub(super) fn process_pattern<'body>(
        &mut self,
        pattern: &'body DefaultPatternV1,
        subject: &scoop_identity::SignatureTypeKey,
        depth: u64,
        pending: &mut Vec<BodyWork<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        match pattern.view() {
            DefaultPatternViewV1::Binding { local } => {
                let site = Self::site(
                    DefaultBodyOperationV1::Pattern(DefaultPatternOperationV1::Binding),
                    DefaultOperationValueRoleV1::Local,
                );
                let record = self.local_record(local, site)?;
                self.expect_type(record.value_type(), subject, site)?;
                self.expect_mutability(record.mutable(), CanonicalBooleanV1::False, site)
            }
            DefaultPatternViewV1::Wildcard => Ok(()),
            DefaultPatternViewV1::Literal {
                value,
                equality,
                subject_type,
            } => {
                let operation = DefaultBodyOperationV1::Pattern(DefaultPatternOperationV1::Literal);
                let subject_site = Self::site(operation, DefaultOperationValueRoleV1::Subject);
                let literal_type = match value {
                    CanonicalConstValueV1::Integer(value) => self.core_type(
                        DefaultOperationCoreTypeV1::Integer(value.kind().into()),
                        subject_site,
                    )?,
                    CanonicalConstValueV1::Boolean(_) => {
                        self.core_type(DefaultOperationCoreTypeV1::Boolean, subject_site)?
                    }
                    CanonicalConstValueV1::String(_) => {
                        self.core_type(DefaultOperationCoreTypeV1::String, subject_site)?
                    }
                };
                self.expect_type(subject_type, &literal_type, subject_site)?;
                self.expect_type(subject, subject_type, subject_site)?;
                if let DefaultLiteralEqualityV1::Integer { kind, .. } = equality {
                    let equality_type = self.core_type(
                        DefaultOperationCoreTypeV1::Integer(*kind),
                        Self::site(operation, DefaultOperationValueRoleV1::Callable),
                    )?;
                    self.expect_type(
                        &equality_type,
                        subject_type,
                        Self::site(operation, DefaultOperationValueRoleV1::Callable),
                    )?;
                }
                self.validate_intrinsic(
                    DefaultOperationIntrinsicV1::LiteralEquality {
                        equality,
                        subject_type,
                    },
                    Self::site(operation, DefaultOperationValueRoleV1::Callable),
                )
            }
            DefaultPatternViewV1::Tuple { elements } => {
                let operation = DefaultBodyOperationV1::Pattern(DefaultPatternOperationV1::Tuple);
                let scoop_identity::SignatureTypeKey::Tuple(subject_elements) = subject else {
                    return Err(ExportDefaultBodyOperationTypingValidationError::TypeShape {
                        site: Self::site(operation, DefaultOperationValueRoleV1::Subject),
                        expected: DefaultOperationExpectedTypeShapeV1::Tuple,
                        actual: Box::new(subject.clone()),
                    });
                };
                self.expect_arity(
                    elements.len(),
                    subject_elements.as_slice().len(),
                    Self::site(operation, DefaultOperationValueRoleV1::Subject),
                )?;
                for (element, element_type) in
                    elements.iter().zip(subject_elements.as_slice()).rev()
                {
                    self.push_node(
                        pending,
                        BodyNode::Pattern {
                            pattern: element,
                            subject: element_type.clone(),
                        },
                        depth,
                    )?;
                }
                Ok(())
            }
            DefaultPatternViewV1::Variant { variant, fields } => {
                let operation = DefaultBodyOperationV1::Pattern(DefaultPatternOperationV1::Variant);
                let site = Self::site(operation, DefaultOperationValueRoleV1::Target);
                let shape =
                    self.aggregate_shape(DefaultOperationEntityV1::Variant(variant), site)?;
                self.expect_type(variant.owner_type(), shape.owner_type(), site)?;
                self.expect_type(subject, shape.owner_type(), site)?;
                for field in fields.iter().rev() {
                    let index = field.declaration_index() as usize;
                    let Some(field_type) = shape.fields().get(index) else {
                        return self.problem(
                            Self::site(operation, DefaultOperationValueRoleV1::Field { index }),
                            DefaultBodyOperationTypingProblemV1::IndexOutOfBounds {
                                index: field.declaration_index(),
                                length: shape.fields().len(),
                            },
                        );
                    };
                    self.push_node(
                        pending,
                        BodyNode::Pattern {
                            pattern: field.pattern(),
                            subject: field_type.clone(),
                        },
                        depth,
                    )?;
                }
                Ok(())
            }
            DefaultPatternViewV1::Struct { owner_type, fields } => {
                let operation = DefaultBodyOperationV1::Pattern(DefaultPatternOperationV1::Struct);
                let site = Self::site(operation, DefaultOperationValueRoleV1::Target);
                let shape =
                    self.aggregate_shape(DefaultOperationEntityV1::Struct(owner_type), site)?;
                self.expect_type(owner_type, shape.owner_type(), site)?;
                self.expect_type(subject, shape.owner_type(), site)?;
                for field in fields.iter().rev() {
                    let index = field.declaration_index() as usize;
                    let Some(field_type) = shape.fields().get(index) else {
                        return self.problem(
                            Self::site(operation, DefaultOperationValueRoleV1::Field { index }),
                            DefaultBodyOperationTypingProblemV1::IndexOutOfBounds {
                                index: field.declaration_index(),
                                length: shape.fields().len(),
                            },
                        );
                    };
                    self.push_node(
                        pending,
                        BodyNode::Pattern {
                            pattern: field.pattern(),
                            subject: field_type.clone(),
                        },
                        depth,
                    )?;
                }
                Ok(())
            }
        }
    }
}
