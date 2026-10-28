use super::*;

impl Lowerer {
    pub(crate) fn validate_compile_time_input_intrinsics(&mut self) {
        let Some(&(function, _)) = self
            .intrinsic_functions
            .get(&hir::IntrinsicFunctionKind::CompileEnv)
        else {
            return;
        };
        self.current_file = self.function_files[&function];
        let declaration = &self.functions[function];
        let signature = &self.signatures[&function];
        let valid = declaration.method.is_none()
            && matches!(declaration.genericity, hir::FunctionGenericity::Plain)
            && !signature.is_suspend
            && matches!(signature.params.as_slice(), [parameter]
                if parameter.ty == self.string && matches!(parameter.calling, crate::model::FnParamCalling::Required))
            && self.as_option(signature.return_ty) == Some(self.string)
            && signature.attributes == hir::FunctionAttributes::default();
        if !valid {
            self.error(declaration.span,
                "intrinsic `compile_env` must be an ordinary top-level `fun env(name: String): Option<String>`".into());
        }
    }
}
