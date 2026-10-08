use super::*;

impl Harness {
    pub(super) fn test_equality_core(&mut self) -> hir::EqualityCore {
        let parameter = hir::TypeParamDecl {
            id: hir::TypeParamId::with_substitution_slot(u32::MAX - 1, 0),
            name: "T".to_string(),
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        };
        let parameter_ty = self.types.alloc(hir::Type::Param(parameter.id));
        let interface =
            self.declare_interface("Equality", vec![parameter], vec![parameter_ty], Vec::new());
        self.add_interface_method_signature(
            interface,
            hir::MethodSig {
                name: "equals".to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: vec![param(
                    "other",
                    parameter_ty,
                    hir::LocalId::from_raw(1.into()),
                )],
                return_ty: self.boolean,
                span: SPAN,
            },
        );
        let equals = self.interfaces[interface].methods[0];
        self.functions[self.interface_methods[equals].function]
            .modifiers
            .operator = Some(hir::OperatorKind::Equals);
        hir::EqualityCore { interface, equals }
    }
}
