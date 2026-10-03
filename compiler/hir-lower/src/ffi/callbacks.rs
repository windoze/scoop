//! C address support for actual parameter-free source implementations.

use super::*;

impl Lowerer {
    pub(crate) fn prepare_native_callback_signatures(
        &mut self,
        producer: scoop_identity::ConeIdentity,
    ) -> Vec<hir::NativeCallbackSignature> {
        let mut signatures = Vec::new();
        for function in self.top_level.clone() {
            let declaration = &self.functions[function];
            if !self.source_function_declarations.contains_key(&function)
                || self.intrinsic_sources[self.function_files[&function]]
                    .identity
                    .cone()
                    != producer
                || declaration.method.is_some()
                || self.extension_receivers.contains_key(&function)
                || declaration.type_param_count() != 0
                || declaration.is_suspend
                || declaration.attributes.gc_effect != hir::GcEffect::NoGc
                || !matches!(declaration.kind, hir::FunctionKind::User(_))
            {
                continue;
            }
            let parameters = declaration
                .params
                .iter()
                .map(|parameter| parameter.ty)
                .collect::<Vec<_>>();
            let result = declaration.return_ty;
            let safe = parameters
                .iter()
                .copied()
                .map(|ty| (ty, false))
                .chain(std::iter::once((result, true)))
                .all(|(ty, allow_unit)| {
                    matches!(
                        self.classify_c_ffi_type(
                            ty,
                            &[],
                            allow_unit,
                            Vec::new(),
                            &mut HashSet::new()
                        ),
                        Ok(Classification::Safe)
                    )
                });
            if !safe {
                continue;
            }
            let ty = self.intern_function_type(false, parameters, result);
            let hir::Type::Function(signature) = self.types[ty] else {
                unreachable!("a callback signature is a function type");
            };
            self.intern_type(hir::Type::FunPtr(signature));
            signatures.push(hir::NativeCallbackSignature {
                function,
                signature,
            });
        }
        signatures
    }
}
