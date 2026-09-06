use super::*;

impl Lowerer {
    pub(crate) fn validate_foreign_callback_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::ForeignCallbackCore> {
        // The exception-core validator owns the missing-Throwable diagnostic.
        // Callback failure typing cannot be validated until that prerequisite
        // exists, so do not manufacture a second error or unwrap incomplete
        // upstream state here.
        let (_, throwable) = self.throwable?;
        let callback = self.ffi_foreign_callback?;
        let mode = self.require_core_enum("ForeignCallbackMode", files)?;
        let state = self.require_core_enum("ForeignCallbackState", files)?;
        let register =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackRegister, files);
        let retain =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackRetain, files);
        let release =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackRelease, files);
        let query_state =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackState, files);
        let failure =
            self.require_intrinsic(hir::IntrinsicFunctionKind::ForeignCallbackFailure, files);

        self.current_file = self.struct_files[&callback];
        let callback_decl = &self.structs[callback];
        let callback_valid = callback_decl.type_params.len() == 1
            && callback_decl.type_params[0].kind() == hir::TypeParamKind::Any
            && callback_decl.interfaces.is_empty()
            && callback_decl.attributes.c_layout.is_none()
            && !callback_decl.attributes.interior_mutable
            && matches!(callback_decl.semantic_fields(), [function, context]
                if function.name == "function"
                    && matches!(self.types[function.ty], hir::Type::Struct(application)
                        if self.struct_applications[application].template
                            == self.ffi_fun_ptr.expect("FunPtr core exists")
                            && matches!(self.struct_applications[application].arguments.as_slice(), [arg] if self.is_type_param(*arg, 0)))
                    && context.name == "context"
                    && matches!(self.types[context.ty], hir::Type::Ptr(pointee) if pointee == self.unit));
        if !callback_valid {
            self.error(
                callback_decl.span,
                "core `ForeignCallback<F>` must contain `function: FunPtr<F>` and `context: Ptr<Unit>`"
                    .to_string(),
            );
        }

        self.validate_unit_enum(mode, &["Reusable", "OneShot"]);
        self.validate_unit_enum(state, &["Registered", "Active", "Completed", "Failed"]);

        let mode_application = self.enums[mode].self_application;
        let reusable = hir::AppliedEnumVariantRef::checked_index(
            &self.enums,
            &self.enum_applications,
            mode_application,
            0,
        );
        let one_shot = hir::AppliedEnumVariantRef::checked_index(
            &self.enums,
            &self.enum_applications,
            mode_application,
            1,
        );
        let modes = reusable.zip(one_shot).and_then(|(reusable, one_shot)| {
            hir::ForeignCallbackModes::checked(
                &self.enums,
                &self.enum_applications,
                reusable,
                one_shot,
            )
        });

        let state_application = self.enums[state].self_application;
        let states = [0, 1, 2, 3].map(|index| {
            hir::AppliedEnumVariantRef::checked_index(
                &self.enums,
                &self.enum_applications,
                state_application,
                index,
            )
        });
        let states = match states {
            [
                Some(registered),
                Some(active),
                Some(completed),
                Some(failed),
            ] => hir::ForeignCallbackStates::checked(
                &self.enums,
                &self.enum_applications,
                registered,
                active,
                completed,
                failed,
            ),
            _ => None,
        };

        for (id, operation) in [
            (register, "register"),
            (retain, "retain"),
            (release, "release"),
            (query_state, "state"),
            (failure, "failure"),
        ] {
            let Some(id) = id else { continue };
            self.current_file = self.function_files[&id];
            let function = &self.functions[id];
            let signature = &self.signatures[&id];
            let common = !signature.is_suspend
                && signature.type_params.len() == 1
                && signature.type_params[0].kind() == hir::TypeParamKind::Any
                && signature.attributes.safety == hir::Safety::Unsafe
                && signature.attributes.gc_effect == hir::GcEffect::Managed
                && function.method.is_none();
            let callback_param = |ty| {
                matches!(self.types[ty], hir::Type::Struct(application)
                    if self.struct_applications[application].template == callback
                        && matches!(self.struct_applications[application].arguments.as_slice(), [arg] if self.is_type_param(*arg, 0)))
            };
            let signature_valid = match operation {
                "register" => {
                    matches!(signature.params.as_slice(), [closure, index, mode_param]
                        if closure.ty == self.any
                            && index.ty == self.integer_type(hir::IntegerKind::SIGNED_64)
                            && mode_param.ty == self.interned_enum_type(mode))
                        && callback_param(signature.return_ty)
                }
                "retain" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty) && callback_param(signature.return_ty)),
                "release" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty) && signature.return_ty == self.unit),
                "state" => matches!(signature.params.as_slice(), [param]
                    if callback_param(param.ty)
                        && signature.return_ty == self.interned_enum_type(state)),
                "failure" => {
                    matches!(signature.params.as_slice(), [param]
                        if callback_param(param.ty)
                            && matches!(self.types[signature.return_ty], hir::Type::Enum(application)
                                if self.enum_applications[application].template
                                    == self
                                        .option_enumeration()
                                        .expect("Option core exists")
                                    && self.enum_applications[application].arguments.as_slice()
                                        == [throwable]))
                }
                _ => unreachable!(),
            };
            if !common || !signature_valid {
                self.error(
                    function.span,
                    format!(
                        "intrinsic `foreign_callback_{operation}` has an invalid core signature"
                    ),
                );
            }
        }

        let failure_id = failure?;
        let failure_application = match self.types[self.signatures[&failure_id].return_ty] {
            hir::Type::Enum(application) => application,
            _ => return None,
        };
        let option = self.option_core?;
        let failure_some = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            failure_application,
            option.some(),
        );
        let failure_none = hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            failure_application,
            option.none(),
        );
        let failure_result = failure_some
            .and_then(|some| {
                hir::AppliedEnumVariantFieldRef::checked(
                    &self.enums,
                    &self.enum_applications,
                    some,
                    option.some_payload().local_index(),
                )
            })
            .zip(failure_none)
            .and_then(|(some_payload, none)| {
                hir::ForeignCallbackFailureResult::checked(
                    &self.enums,
                    &self.enum_applications,
                    option,
                    throwable,
                    some_payload,
                    none,
                )
            });

        Some(hir::ForeignCallbackCore {
            callback,
            modes: modes?,
            states: states?,
            failure_result: failure_result?,
            register: register?,
            retain: retain?,
            release: release?,
            query_state: query_state?,
            failure: failure_id,
        })
    }
}
