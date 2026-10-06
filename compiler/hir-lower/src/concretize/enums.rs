use super::*;

mod definition;

use definition::ResolvedEnumDefinition;

impl Concretizer<'_> {
    pub(super) fn ensure_enum(
        &mut self,
        source: export::EnumId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::EnumId {
        let origin = self.source.nominal_identities[source].declaration_id();
        self.ensure_enum_definition(origin, arguments)
    }

    pub(super) fn ensure_enum_definition(
        &mut self,
        origin: export::SourceNominalId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::EnumId {
        let key = (origin, arguments.clone());
        if let Some(&id) = self.enum_by_key.get(&key) {
            return id;
        }
        let previous_site = self.type_use_site;
        self.type_use_site = previous_site.or_else(|| self.source_nominal_site(origin));
        let source = self.source.nominal_identities.enum_id(origin);
        let definition = match source {
            Some(source) => self.source_enum_definition(source),
            None => ResolvedEnumDefinition::from_dependency(
                &self.source.loaded_enum_definitions[&origin],
            ),
        };
        let id = self.allocate_enum_definition(&definition, arguments.clone());
        if let Some(source) = source {
            self.enum_source.insert(id, source);
        }
        self.complete_enum_definition(id, definition, &arguments);
        if let Some(imported) = self.source.loaded_enum_definitions.get(&origin) {
            self.check_loaded_contexts(&imported.context_contracts, &arguments);
        }
        self.type_use_site = previous_site;
        id
    }

    fn allocate_enum_definition(
        &mut self,
        definition: &ResolvedEnumDefinition<'_>,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::EnumId {
        let key = (definition.origin.declaration_id(), arguments.clone());
        let id = concrete::EnumId::from_raw(
            u32::try_from(self.enums.len())
                .expect("concrete enum ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Enum(id), false);
        let allocated = self.enums.alloc(concrete::EnumDef {
            origin: definition.origin.clone(),
            canonical_type: ty,
            name: definition.name.clone(),
            owner: definition.owner.clone(),
            type_arguments: arguments,
            gc_free: false,
            variants: Vec::new(),
            direct_interfaces: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: definition.span,
        });
        assert_eq!(allocated, id);
        self.enum_by_key.insert(key, id);
        self.enum_type.insert(id, ty);
        id
    }

    fn complete_enum_definition(
        &mut self,
        id: concrete::EnumId,
        definition: ResolvedEnumDefinition<'_>,
        substitution: &[concrete::TypeId],
    ) {
        let variants: Vec<_> = definition
            .variants
            .into_iter()
            .map(|variant| {
                let fields: Vec<_> = variant
                    .fields
                    .into_iter()
                    .map(|field| concrete::VariantField {
                        identity: field.identity,
                        name: field.name.to_owned(),
                        ty: self.lower_type(field.ty, substitution),
                    })
                    .collect();
                concrete::Variant {
                    identity: variant.identity,
                    name: variant.name.to_owned(),
                    gc_free: fields.iter().all(|field| self.types[field.ty].gc_free),
                    fields,
                }
            })
            .collect();
        let gc_free = variants.iter().all(|variant| variant.gc_free);
        self.enums[id].variants = variants;
        self.enums[id].gc_free = gc_free;
        self.types[self.enum_type[&id]].gc_free = gc_free;
        self.check_completed_no_gc_type("enum", &definition.name, definition.no_gc, gc_free);
        let encoding_applies = definition
            .element_encoding
            .is_some_and(|encoding| self.concrete_encoding_applies(encoding, substitution));
        let methods = self.request_encoding_methods(
            definition.methods,
            concrete::MethodOwner::Enum(id),
            definition.element_encoding.filter(|_| !encoding_applies),
        );
        let mut direct_interfaces: Vec<_> = definition
            .interfaces
            .iter()
            .map(|interface| self.lower_type(*interface, substitution))
            .collect();
        let mut interface_implementations = self
            .lower_interface_implementations(definition.interface_implementations, substitution);
        if let Some(encoding) = definition.element_encoding.filter(|_| encoding_applies) {
            direct_interfaces
                .push(self.lower_type(encoding.implementation.interface, substitution));
            interface_implementations.extend(self.lower_interface_implementations(
                std::slice::from_ref(&encoding.implementation),
                substitution,
            ));
        }
        let interfaces = interface_implementations
            .iter()
            .map(|implementation| self.interface_type[&implementation.interface])
            .collect();
        self.enums[id].direct_interfaces = direct_interfaces;
        self.enums[id].interfaces = interfaces;
        self.enums[id].interface_implementations = interface_implementations;
        self.enums[id].methods = methods;
    }
}
