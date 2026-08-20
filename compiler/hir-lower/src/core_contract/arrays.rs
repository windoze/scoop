use super::*;
use crate::types::ArrayKind;

impl Lowerer {
    pub(crate) fn validate_array_conversion_intrinsics(&mut self, files: &[ast::SourceFile]) {
        for kind in hir::ArrayIntrinsic::ALL {
            let intrinsic = hir::IntrinsicFunctionKind::Array(kind);
            if let Some(function) = self.require_intrinsic(intrinsic, files) {
                self.validate_array_conversion_intrinsic(function, kind);
            }
        }
    }

    fn validate_array_conversion_intrinsic(
        &mut self,
        function: hir::FunctionId,
        kind: hir::ArrayIntrinsic,
    ) {
        self.current_file = self.function_files[&function];
        let signature = &self.signatures[&function];
        let (owner_kind, result_kind, source_name) = match kind {
            hir::ArrayIntrinsic::ImmutableLength => {
                (ArrayKind::Immutable, None, "Array.arrayLength")
            }
            hir::ArrayIntrinsic::MutableLength => {
                (ArrayKind::Mutable, None, "MutableArray.arrayLength")
            }
            hir::ArrayIntrinsic::ToImmutable => (
                ArrayKind::Mutable,
                Some(ArrayKind::Immutable),
                "MutableArray.toArray",
            ),
            hir::ArrayIntrinsic::ToMutable => (
                ArrayKind::Immutable,
                Some(ArrayKind::Mutable),
                "Array.toMutableArray",
            ),
        };
        let owner = self.array_class(owner_kind);
        let result = result_kind.map(|kind| self.array_class(kind));
        let result_matches = match self.types[signature.return_ty] {
            Type::Integer(hir::IntegerKind::SIGNED_64) if result.is_none() => true,
            Type::Class(application) if result.is_some() => {
                let application = &self.class_applications[application];
                application.template
                    == self
                        .nominal_identity(crate::Owner::Class(
                            result.expect("a clone returns an array"),
                        ))
                        .declaration_id()
                    && matches!(application.arguments.as_slice(), [argument] if self.is_type_param(*argument, 0))
            }
            _ => false,
        };
        let valid = self.function_owner.get(&function) == Some(&Owner::Class(owner))
            && self.functions[function].name.ends_with(source_name)
            && !signature.is_suspend
            && signature.owner_type_param_count == 1
            && signature.type_params.len() == 1
            && signature.type_params[0].kind() == hir::TypeParamKind::Any
            && signature.params.is_empty()
            && result_matches;
        if !valid {
            self.error(
                self.functions[function].span,
                format!("malformed core array intrinsic `{}`", kind.name()),
            );
        }
    }
}
