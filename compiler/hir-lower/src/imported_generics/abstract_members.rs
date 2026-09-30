use super::*;
use scoop_identity::{LocalValueSelector, SignatureTypeKey};

impl Lowerer {
    pub(super) fn prepare_imported_abstract_member(
        &mut self,
        declaration: hir::ImportedCallableDeclaration,
        identity: hir::ImportedCallableTemplateOrigin,
    ) -> Result<PreparedImportedGeneric, String> {
        let hir::ImportedCallableTemplateOrigin::Nominal {
            owner,
            modifier: hir::MethodModifier::Abstract,
            ..
        } = identity
        else {
            return Err("an abstract dependency callable must retain its nominal owner".into());
        };
        let nominal = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .cloned()
            .ok_or("abstract member owner is missing")?;
        let source = PreparedImportedCallableSource::Declaration(Box::new(declaration.clone()));
        let origin = self.import_generic_definition(&source, declaration.definition_origin())?;
        let span = origin.span;
        let binders = nominal
            .interface
            .type_parameters()
            .binders()
            .iter()
            .collect::<Vec<_>>();
        let signatures = (0..binders.len())
            .map(|index| SignatureTypeKey::Binder {
                depth: 0,
                index: index as u32,
            })
            .collect::<Vec<_>>();
        let (type_parameters, bindings) =
            self.prepare_imported_type_parameters(&binders, &signatures, span)?;
        let arguments = signatures
            .iter()
            .map(|signature| bindings[signature])
            .collect();
        let receiver = self
            .imported_nominal_application(owner, arguments)
            .map_err(|error| format!("cannot resolve abstract member owner: {error:?}"))?;
        let mut locals = Arena::new();
        let this = locals.alloc(hir::Local {
            binding: self.fresh_binding(),
            selector: LocalValueSelector::This,
            definition: hir::LocalValueDefinitionSite::Source(origin),
            name: "this".to_owned(),
            ty: receiver,
            mutable: false,
        });
        let mut parameters = vec![hir::Param {
            name: "this".to_owned(),
            ty: receiver,
            local: this,
        }];
        for (index, parameter) in declaration
            .interface()
            .parameters()
            .parameters()
            .iter()
            .enumerate()
        {
            let ty = self.imported_generic_type(parameter.value_type(), &bindings)?;
            let definition = match declaration.source_interface() {
                Some(interface) => {
                    let parameter = interface
                        .parameters()
                        .parameters()
                        .get(index)
                        .ok_or("abstract member parameter is missing its source declaration")?;
                    self.import_generic_definition(&source, parameter.definition_origin())?
                }
                // The implicit setter parameter belongs to its accessor declaration.
                None => origin,
            };
            let name = parameter.name().as_str().to_owned();
            let local = locals.alloc(hir::Local {
                binding: self.fresh_binding(),
                selector: LocalValueSelector::Parameter {
                    declaration_index: index as u32,
                },
                definition: hir::LocalValueDefinitionSite::Source(definition),
                name: name.clone(),
                ty,
                mutable: false,
            });
            parameters.push(hir::Param { name, ty, local });
        }
        let return_type =
            self.imported_generic_type(declaration.interface().result(), &bindings)?;
        let member_name = match declaration.interface().declaration() {
            scoop_identity::CallableTemplateOrigin::Accessor(accessor) => {
                let property = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.property_for_accessor(accessor))
                    .ok_or("abstract accessor property is missing")?;
                let role = if property.accessors().getter() == accessor {
                    "get"
                } else {
                    "set"
                };
                format!("${role}${}", declaration.name())
            }
            _ => declaration.name().to_owned(),
        };
        Ok(PreparedImportedGeneric {
            signature: hir::ImportedGenericCallableSignature {
                declaration: identity,
                name: format!("{}.{member_name}", nominal.name()),
                type_parameters: hir::ImportedCallableTypeParameters::Declared(type_parameters),
                no_gc_type_params: Vec::new(),
                gc_free_pointee_requirements: Vec::new(),
                parameters,
                return_type,
                receiver: Some(receiver),
                effects: declaration.interface().effects(),
                origin,
                span,
            },
            source,
            bindings,
            locals,
            statements: None,
        })
    }
}
