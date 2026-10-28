//! Materialize already evaluated values without invoking source constructors.

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn constant_expression(
        &mut self,
        value: hir::HirConstantImage,
        ty: hir::TypeId,
        owner: hir::StringConstantOwner<hir::PropertyId>,
        span: Span,
        origin: hir::ExpressionOrigin,
    ) -> hir::Expr {
        self.constant_expression_inner(value, ty, owner, span, origin, &mut 0)
    }

    #[allow(clippy::too_many_arguments)]
    fn constant_expression_inner(
        &mut self,
        value: hir::HirConstantImage,
        ty: hir::TypeId,
        owner: hir::StringConstantOwner<hir::PropertyId>,
        span: Span,
        origin: hir::ExpressionOrigin,
        next_string: &mut u32,
    ) -> hir::Expr {
        use hir::{ExprKind as E, HirConstantImage as C};
        let kind = match value {
            C::Unit => E::UnitLiteral,
            C::Integer(value) => E::IntegerLiteral(value),
            C::Float(value) => E::FloatLiteral(value),
            C::Char(value) => E::CharLiteral(value),
            C::Boolean(value) => E::BoolLiteral(value),
            C::String(value) => {
                let ordinal = *next_string;
                *next_string = ordinal
                    .checked_add(1)
                    .expect("constant string ordinal fits u32");
                let owner = match owner {
                    hir::StringConstantOwner::Property(property) if ordinal != 0 => {
                        hir::StringConstantOwner::PropertyElement { property, ordinal }
                    }
                    owner => owner,
                };
                E::StringLiteral { value, owner }
            }
            C::Tuple(elements) => {
                let hir::Type::Tuple(types) = self.types[ty].clone() else {
                    unreachable!("tuple constants retain their complete tuple type")
                };
                assert_eq!(elements.len(), types.len());
                E::TupleLiteral(
                    elements
                        .into_iter()
                        .zip(types)
                        .map(|(element, ty)| {
                            self.constant_expression_inner(
                                element,
                                ty,
                                owner,
                                span,
                                origin,
                                next_string,
                            )
                        })
                        .collect(),
                )
            }
            C::Struct {
                application,
                fields,
            } => {
                let types = self
                    .struct_fields(ty)
                    .expect("struct constant has its declared fields");
                assert_eq!(fields.len(), types.fields.len());
                E::StructConstruct {
                    application,
                    fields: fields
                        .into_iter()
                        .zip(types.fields)
                        .map(|(field, definition)| {
                            self.constant_expression_inner(
                                field,
                                definition.ty,
                                owner,
                                span,
                                origin,
                                next_string,
                            )
                        })
                        .collect(),
                }
            }
            C::Enum { variant, fields } => {
                let types = self.enum_variant(variant).fields;
                assert_eq!(fields.len(), types.len());
                E::VariantConstruct {
                    variant,
                    args: fields
                        .into_iter()
                        .zip(types)
                        .map(|(field, (_, ty))| {
                            self.constant_expression_inner(
                                field,
                                ty,
                                owner,
                                span,
                                origin,
                                next_string,
                            )
                        })
                        .collect(),
                }
            }
            C::NullPointer(_) => unreachable!("raw null storage is not a source constant value"),
        };
        hir::Expr {
            kind,
            ty,
            span,
            origin,
        }
    }

    pub(crate) fn expression_constant(
        &mut self,
        expression: &hir::Expr,
    ) -> Option<hir::HirConstantImage> {
        use hir::{ExprKind as E, HirConstantImage as C};
        Some(match &expression.kind {
            E::UnitLiteral => C::Unit,
            E::IntegerLiteral(value) => C::Integer(*value),
            E::FloatLiteral(value) => C::Float(*value),
            E::CharLiteral(value) => C::Char(*value),
            E::BoolLiteral(value) => C::Boolean(*value),
            E::StringLiteral { value, .. } => C::String(value.clone()),
            E::TupleLiteral(elements) => C::Tuple(
                elements
                    .iter()
                    .map(|element| self.expression_constant(element))
                    .collect::<Option<_>>()?,
            ),
            E::StructConstruct {
                application,
                fields,
            } => C::Struct {
                application: *application,
                fields: fields
                    .iter()
                    .map(|field| self.expression_constant(field))
                    .collect::<Option<_>>()?,
            },
            E::VariantConstruct { variant, args } => C::Enum {
                variant: *variant,
                fields: args
                    .iter()
                    .map(|field| self.expression_constant(field))
                    .collect::<Option<_>>()?,
            },
            E::FieldAccess { receiver, field } => {
                let value = self.expression_constant(receiver)?;
                self.constant_field(value, receiver.ty, *field)?
            }
            _ => return None,
        })
    }

    pub(crate) fn constant_field(
        &mut self,
        value: hir::HirConstantImage,
        ty: hir::TypeId,
        field: hir::FieldRef,
    ) -> Option<hir::HirConstantImage> {
        let (elements, index) = match (value, field) {
            (hir::HirConstantImage::Tuple(elements), hir::FieldRef::TupleIndex(index)) => {
                (elements, index as usize)
            }
            (hir::HirConstantImage::Struct { fields, .. }, hir::FieldRef::StructField { .. }) => {
                let definition = self.struct_fields(ty)?;
                let index = definition
                    .fields
                    .iter()
                    .position(|candidate| candidate.reference == field)?;
                (fields, index)
            }
            _ => return None,
        };
        elements.into_iter().nth(index)
    }
}
