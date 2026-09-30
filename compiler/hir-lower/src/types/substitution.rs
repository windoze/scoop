use super::*;

impl Lowerer {
    pub(crate) fn type_contains_param(&self, ty: TypeId) -> bool {
        if let Some(application) = self.nominal_application(ty) {
            return application
                .arguments
                .iter()
                .any(|argument| self.type_contains_param(*argument));
        }
        match &self.types[ty] {
            Type::Param(_) => true,
            Type::Ptr(element) => self.type_contains_param(*element),
            Type::Tuple(args) => args.iter().any(|ty| self.type_contains_param(*ty)),
            Type::Function(id) | Type::FunPtr(id) => self.function_type_contains_param(*id),
            _ => false,
        }
    }

    pub(crate) fn function_type_contains_param(&self, id: hir::FunctionTypeId) -> bool {
        let function = &self.function_types[id];
        function
            .parameter_types
            .iter()
            .any(|ty| self.type_contains_param(*ty))
            || self.type_contains_param(function.return_type)
    }

    /// Substitute bound type arguments for `Type::Param`, recursively.
    /// Used both for generic function instantiation (parameters index
    /// `Function::type_params`) and for enum variant field types
    /// (parameters index `EnumDecl::type_params`). Callers guarantee
    /// every parameter is bound (unbound parameters are diagnosed at
    /// the use site first).
    pub(crate) fn instantiate_ty(&mut self, ty: TypeId, type_args: &[TypeId]) -> TypeId {
        if let Some(application) = self.nominal_application(ty) {
            let arguments = application
                .arguments
                .iter()
                .map(|argument| self.instantiate_ty(*argument, type_args))
                .collect();
            return self
                .apply_nominal_type(application.template, arguments)
                .expect("substitution preserves a resolved nominal declaration");
        }
        match self.types[ty].clone() {
            Type::Param(index) => type_args[index.into_raw() as usize],
            Type::Tuple(elements) => {
                let mut substituted = Vec::with_capacity(elements.len());
                for element in elements {
                    substituted.push(self.instantiate_ty(element, type_args));
                }
                self.intern_type(Type::Tuple(substituted))
            }
            Type::Function(id) => self
                .instantiate_function_type(id, |this, ty| Some(this.instantiate_ty(ty, type_args)))
                .expect("complete type argument substitution"),
            Type::Ptr(pointee) => {
                let pointee = self.instantiate_ty(pointee, type_args);
                self.intern_type(Type::Ptr(pointee))
            }
            Type::FunPtr(id) => {
                let function = self
                    .instantiate_function_type(id, |this, ty| {
                        Some(this.instantiate_ty(ty, type_args))
                    })
                    .expect("complete function pointer substitution");
                let Type::Function(id) = self.types[function] else {
                    unreachable!("function type instantiation stays a function type")
                };
                self.intern_type(Type::FunPtr(id))
            }
            _ => ty,
        }
    }

    /// Apply an exact source-produced type-parameter relation while comparing
    /// method signatures. Both owner substitutions and callable-parameter
    /// alpha-renaming are explicit bindings; this routine never derives a
    /// split point from an index.
    pub(crate) fn instantiate_method_ty(
        &mut self,
        ty: TypeId,
        bindings: &[(hir::TypeParamId, TypeId)],
    ) -> TypeId {
        if let Some(application) = self.nominal_application(ty) {
            let arguments = application
                .arguments
                .iter()
                .map(|argument| self.instantiate_method_ty(*argument, bindings))
                .collect();
            return self
                .apply_nominal_type(application.template, arguments)
                .expect("substitution preserves a resolved nominal declaration");
        }
        match self.types[ty].clone() {
            Type::Param(parameter) => bindings
                .iter()
                .find_map(|(source, target)| (*source == parameter).then_some(*target))
                .expect("a complete method substitution binds every referenced parameter"),
            Type::Tuple(elements) => {
                let elements = elements
                    .into_iter()
                    .map(|element| self.instantiate_method_ty(element, bindings))
                    .collect();
                self.intern_type(Type::Tuple(elements))
            }
            Type::Function(id) => self
                .instantiate_function_type(id, |this, ty| {
                    Some(this.instantiate_method_ty(ty, bindings))
                })
                .expect("complete owner substitution"),
            Type::Ptr(pointee) => {
                let pointee = self.instantiate_method_ty(pointee, bindings);
                self.intern_type(Type::Ptr(pointee))
            }
            Type::FunPtr(id) => {
                let function = self
                    .instantiate_function_type(id, |this, ty| {
                        Some(this.instantiate_method_ty(ty, bindings))
                    })
                    .expect("complete function pointer owner substitution");
                let Type::Function(id) = self.types[function] else {
                    unreachable!("function type instantiation stays a function type")
                };
                self.intern_type(Type::FunPtr(id))
            }
            _ => ty,
        }
    }

    /// Best-effort substitution for expected-type hints: `None` when
    /// the type still mentions an unbound type parameter.
    pub(crate) fn try_substitute(
        &mut self,
        ty: TypeId,
        bindings: &[Option<TypeId>],
    ) -> Option<TypeId> {
        if let Some(application) = self.nominal_application(ty) {
            let arguments = application
                .arguments
                .iter()
                .map(|argument| self.try_substitute(*argument, bindings))
                .collect::<Option<Vec<_>>>()?;
            return self
                .apply_nominal_type(application.template, arguments)
                .ok();
        }
        match self.types[ty].clone() {
            Type::Param(index) => bindings.get(index.into_raw() as usize).copied().flatten(),
            Type::Tuple(elements) => {
                let mut substituted = Vec::with_capacity(elements.len());
                for element in elements {
                    substituted.push(self.try_substitute(element, bindings)?);
                }
                Some(self.intern_type(Type::Tuple(substituted)))
            }
            Type::Function(id) => {
                self.instantiate_function_type(id, |this, ty| this.try_substitute(ty, bindings))
            }
            Type::Ptr(pointee) => {
                let pointee = self.try_substitute(pointee, bindings)?;
                Some(self.intern_type(Type::Ptr(pointee)))
            }
            Type::FunPtr(id) => {
                let function = self
                    .instantiate_function_type(id, |this, ty| this.try_substitute(ty, bindings))?;
                let Type::Function(id) = self.types[function] else {
                    unreachable!("function type substitution stays a function type")
                };
                Some(self.intern_type(Type::FunPtr(id)))
            }
            _ => Some(ty),
        }
    }
}
