use super::*;
use hir::ImportedCallableSource;
use scoop_identity::SignatureTypeKey;

impl Lowerer {
    pub(super) fn has_imported_same_type_equals(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<bool, String> {
        let (declaration, arguments) = self
            .dependency_nominal_application(ty)
            .expect("an imported equality owner retains its declaration");
        let owner = declaration.owner();
        let bindings = arguments
            .iter()
            .enumerate()
            .map(|(index, &argument)| {
                (
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    argument,
                )
            })
            .collect();
        let candidates = self
            .dependencies
            .as_ref()
            .expect("an imported equality owner retains its provider")
            .member_callable_candidates(
                owner,
                hir::ImportedMemberLookup::Operator(hir::CallableOperatorRoleV1::Language(
                    hir::CallableOperatorV1::Equals,
                )),
            )
            .map_err(|error| error.to_string())?;
        for candidate in candidates {
            let [parameter] = candidate.interface().parameters().parameters() else {
                continue;
            };
            let parameter = self
                .imported_signature_type_with_bindings(parameter.value_type(), &bindings)
                .map_err(|error| error.diagnostic("equals parameter"))?;
            if self.types_equal(parameter, ty) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
