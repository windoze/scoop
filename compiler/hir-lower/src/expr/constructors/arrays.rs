//! Construction-time conversions between immutable and mutable arrays.

use super::*;

impl Lowerer {
    pub(in crate::expr) fn lower_array_conversion(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        target_class: hir::ClassId,
        target_kind: ArrayKind,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        debug_assert_eq!(self.array_class_kind(target_class), Some(target_kind));
        let source_kind = match target_kind {
            ArrayKind::Immutable => ArrayKind::Mutable,
            ArrayKind::Mutable => ArrayKind::Immutable,
        };
        let mut view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Class(target_class),
        );
        let [owner_parameter] = view.owner_parameters.as_slice() else {
            unreachable!("the intrinsic array contract declares one type parameter")
        };
        let parameter_ty = self.intern_type(Type::Param(owner_parameter.id));
        let source_ty = self.class_application(self.array_class(source_kind), vec![parameter_ty]);
        view.value_parameters = vec![crate::call_resolution::candidates::ValueParameter {
            name: "source".to_string(),
            calling: crate::defaults::SourceParameterCalling::Required,
            ty: source_ty,
        }];

        let argument_map =
            match crate::call_resolution::arguments::CandidateArgumentMap::source_nominal(
                &view, &call.args,
            ) {
                Ok(argument_map) => argument_map,
                Err(failure) => {
                    self.diagnose_nominal_shape_failure(&view, call.span, failure.describe());
                    return None;
                }
            };
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        if !explicit_type_args.is_empty() && explicit_type_args.len() != view.owner_parameters.len()
        {
            self.diagnose_nominal_shape_failure(
                &view,
                call.span,
                format!(
                    "expects {} explicit type argument(s), but {} were supplied",
                    view.owner_parameters.len(),
                    explicit_type_args.len()
                ),
            );
            return None;
        }
        let expected_arguments = expected.and_then(|expected| {
            self.array_type_info(expected)
                .filter(|array| array.kind == target_kind)
                .map(|array| vec![array.element])
        });
        let inferred = self.lower_nominal_arguments(NominalArgumentInput {
            view: &view,
            argument_map: &argument_map,
            expressions: &call.args,
            explicit_type_args: &explicit_type_args,
            expected_type_args: expected_arguments.as_deref(),
            span: call.span,
        })?;
        let NominalArguments {
            args,
            argument_sinks,
            type_args,
        } = inferred;
        let [element_ty] = type_args.as_slice() else {
            unreachable!("the solved array conversion has one concrete type argument")
        };
        let [arg] = args.as_slice() else {
            unreachable!("the solved array conversion has one typed source argument")
        };
        let expected_source = self.array_type(source_kind, *element_ty);
        debug_assert!(self.types_equal(arg.ty, expected_source));
        let ty = self.array_type(target_kind, *element_ty);
        let mut args = self.materialize_nominal_arguments(
            crate::argument_materialization::NominalArgumentMaterialization {
                view: &view,
                argument_map: &argument_map,
                type_args: &type_args,
                source_args: args,
                argument_sinks,
                call_span: call.span,
            },
            sink,
        );
        Some(hir::Expr {
            kind: ExprKind::ArrayClone(Box::new(
                args.pop()
                    .expect("the solved array conversion has one typed source argument"),
            )),
            ty,
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }
}
