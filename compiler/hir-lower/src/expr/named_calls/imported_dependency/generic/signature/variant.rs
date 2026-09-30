//! Variant inference uses the enum declaration without manufacturing a body.

use super::*;
use crate::imported_core::ImportedTypeBindings;
use scoop_identity::SignatureTypeKey;

pub(in crate::expr) struct ImportedVariantSignature {
    pub(super) declaration: hir::ImportedCallableDeclaration,
    pub(super) signature: LoadedCallableSignature,
    pub(super) bindings: ImportedTypeBindings,
}

impl ImportedVariantSignature {
    pub(super) fn prepare(
        state: &mut Lowerer,
        declaration: hir::ImportedCallableDeclaration,
    ) -> Result<Self, String> {
        let interface = declaration.interface();
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = interface.owner() else {
            return Err("a dependency variant requires its enum declaration".into());
        };
        let nominal = state
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .cloned()
            .ok_or("the dependency variant owner is missing")?;
        let source = declaration.definition_origin();
        let location = declaration
            .definition_source(source)
            .ok_or("the dependency variant source is missing")?;
        let origin = state
            .import_dependency_definition_origin(source, location)
            .map_err(|error| error.to_string())?;
        let binders = nominal
            .interface
            .type_parameters()
            .binders()
            .iter()
            .collect::<Vec<_>>();
        let keys = (0..binders.len())
            .map(|index| SignatureTypeKey::Binder {
                depth: 0,
                index: index as u32,
            })
            .collect::<Vec<_>>();
        let (type_parameters, bindings) =
            state.prepare_imported_type_parameters(&binders, &keys, origin.span)?;
        let parameters = interface
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| {
                Ok((
                    parameter.name().as_str().to_owned(),
                    state.imported_generic_type(parameter.value_type(), &bindings)?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let return_type = state.imported_generic_type(interface.result(), &bindings)?;
        let value_parameters =
            state.imported_parameter_views(&declaration, parameters.iter().map(|(_, ty)| *ty));
        Ok(Self {
            declaration,
            signature: LoadedCallableSignature {
                signature: crate::call_resolution::candidates::DeclarationSignature {
                    owner_parameters: type_parameters,
                    callable_parameters: Vec::new(),
                    value_parameters,
                    return_type,
                },
                receiver: None,
                origin,
                span: origin.span,
            },
            bindings,
        })
    }
}
