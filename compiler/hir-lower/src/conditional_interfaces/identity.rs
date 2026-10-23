use super::*;
use crate::persistent_types::{
    SignatureBinder, SignatureTypeMapper, signature_parameter_conditions,
};

impl Lowerer {
    pub(super) fn conditional_interface_key(
        &mut self,
        owner: Owner,
        interface: TypeId,
        parameters: &[hir::TypeParamDecl],
        span: ast::Span,
    ) -> Option<hir::ConditionalInterfaceKey> {
        let core_types = match &self.core {
            crate::CoreLoweringAuthority::Defined => hir::HirCoreTypeIdentityAuthority::Defined(
                self.defined_intrinsic_type_core.as_ref()?,
            ),
            crate::CoreLoweringAuthority::Imported(protocols) => {
                hir::HirCoreTypeIdentityAuthority::Imported(protocols.fundamental_types())
            }
        };
        let mapper = SignatureTypeMapper::new(
            self,
            self.nominal_identities
                .as_ref()
                .expect("implementation binding follows nominal declaration identities"),
            core_types,
        );
        let binders = parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| SignatureBinder {
                parameter: parameter.id,
                depth: 0,
                index: u32::try_from(index).expect("owner parameter counts fit u32"),
            })
            .collect::<Vec<_>>();
        let key = (|| {
            let interface = mapper.map(interface, &binders)?;
            let conditions = signature_parameter_conditions(parameters, &binders, |ty| {
                mapper.map(ty, &binders)
            })?;
            Ok::<_, crate::persistent_types::SignatureTypeMappingError>(
                hir::ConditionalInterfaceKey {
                    owner: self.nominal_identity(owner).declaration_id(),
                    interface,
                    conditions,
                },
            )
        })();
        match key {
            Ok(key) => Some(key),
            Err(error) => {
                self.error(
                    span,
                    format!("invalid conditional interface declaration: {error}"),
                );
                None
            }
        }
    }
}
