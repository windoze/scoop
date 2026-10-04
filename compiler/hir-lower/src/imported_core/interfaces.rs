//! Complete dependency interface signatures in provider declaration order.

use super::*;
use hir::ImportedCallableSource;
use std::sync::Arc;

impl Lowerer {
    pub(super) fn imported_interface_type(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
        arguments: Vec<hir::TypeId>,
    ) -> Result<hir::TypeId, ImportedSignatureTypeError> {
        let owner = declaration.owner();
        if !self.loaded_interface_definitions.contains_key(&owner) {
            self.load_interface_definition(declaration)?;
        }
        let application = self.intern_interface_application(owner, arguments);
        Ok(self.interface_applications[application].canonical_type)
    }

    fn load_interface_definition(
        &mut self,
        declaration: Arc<hir::ImportedNominalDeclaration>,
    ) -> Result<(), ImportedSignatureTypeError> {
        let hir::NominalDispatchOrderV1::Interface { parents, members } =
            declaration.interface.declaration_details().dispatch_order()
        else {
            return Err(ImportedSignatureTypeError::Structural);
        };
        let owner = declaration.owner();
        let source_span = declaration.origin.origin().span();
        let span = Span {
            start: u32::try_from(source_span.start_byte())
                .map_err(|_| ImportedSignatureTypeError::Structural)?,
            end: u32::try_from(source_span.end_byte())
                .map_err(|_| ImportedSignatureTypeError::Structural)?,
        };
        let mut bindings = ImportedTypeBindings::new();
        let mut type_params = Vec::new();
        let mut arguments = Vec::new();
        for (index, binder) in declaration
            .interface
            .type_parameters()
            .binders()
            .iter()
            .enumerate()
        {
            let id = self.fresh_type_param(index);
            let ty = self.intern_type(hir::Type::Param(id));
            bindings.insert(
                SignatureTypeKey::Binder {
                    depth: 0,
                    index: index as u32,
                },
                ty,
            );
            arguments.push(ty);
            type_params.push(hir::TypeParamDecl {
                id,
                name: binder.name().as_str().to_owned(),
                bounds: hir::TypeParamBounds::Unconstrained,
                span,
            });
        }
        let self_application = self.intern_interface_application(owner, arguments);
        self.loaded_interface_definitions.insert(
            owner,
            hir::LoadedInterfaceDefinition {
                declaration: Arc::clone(&declaration),
                definition: hir::InterfaceDefinition {
                    gc_free_pointee_requirements: Self::decoded_nominal_pointee_requirements(
                        &declaration,
                        &type_params,
                    ),
                    self_application,
                    type_params: type_params.clone(),
                    parents: Vec::new(),
                },
                methods: Vec::new(),
            },
        );
        for (parameter, binder) in type_params
            .iter_mut()
            .zip(declaration.interface.type_parameters().binders())
        {
            *parameter = self
                .resolve_imported_type_parameter(binder, parameter.id, &bindings, span)
                .map_err(|_| ImportedSignatureTypeError::Structural)?;
        }
        let mut parent_types = Vec::new();
        let mut methods = Vec::<hir::LoadedInterfaceMethod>::new();
        for parent in parents {
            let parent_ty = self.imported_signature_type_with_bindings(parent, &bindings)?;
            let hir::Type::Interface(application) = self.types[parent_ty] else {
                return Err(ImportedSignatureTypeError::Structural);
            };
            let application = self.interface_applications[application].clone();
            let parent_methods = self.loaded_interface_definitions[&application.template]
                .methods
                .clone();
            for mut method in parent_methods {
                if !methods
                    .iter()
                    .any(|existing| existing.slot.id() == method.slot.id())
                {
                    for (_, ty) in &mut method.parameters {
                        *ty = self.instantiate_ty(*ty, &application.arguments);
                    }
                    for ty in &mut method.context_parameters {
                        *ty = self.instantiate_ty(*ty, &application.arguments);
                    }
                    method.return_type =
                        self.instantiate_ty(method.return_type, &application.arguments);
                    methods.push(method);
                }
            }
            parent_types.push(parent_ty);
        }
        for member in members {
            methods.retain(|method| !member.overrides().values().contains(&method.slot.id()));
            let candidate = self
                .dependencies
                .as_ref()
                .and_then(|dependencies| {
                    dependencies
                        .callable_for_slot(declaration.owner(), member.slot())
                        .ok()
                        .flatten()
                })
                .ok_or(ImportedSignatureTypeError::Structural)?;
            let callable = candidate.interface();
            let source = candidate.source_interface();
            // Dispatch signatures use the complete typed parameters. Generated
            // and abstract accessors have no separate named-call source protocol.
            let parameters = callable
                .parameters()
                .parameters()
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let name = source
                        .and_then(|source| source.parameters().parameters().get(index))
                        .map(|parameter| parameter.name().as_str().to_owned())
                        .unwrap_or_else(|| format!("$parameter.{index}"));
                    Ok((
                        name,
                        self.imported_signature_type_with_bindings(
                            parameter.value_type(),
                            &bindings,
                        )?,
                    ))
                })
                .collect::<Result<Vec<_>, ImportedSignatureTypeError>>()?;
            let return_type =
                self.imported_signature_type_with_bindings(callable.result(), &bindings)?;
            let origin = candidate.definition_origin().origin().span();
            methods.push(hir::LoadedInterfaceMethod {
                slot: declaration
                    .dispatch_slots
                    .iter()
                    .find(|slot| slot.id() == member.slot())
                    .ok_or(ImportedSignatureTypeError::Structural)?
                    .clone(),
                overrides: member.overrides().values().to_vec(),
                declaration: callable.clone(),
                name: candidate.name().to_owned(),
                parameters,
                context_parameters: callable
                    .context_parameters()
                    .iter()
                    .map(|parameter| {
                        self.imported_signature_type_with_bindings(
                            parameter.value_type(),
                            &bindings,
                        )
                    })
                    .collect::<Result<_, _>>()?,
                return_type,
                span: Span {
                    start: u32::try_from(origin.start_byte())
                        .map_err(|_| ImportedSignatureTypeError::Structural)?,
                    end: u32::try_from(origin.end_byte())
                        .map_err(|_| ImportedSignatureTypeError::Structural)?,
                },
            });
        }
        let suppressed = methods
            .iter()
            .flat_map(|method| &method.overrides)
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        methods.retain(|method| !suppressed.contains(&method.slot.id()));
        let loaded = self
            .loaded_interface_definitions
            .get_mut(&owner)
            .expect("the interface builder registered its identity");
        loaded.definition.type_params = type_params;
        loaded.definition.parents = parent_types;
        loaded.methods = methods;
        Ok(())
    }
}
