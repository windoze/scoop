use super::*;

impl Lowerer {
    pub(crate) fn declaration_is_exported(access: &hir::DeclarationAccess) -> bool {
        access.declared == hir::DeclaredVisibility::Public && access.lookup.0.is_universal()
    }

    fn nominal_is_exported(access: &hir::NominalAccess) -> bool {
        access.declared == hir::DeclaredVisibility::Public && access.lookup.0.is_universal()
    }

    pub(crate) fn public_semantic_surface(&self) -> hir::PublicSemanticSurface {
        let object_backings = self
            .objects
            .iter()
            .map(|(_, declaration)| declaration.backing_class)
            .collect::<std::collections::HashSet<_>>();
        let accessor_functions = self
            .property_accessor_sources
            .iter()
            .map(|source| source.function)
            .collect::<std::collections::HashSet<_>>();
        let functions = self
            .functions
            .iter()
            .filter_map(|(id, function)| {
                (self.source_function_declarations.contains_key(&id)
                    && !accessor_functions.contains(&id)
                    && Self::declaration_is_exported(&function.access))
                .then_some(id)
            })
            .collect::<Vec<_>>();
        let generic_functions = self
            .generic_functions
            .iter()
            .filter_map(|(id, generic)| functions.contains(&generic.function).then_some(id))
            .collect();
        let generic_methods = self
            .generic_methods
            .iter()
            .filter_map(|(id, generic)| functions.contains(&generic.function).then_some(id))
            .collect();
        hir::PublicSemanticSurface {
            functions: functions.clone(),
            properties: self
                .properties
                .iter()
                .filter_map(|(id, property)| {
                    Self::declaration_is_exported(&property.access).then_some(id)
                })
                .collect(),
            property_getters: self
                .property_getters
                .iter()
                .filter_map(|(id, getter)| {
                    Self::declaration_is_exported(&getter.access).then_some(id)
                })
                .collect(),
            property_setters: self
                .property_setters
                .iter()
                .filter_map(|(id, setter)| {
                    Self::declaration_is_exported(&setter.access).then_some(id)
                })
                .collect(),
            generic_functions,
            generic_methods,
            structs: self
                .structs
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(id)
                })
                .collect(),
            struct_constructors: self
                .struct_constructors
                .iter()
                .filter_map(|(id, constructor)| {
                    Self::declaration_is_exported(&constructor.access).then_some(id)
                })
                .collect(),
            enums: self
                .enums
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(id)
                })
                .collect(),
            classes: self
                .classes
                .iter()
                .filter_map(|(id, declaration)| {
                    (!object_backings.contains(&id)
                        && Self::nominal_is_exported(&declaration.access))
                    .then_some(id)
                })
                .collect(),
            class_constructors: self
                .class_constructors
                .iter()
                .filter_map(|(id, constructor)| {
                    (!object_backings.contains(&constructor.owner)
                        && Self::declaration_is_exported(&constructor.access))
                    .then_some(id)
                })
                .collect(),
            interfaces: self
                .interfaces
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(id)
                })
                .collect(),
            interface_methods: self
                .interface_method_entities
                .iter()
                .filter_map(|(id, method)| functions.contains(&method.function).then_some(id))
                .collect(),
            objects: self
                .objects
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(id)
                })
                .collect(),
            object_types: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| {
                    Self::nominal_is_exported(&declaration.access)
                        .then_some(declaration.object_type)
                })
                .collect(),
            companion_relations: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| {
                    if !Self::nominal_is_exported(&declaration.access) {
                        return None;
                    }
                    match declaration.kind {
                        hir::ObjectKind::Standalone => None,
                        hir::ObjectKind::Companion(relation) => Some(relation),
                    }
                })
                .collect(),
            singleton_values: self
                .objects
                .iter()
                .filter_map(|(_, declaration)| {
                    Self::nominal_is_exported(&declaration.access)
                        .then_some(declaration.singleton_value)
                })
                .collect(),
            annotations: self
                .source_annotations
                .iter()
                .filter_map(|(id, declaration)| {
                    Self::nominal_is_exported(&declaration.access).then_some(*id)
                })
                .collect(),
            type_aliases: self
                .type_aliases
                .iter()
                .filter_map(|(id, alias)| {
                    Self::declaration_is_exported(&alias.access).then_some(id)
                })
                .collect(),
        }
    }
}
