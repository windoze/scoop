//! Constructor identities use the same original-definition requests as bodies.

use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn resolve_class_constructor(&mut self, index: usize) -> CallableMaterialization {
        if let Some(materialization) = self.class_constructor_materializations[index] {
            return materialization;
        }
        let (definition, class) = self.concretizer.class_constructor_definitions[index];
        let arguments = self.concretizer.classes[class].type_arguments.clone();
        let materialization = self.class_definition_materialization(definition, class, &arguments);
        self.class_constructor_materializations[index] = Some(materialization);
        materialization
    }

    pub(super) fn resolve_struct_constructor(&mut self, index: usize) -> CallableMaterialization {
        if let Some(materialization) = self.struct_constructor_materializations[index] {
            return materialization;
        }
        let (definition, structure) = self.concretizer.struct_constructor_definitions[index];
        let arguments = self.concretizer.structs[structure].type_arguments.clone();
        let materialization =
            self.struct_definition_materialization(definition, structure, &arguments);
        self.struct_constructor_materializations[index] = Some(materialization);
        materialization
    }

    fn class_definition_materialization(
        &mut self,
        definition: export::ClassConstructorDefinition,
        class: concrete::ClassId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let (identity, source) = self.concretizer.class_constructor_origin(definition);
        let template = match identity {
            export::DefaultClassConstructorIdV1::Source(id) => {
                CallableTemplateOwner::Constructor(id)
            }
            export::DefaultClassConstructorIdV1::Generated(id) => {
                CallableTemplateOwner::Generated(id)
            }
        };
        self.constructor_materialization(
            template,
            source,
            self.concretizer.class_type[&class],
            arguments,
        )
    }

    fn struct_definition_materialization(
        &mut self,
        definition: export::StructConstructorDefinition,
        structure: concrete::StructId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let origin = self.concretizer.struct_constructor_origin(definition);
        self.constructor_materialization(
            CallableTemplateOwner::Constructor(origin),
            origin,
            self.concretizer.struct_type[&structure],
            arguments,
        )
    }

    pub(super) fn constructor_materialization(
        &mut self,
        template: CallableTemplateOwner,
        source: scoop_identity::PersistentConstructorId,
        owner: concrete::TypeId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let owner = self.exact_types[owner].id();
        CallableMaterialization::new(
            template,
            self.constructor_application_context(source, owner, arguments),
        )
    }

    pub(super) fn constructor_declaration_materialization(
        &mut self,
        constructor: scoop_identity::PersistentConstructorId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let class = self.concretizer.classes.iter().find(|(id, class)| {
            class.type_arguments == arguments
                && self
                    .class_constructor_declarations(*id)
                    .contains(&constructor)
        });
        let owner = if let Some((class, _)) = class {
            self.concretizer.class_type[&class]
        } else {
            let source = self.concretizer.source;
            let (structure, _) = self
                .concretizer
                .structs
                .iter()
                .find(|(_, structure)| {
                    if structure.type_arguments != arguments {
                        return false;
                    }
                    let origin = structure.origin.declaration_id();
                    match source.nominal_identities.struct_id(origin) {
                        Some(id) => source.structs[id]
                            .constructors
                            .iter()
                            .any(|id| source.constructor_identities[*id].id() == constructor),
                        None => source.loaded_struct_definitions[&origin]
                            .declaration
                            .interface
                            .declaration_details()
                            .constructors()
                            .values()
                            .contains(&constructor),
                    }
                })
                .expect("a lexical constructor retains its original nominal declaration");
            self.concretizer.struct_type[&structure]
        };
        self.constructor_materialization(
            CallableTemplateOwner::Constructor(constructor),
            constructor,
            owner,
            arguments,
        )
    }

    pub(super) fn class_constructor_materialization(
        &mut self,
        constructor: export::ClassConstructorId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let declaration = &self.concretizer.source.class_constructors[constructor];
        let origin = self.concretizer.source.nominal_identities[declaration.owner].declaration_id();
        let owner = self.concretizer.class_by_key[&(origin, arguments.to_vec())];
        self.class_definition_materialization(
            export::ClassConstructorDefinition::Local(constructor),
            owner,
            arguments,
        )
    }

    pub(super) fn struct_constructor_materialization(
        &mut self,
        constructor: export::StructConstructorId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let declaration = &self.concretizer.source.struct_constructors[constructor];
        let origin = self.concretizer.source.nominal_identities[declaration.owner].declaration_id();
        let owner = self.concretizer.struct_by_key[&(origin, arguments.to_vec())];
        self.struct_definition_materialization(
            export::StructConstructorDefinition::Local(constructor),
            owner,
            arguments,
        )
    }
}
