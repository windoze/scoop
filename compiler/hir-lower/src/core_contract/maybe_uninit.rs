use super::*;

impl Lowerer {
    pub(crate) fn validate_maybe_uninit_intrinsics(&mut self) {
        for kind in hir::MaybeUninitIntrinsic::ALL {
            let Some(&(function, _)) = self
                .intrinsic_functions
                .get(&hir::IntrinsicFunctionKind::MaybeUninit(kind))
            else {
                continue;
            };
            if !self.maybe_uninit_signature_matches(function, kind) {
                self.current_file = self.function_files[&function];
                self.error(
                    self.functions[function].span,
                    format!("malformed core MaybeUninit intrinsic `{}`", kind.name()),
                );
            }
        }
    }

    fn maybe_uninit_signature_matches(
        &self,
        function: FunctionId,
        kind: hir::MaybeUninitIntrinsic,
    ) -> bool {
        let Some(&owner) = self.function_owner.get(&function) else {
            return false;
        };
        let host = match (kind, owner) {
            (hir::MaybeUninitIntrinsic::AssumeInit, Owner::Struct(host)) => {
                Some(Owner::Struct(host))
            }
            (
                hir::MaybeUninitIntrinsic::Uninit | hir::MaybeUninitIntrinsic::Initialized,
                Owner::Object(object),
            ) => self.companion_host(object),
            _ => None,
        };
        let Some(Owner::Struct(host)) = host else {
            return false;
        };
        if self.structs[host].representation
            != hir::StructRepresentation::Intrinsic(hir::IntrinsicTypeKind::MaybeUninit)
        {
            return false;
        }
        let signature = &self.signatures[&function];
        if signature.is_suspend
            || !signature.context_parameters.is_empty()
            || signature.owner_type_param_count != 1
            || signature.type_params.len() != 1
            || signature.modifiers != hir::CallableModifiers::default()
            || self.functions[function].name.rsplit('.').next() != Some(kind.source_name())
        {
            return false;
        }
        let payload = match kind {
            hir::MaybeUninitIntrinsic::AssumeInit => signature.return_ty,
            _ => match self.maybe_uninit_value_type(signature.return_ty) {
                Some(value) => value,
                None => return false,
            },
        };
        if self.types[payload] != Type::Param(signature.type_params[0].id) {
            return false;
        }
        match kind {
            hir::MaybeUninitIntrinsic::Uninit | hir::MaybeUninitIntrinsic::AssumeInit => {
                signature.params.is_empty()
            }
            hir::MaybeUninitIntrinsic::Initialized => {
                matches!(signature.params.as_slice(), [parameter]
                if parameter.ty == payload && parameter.name.text == "value"
                && matches!(parameter.calling, crate::FnParamCalling::Required))
            }
        }
    }
}
