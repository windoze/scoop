use super::*;

mod targets;

impl SourceRoots {
    pub(super) fn default_source(
        &mut self,
        export: &ExportHir,
        source: ExportDefaultSourceId,
        index: &super::super::Index,
        roots: &mut Roots,
    ) -> Result<(), Error> {
        let source = &export.export_default_sources[source];
        for ty in source.type_arguments() {
            roots.require_field_type(export, index, *ty)?;
        }
        let Some((expression, _)) = source.declared() else {
            // Dependency declarations remain owned by their actual provider.
            return Ok(());
        };
        if !insert(&mut self.defaults, expression)? {
            return Ok(());
        }
        let template = &export.export_default_exprs[expression];
        let original = match template.definition_root {
            LexicalDefinitionRoot::Function(id) => ExportParameterOwner::Function(id),
            LexicalDefinitionRoot::StructConstructor(id) => {
                ExportParameterOwner::StructConstructor(id)
            }
            LexicalDefinitionRoot::ClassConstructor(id) => {
                ExportParameterOwner::ClassConstructor(id)
            }
            LexicalDefinitionRoot::VariantConstructor(id) => {
                ExportParameterOwner::VariantConstructor(id)
            }
        };
        self.callable(export, original, roots)?;
        let references = &template.references;
        for reference in &references.callables {
            self.default_callable(export, reference.target, index, roots)?;
        }
        for reference in &references.constructors {
            let (owner, ty) = match reference.target {
                ExportDefaultConstructorTarget::Imported { owner_type, .. } => {
                    roots.require_field_type(export, index, owner_type)?;
                    continue;
                }
                ExportDefaultConstructorTarget::Struct(id) => {
                    let application = &export.struct_constructor_applications[id];
                    (
                        ExportParameterOwner::StructConstructor(application.constructor),
                        export.struct_applications[application.owner].canonical_type,
                    )
                }
                ExportDefaultConstructorTarget::Class(id) => {
                    let application = &export.class_constructor_applications[id];
                    (
                        ExportParameterOwner::ClassConstructor(application.constructor),
                        export.class_applications[application.owner].canonical_type,
                    )
                }
                ExportDefaultConstructorTarget::Variant(variant) => {
                    let Some(declaration) = export
                        .enum_member_identities
                        .variant_declaration(variant.variant)
                    else {
                        roots.require_field_type(export, index, variant.owner)?;
                        continue;
                    };
                    (
                        ExportParameterOwner::VariantConstructor(declaration),
                        variant.owner,
                    )
                }
            };
            roots.require_field_type(export, index, ty)?;
            self.callable(export, owner, roots)?;
        }
        for reference in &references.types {
            match reference.target {
                ExportDefaultTypeTarget::Type(ty) => roots.require_field_type(export, index, ty)?,
                ExportDefaultTypeTarget::LocalFunctionSignature(id) => {
                    let signature = &export.function_types
                        [export.local_functions[id].declaration_function_type];
                    roots.require_field_type(export, index, signature.return_type)?;
                    for parameter in &signature.parameter_types {
                        roots.require_field_type(export, index, *parameter)?;
                    }
                }
            }
        }
        for reference in &references.globals {
            self.property(export, export.globals[reference.target].property, roots)?;
        }
        for reference in &references.fields {
            let ty = match reference.target {
                FieldRef::StructField { owner, .. } | FieldRef::ClassField { owner, .. } => owner,
                // Tuple fields have no source declaration. Their complete
                // receiver and element types are in the typed reference set.
                FieldRef::TupleIndex(_) => continue,
            };
            roots.require_field_type(export, index, ty)?;
        }
        Ok(())
    }
}
