use super::super::*;

impl Harness {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::tests) fn class(
        &mut self,
        name: &str,
        modifier: hir::ClassModifier,
        constructor: &[(&str, hir::TypeId)],
        base: Option<(hir::ClassId, Vec<hir::Expr>)>,
        interfaces: &[hir::InterfaceId],
    ) -> hir::ClassId {
        let interfaces: Vec<_> = interfaces
            .iter()
            .map(|&interface| self.interface_ty(interface))
            .collect();
        let base = base.map(|(base, arguments)| (self.class_ty(base), arguments));
        self.declare_class(name, modifier, constructor, base, interfaces)
    }

    pub(in crate::tests) fn declare_class(
        &mut self,
        name: &str,
        modifier: hir::ClassModifier,
        constructor: &[(&str, hir::TypeId)],
        base_class: Option<(hir::TypeId, Vec<hir::Expr>)>,
        interfaces: Vec<hir::TypeId>,
    ) -> hir::ClassId {
        let mut interface_implementations = base_class
            .as_ref()
            .map(|(base, _)| {
                let hir::Type::Class(application) = self.types[*base] else {
                    panic!("test harness class bases are class applications")
                };
                let base = self.class_applications[application].template;
                self.classes[base].interface_implementations.clone()
            })
            .unwrap_or_default();
        for implementation in self.interface_implementation_shells(&interfaces) {
            if let Some(existing) = interface_implementations
                .iter_mut()
                .find(|existing| existing.interface == implementation.interface)
            {
                *existing = implementation;
            } else {
                interface_implementations.push(implementation);
            }
        }
        let self_application =
            hir::ClassApplicationId::from_raw((self.class_applications.len() as u32).into());
        let class = self.classes.alloc(hir::ClassDecl {
            modifier,
            name: name.to_string(),
            self_application,
            type_params: Vec::new(),
            representation: hir::ClassRepresentation::Declared(
                constructor
                    .iter()
                    .enumerate()
                    .map(|(index, (name, ty))| hir::ConstructorField {
                        parameter: hir::ConstructorParamId::from_raw(index as u32),
                        name: name.to_string(),
                        ty: *ty,
                        mutable: false,
                    })
                    .collect(),
            ),
            base_class,
            interfaces,
            interface_implementations,
            methods: Vec::new(),
            span: SPAN,
        });
        let actual = self.class_application(class, Vec::new());
        assert_eq!(actual, self_application);
        class
    }

    /// A concrete zero-argument exception shell used by tests that exercise
    /// compiler-generated exception edges.
    pub(in crate::tests) fn exception(&mut self, name: &str) -> hir::ClassId {
        self.class(name, hir::ClassModifier::Final, &[], None, &[])
    }

    pub(in crate::tests) fn exception_target(
        &mut self,
        name: &str,
        include: bool,
    ) -> hir::CompilerException {
        let existing = self
            .classes
            .iter()
            .find_map(|(id, declaration)| (declaration.name == name).then_some(id));
        let class = if let Some(existing) = existing {
            existing
        } else if include {
            self.exception(name)
        } else {
            self.class(
                &format!("${name}Protocol"),
                // LocalConcreteHir's exception contract always includes a
                // real constructor callable. The protocol shell remains
                // hidden from unrelated dump assertions by the test helper.
                hir::ClassModifier::Final,
                &[],
                None,
                &[],
            )
        };
        hir::CompilerException {
            constructor: hir::ZeroArgClassConstructor { class },
        }
    }

    pub(in crate::tests) fn test_exception_core(
        &mut self,
        include: bool,
    ) -> hir::CompilerExceptionCore {
        hir::CompilerExceptionCore {
            throwable: self.exception_target("Throwable", include),
            unwrap_exception: self.exception_target("UnwrapException", include),
            class_cast_exception: self.exception_target("ClassCastException", include),
            arithmetic_exception: self.exception_target("ArithmeticException", include),
            index_out_of_bounds_exception: self
                .exception_target("IndexOutOfBoundsException", include),
            illegal_state_exception: self.exception_target("IllegalStateException", include),
        }
    }
}
