//! Intrinsics need the actual declaration signature, never a fabricated body.

use super::*;
use crate::call_resolution::candidates::DeclarationSignature;
use crate::imported_core::ImportedTypeBindings;
use scoop_identity::SignatureTypeKey;

#[derive(Clone)]
pub(crate) struct LoadedCallableSignature {
    pub(crate) signature: DeclarationSignature<hir::ExportDefaultTemplateKeyV1>,
    pub(crate) receiver: Option<hir::TypeId>,
    pub(crate) origin: hir::DefinitionOrigin,
    pub(crate) span: hir::Span,
}

pub(crate) struct ImportedIntrinsicSignature {
    pub(crate) declaration: hir::ImportedCallableDeclaration,
    pub(crate) signature: LoadedCallableSignature,
    pub(crate) bindings: ImportedTypeBindings,
}

impl ImportedIntrinsicSignature {
    pub(crate) fn prepare(
        state: &mut Lowerer,
        declaration: hir::ImportedCallableDeclaration,
    ) -> Result<Self, String> {
        let interface = declaration.interface();
        if matches!(interface.effects().implementation(), hir::CallableImplementationV1::Intrinsic(kind) if kind.is_foreign_callback())
        {
            state.prepare_imported_foreign_callback_protocol()?;
        }
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
        for parameter in interface.parameters().parameters() {
            value_parameters.push((
                parameter.name().as_str().to_owned(),
                state.imported_generic_type(parameter.value_type(), &bindings)?,
            ));
        }
        let return_type = state.imported_generic_type(interface.result(), &bindings)?;
        let value_parameters = state
            .imported_parameter_views(&declaration, value_parameters.iter().map(|(_, ty)| *ty));
        Ok(Self {
            declaration,
            signature: LoadedCallableSignature {
                signature: crate::call_resolution::candidates::DeclarationSignature {
                    owner_parameters: parameters,
                    callable_parameters: type_parameters,
                    value_parameters,
                    return_type,
                },
                receiver,
                origin,
                span: origin.span,
            },
            bindings,
        })
    }
}

impl Lowerer {
    pub(super) fn prepare_imported_intrinsic(
        &mut self,
        declaration: hir::ImportedCallableDeclaration,
        identity: hir::ImportedCallableTemplateOrigin,
    ) -> Result<PreparedImportedGeneric, String> {
        let prepared = ImportedIntrinsicSignature::prepare(self, declaration)?;
        let signature = prepared.signature;
        let mut locals = Arena::new();
        let values = signature
            .receiver
            .map(|ty| ("this".to_owned(), ty))
            .into_iter()
            .chain(
                signature
                    .signature
                    .value_parameters
                    .iter()
                    .map(|p| (p.name.clone(), p.ty)),
            );
        let params = values
            .enumerate()
            .map(|(index, (name, ty))| {
                let selector = if signature.receiver.is_some() && index == 0 {
                    scoop_identity::LocalValueSelector::This
                } else {
                    scoop_identity::LocalValueSelector::Parameter {
                        declaration_index: (index - usize::from(signature.receiver.is_some()))
                            as u32,
                    }
                };
                let local = locals.alloc(hir::Local {
                    binding: self.fresh_binding(),
                    selector,
                    definition: hir::LocalValueDefinitionSite::Source(signature.origin),
                    name: name.clone(),
                    ty,
                    mutable: false,
                });
                hir::Param { name, ty, local }
            })
            .collect();
        let type_parameters = signature
            .signature
            .owner_parameters
            .into_iter()
            .chain(signature.signature.callable_parameters)
            .collect();
        Ok(PreparedImportedGeneric {
            signature: hir::ImportedGenericCallableSignature {
                declaration: identity,
                signature: hir::CallableSignature::from_source_effects(
                    prepared.declaration.name().to_owned(),
                    params,
                    signature.signature.return_type,
                    prepared.declaration.interface().effects(),
                    self.imported_release_callability(
                        &prepared.declaration.interface().effects(),
                        &prepared.bindings,
                    )?,
                    signature.span,
                ),
                type_parameters: hir::ImportedCallableTypeParameters::Declared(type_parameters),
                no_gc_type_params: Vec::new(),
                gc_free_pointee_requirements: Vec::new(),
                receiver: signature.receiver,
                origin: signature.origin,
            },
            source: PreparedImportedCallableSource::Declaration(Box::new(prepared.declaration)),
            bindings: prepared.bindings,
            locals,
            statements: None,
        })
    }
}
