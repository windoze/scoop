//! Existing aggregate expressions; no source constructor is evaluated here.

use super::*;
use crate::imports::lookup::{
    LookupResult,
    values::{ValueOrigin, ValueTarget},
};

impl Lowerer {
    pub(super) fn const_name_is_variant(
        &self,
        lookup: &LookupResult<ValueOrigin>,
        expected: Option<hir::TypeId>,
        name: &str,
    ) -> bool {
        match lookup {
            LookupResult::Unique(ValueOrigin::Dependency(binding)) => {
                matches!(binding.target(), hir::ImportedTarget::EnumVariant(_))
            }
            LookupResult::Unique(origin) => matches!(
                self.materialized_value_target(origin),
                Some(ValueTarget::Variant(_))
            ),
            LookupResult::Missing => {
                expected.is_some_and(|ty| self.named_enum_variant(ty, name).is_some())
            }
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_const_tuple(
        &mut self,
        elements: &[ast::Expr],
        span: ast::Span,
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
    ) -> Option<EvaluatedConst> {
        let types = match expected.map(|ty| self.types[ty].clone()) {
            Some(hir::Type::Tuple(types)) => {
                if types.len() != elements.len() {
                    self.error(
                        span,
                        format!(
                            "constant tuple has {} elements, expected {}",
                            elements.len(),
                            types.len()
                        ),
                    );
                    return None;
                }
                Some(types)
            }
            _ => None,
        };
        let mut values = Vec::with_capacity(elements.len());
        let mut actual = Vec::with_capacity(elements.len());
        for (index, element) in elements.iter().enumerate() {
            let expected = types.as_ref().map(|types| types[index]);
            let value = self.evaluate_const_expression(
                element,
                expected,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )?;
            if let Some(expected) = expected
                && !self.types_equal(value.ty, expected)
            {
                self.error(
                    element.span(),
                    format!(
                        "constant tuple element must be of type {}, found {}",
                        self.type_name(expected),
                        self.type_name(value.ty)
                    ),
                );
                return None;
            }
            values.push(value.value);
            actual.push(value.ty);
        }
        let ty = self.intern_type(hir::Type::Tuple(actual));
        Some(EvaluatedConst {
            value: hir::HirConstantImage::Tuple(values),
            ty,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_const_field(
        &mut self,
        access: &ast::FieldAccess,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
    ) -> Option<EvaluatedConst> {
        let receiver = self.evaluate_const_expression(
            &access.receiver,
            None,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        let (field, ty) = self.resolve_field(receiver.ty, &access.selector)?;
        let value = self
            .constant_field(receiver.value, receiver.ty, field)
            .expect("a checked aggregate has its declared fields");
        Some(EvaluatedConst { value, ty })
    }
}
