use super::*;
use hir::ImportedCallableSource;
use scoop_identity::{LocalValueSelector, NominalDeclarationOwner, SignatureTypeKey};

impl Lowerer {
    pub(super) fn prepare_imported_constructor(
        &mut self,
        source: hir::ImportedCallableDeclaration,
        declaration: PersistentConstructorId,
    ) -> Result<PreparedImportedConstructor, String> {
        let hir::PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::GenericTemplate(owner)) =
            source.interface().owner()
        else {
            return Err("dependency constructor template requires a generic nominal owner".into());
        };
        let dependencies = self
            .dependencies
            .as_ref()
            .expect("imported constructors have a dependency catalog");
        let nominal = dependencies
            .nominal_declaration(hir::SourceNominalId::GenericTemplate(owner))
            .cloned()
            .ok_or("constructor owner is missing")?;
        let initialization = dependencies
            .nominal_initialization(owner)
            .ok_or("constructor owner has no initialization template")?;
        let constructor = initialization
            .constructors()
            .iter()
            .position(|c| source_constructor(c.declaration()) == Some(declaration))
            .ok_or("constructor has no initialization template")?;
        let executable = &initialization.constructors()[constructor];
        let definition = source
            .definition_source(executable.definition_origin())
            .ok_or("constructor definition source is missing")?;
        let origin = self
            .import_dependency_definition_origin(executable.definition_origin(), definition)
            .map_err(|e| e.to_string())?;
        let span = origin.span;
        let mut bindings = ImportedTypeBindings::new();
        let binders = nominal.interface.type_parameters().binders();
        let ids = binders
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let id = self.fresh_type_param(index);
                let ty = self.intern_type(hir::Type::Param(id));
                bindings.insert(
                    SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    },
                    ty,
                );
                id
            })
            .collect::<Vec<_>>();
        let mut type_parameters = Vec::new();
        for (binder, id) in binders.iter().zip(ids) {
            let bounds = match binder.bounds() {
                hir::TypeParameterBoundsV1::Unconstrained => hir::TypeParamBounds::Unconstrained,
                hir::TypeParameterBoundsV1::Value => hir::TypeParamBounds::Value { span },
                hir::TypeParameterBoundsV1::Ref => hir::TypeParamBounds::Ref { span },
                hir::TypeParameterBoundsV1::Nominal(bounds) => {
                    self.imported_generic_nominal_bounds(bounds, &bindings, span)?
                }
            };
            type_parameters.push(hir::TypeParamDecl {
                id,
                name: binder.name().as_str().to_owned(),
                bounds,
                span,
            });
        }
        let owner = self.imported_generic_type(executable.declaration().owner_type(), &bindings)?;
        let names = source
            .source_interface()
            .ok_or("constructor source parameters are missing")?
            .parameters()
            .parameters();
        let mut parameters = Vec::new();
        for (index, parameter) in names.iter().enumerate() {
            let local = executable
                .inputs()
                .get(&LocalValueSelector::Parameter {
                    declaration_index: index as u32,
                })
                .expect("validated constructor inputs contain every parameter");
            let hir::TemplateLocalDefinitionV1::Source(definition) = local.definition() else {
                return Err("source constructor parameter is missing its definition".into());
            };
            let location = source
                .definition_source(definition)
                .ok_or("constructor parameter definition source is missing")?;
            let definition = self
                .import_dependency_definition_origin(definition, location)
                .map_err(|e| e.to_string())?;
            parameters.push(hir::ConstructorParameter {
                id: hir::ConstructorParamId::from_raw(index as u32),
                binding: self.fresh_binding(),
                definition,
                name: parameter.name().as_str().to_owned(),
                ty: self.imported_generic_type(local.value_type(), &bindings)?,
            });
        }
        let (no_gc_type_params, gc_free_pointee_requirements) =
            self.imported_template_predicates(executable.predicates(), &bindings)?;
        let evaluation_context = if matches!(self.types[owner], hir::Type::Class(_)) {
            let key = scoop_identity::SourceContextKey::Callable {
                source: executable.definition_origin().origin().source().clone(),
                owner: scoop_identity::CallableOwner::Constructor(declaration),
            };
            let key = source
                .source_context(&key)
                .ok_or("constructor execution context is missing")?
                .clone();
            self.intern_imported_source_context(key)
        } else {
            origin.context
        };
        let signature = hir::ImportedConstructorSignature {
            declaration,
            name: source.name().to_owned(),
            type_parameters,
            owner,
            parameters,
            no_gc_type_params,
            gc_free_pointee_requirements,
            effects: executable.effects(),
            origin,
            evaluation_context,
        };
        Ok(PreparedImportedConstructor {
            signature,
            source,
            initialization,
            constructor,
            bindings,
            kind: None,
        })
    }
}
