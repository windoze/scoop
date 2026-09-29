//! Intrinsics need the actual declaration signature, never a fabricated body.

use super::*;
use crate::imported_core::ImportedTypeBindings;
use scoop_identity::SignatureTypeKey;

pub(in crate::expr) struct ImportedIntrinsicSignature {
    pub(super) declaration: hir::ImportedCallableDeclaration,
    pub(super) signature: ImportedInferenceSignature,
    pub(super) bindings: ImportedTypeBindings,
}

impl ImportedIntrinsicSignature {
    pub(super) fn prepare(
        state: &mut Lowerer,
        declaration: hir::ImportedCallableDeclaration,
    ) -> Result<Self, String> {
        let interface = declaration.interface();
        let nominal = match interface.owner() {
            hir::PublicDeclarationOwnerV1::Nominal(owner) => Some(
                state
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.nominal_declaration(owner))
                    .cloned()
                    .ok_or("the dependency intrinsic owner is missing")?,
            ),
            _ => None,
        };
        let source = declaration.definition_origin();
        let location = declaration
            .definition_source(source)
            .ok_or("the dependency intrinsic source is missing")?;
        let origin = state
            .import_dependency_definition_origin(source, location)
            .map_err(|error| error.to_string())?;
        let own = interface.type_parameters().binders();
        let owners = nominal.as_ref().map_or(&[][..], |nominal| {
            nominal.interface.type_parameters().binders()
        });
        let binders = owners.iter().chain(own).collect::<Vec<_>>();
        let keys = (0..owners.len())
            .map(|index| SignatureTypeKey::Binder {
                depth: u32::from(!own.is_empty()),
                index: index as u32,
            })
            .chain((0..own.len()).map(|index| SignatureTypeKey::Binder {
                depth: 0,
                index: index as u32,
            }))
            .collect::<Vec<_>>();
        let (mut parameters, bindings) =
            state.prepare_imported_type_parameters(&binders, &keys, origin.span)?;
        let type_parameters = parameters.split_off(owners.len());
        let receiver = match interface.owner() {
            hir::PublicDeclarationOwnerV1::Nominal(owner) => Some(
                state
                    .imported_nominal_application(
                        owner,
                        keys.iter()
                            .take(owners.len())
                            .map(|key| bindings[key])
                            .collect(),
                    )
                    .map_err(|error| format!("cannot resolve intrinsic receiver: {error:?}"))?,
            ),
            _ => interface
                .receiver()
                .map(|receiver| state.imported_generic_type(receiver, &bindings))
                .transpose()?,
        };
        let mut value_parameters = Vec::new();
        if let Some(receiver) = receiver {
            value_parameters.push(("this".to_owned(), receiver));
        }
        for parameter in interface.parameters().parameters() {
            value_parameters.push((
                parameter.name().as_str().to_owned(),
                state.imported_generic_type(parameter.value_type(), &bindings)?,
            ));
        }
        let return_type = state.imported_generic_type(interface.result(), &bindings)?;
        Ok(Self {
            declaration,
            signature: ImportedInferenceSignature {
                owner_parameters: parameters,
                type_parameters,
                parameters: value_parameters,
                receiver,
                return_type,
                origin,
                span: origin.span,
            },
            bindings,
        })
    }
}
