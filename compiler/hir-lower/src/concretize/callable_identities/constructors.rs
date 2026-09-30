//! Constructor identities distinguish lexical origins from emitted bodies.

use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn resolve_class_constructor(&mut self, index: usize) -> CallableMaterialization {
        if let Some(materialization) = self.class_constructor_materializations[index] {
            return materialization;
        }
        let (source, class) = self.concretizer.class_constructor_keys[index];
        let arguments = self.concretizer.classes[class].type_arguments.clone();
        let materialization = match source {
            constructor_work::ClassConstructorSource::Local(source) => {
                self.class_constructor_materialization(source, &arguments)
            }
            constructor_work::ClassConstructorSource::Imported(source) => {
                self.imported_constructor_materialization(source, &arguments)
            }
        };
        self.class_constructor_materializations[index] = Some(materialization);
        materialization
    }

    pub(super) fn resolve_struct_constructor(&mut self, index: usize) -> CallableMaterialization {
        if let Some(materialization) = self.struct_constructor_materializations[index] {
            return materialization;
        }
        let (source, structure) = self.concretizer.struct_constructor_keys[index];
        let arguments = self.concretizer.structs[structure].type_arguments.clone();
        let materialization = match source {
            constructor_work::StructConstructorSource::Local(source) => {
                self.struct_constructor_materialization(source, &arguments)
            }
            constructor_work::StructConstructorSource::Imported(source) => {
                self.imported_constructor_materialization(source, &arguments)
            }
        };
        self.struct_constructor_materializations[index] = Some(materialization);
        materialization
    }

    pub(super) fn imported_constructor_materialization(
        &mut self,
        constructor: export::ImportedConstructorTemplateId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let template = &self.concretizer.source.imported_constructor_templates[constructor];
        let ty = match &self.concretizer.source.types[template.owner] {
            export::Type::ImportedStruct(owner) => {
                let owner = self.concretizer.struct_by_key
                    [&(owner.declaration.owner(), arguments.to_vec())];
                self.concretizer.struct_type[&owner]
            }
            export::Type::ImportedClass(owner) => {
                let owner =
                    self.concretizer.class_by_key[&(owner.declaration.owner(), arguments.to_vec())];
                self.concretizer.class_type[&owner]
            }
            _ => unreachable!("imported constructors retain their nominal owner"),
        };
        let exact_owner = self.exact_types[ty].id();
        let origin = template.declaration;
        CallableMaterialization::new(
            CallableTemplateOwner::Constructor(origin),
            self.constructor_application_context(origin, exact_owner, arguments),
        )
    }

    pub(super) fn class_constructor_materialization(
        &mut self,
        constructor: export::ClassConstructorId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let declaration = &self.concretizer.source.class_constructors[constructor];
        assert_eq!(
            self.concretizer.source.classes[declaration.owner]
                .type_params
                .len(),
            arguments.len()
        );
        let origin = self.concretizer.source.nominal_identities[declaration.owner].declaration_id();
        let owner = self.concretizer.class_by_key[&(origin, arguments.to_vec())];
        let exact_owner = self.exact_types[self.concretizer.class_type[&owner]].id();
        let identity = &self.concretizer.source.constructor_identities[constructor];
        let (template, origin) = match identity {
            export::HirClassConstructorIdentity::Source(record) => {
                (CallableTemplateOwner::Constructor(record.id()), record.id())
            }
            export::HirClassConstructorIdentity::ZeroArgumentAdapter { source, record } => {
                let origin = self.concretizer.source.constructor_identities[*source]
                    .source_record()
                    .expect("a zero-argument adapter references a source constructor")
                    .id();
                (CallableTemplateOwner::Generated(record.id()), origin)
            }
        };
        CallableMaterialization::new(
            template,
            self.constructor_application_context(origin, exact_owner, arguments),
        )
    }

    pub(super) fn struct_constructor_materialization(
        &mut self,
        constructor: export::StructConstructorId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let declaration = &self.concretizer.source.struct_constructors[constructor];
        assert_eq!(
            self.concretizer.source.structs[declaration.owner]
                .type_params
                .len(),
            arguments.len()
        );
        let origin = self.concretizer.source.nominal_identities[declaration.owner].declaration_id();
        let owner = self.concretizer.struct_by_key[&(origin, arguments.to_vec())];
        let exact_owner = self.exact_types[self.concretizer.struct_type[&owner]].id();
        let origin = self.concretizer.source.constructor_identities[constructor].id();
        CallableMaterialization::new(
            CallableTemplateOwner::Constructor(origin),
            self.constructor_application_context(origin, exact_owner, arguments),
        )
    }
}
