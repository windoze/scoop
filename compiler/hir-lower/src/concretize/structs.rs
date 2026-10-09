use super::*;

mod c_abi;
mod definition;
mod intrinsics;

use definition::{ResolvedStructDefinition, ResolvedStructRepresentation};

impl Concretizer<'_> {
    pub(super) fn ensure_struct_definition(
        &mut self,
        origin: export::SourceNominalId,
        arguments: Vec<concrete::TypeId>,
        application: ConcreteApplicationRepresentation,
    ) -> concrete::StructId {
        let key = (origin, arguments.clone());
        if let Some(&id) = self.struct_by_key.get(&key) {
            return id;
        }
        let previous_site = self.type_use_site;
        self.type_use_site = previous_site.or_else(|| self.source_nominal_site(origin));
        let source = self.source.nominal_identities.struct_id(origin);
        let definition = match source {
            Some(source) => self.source_struct_definition(source, application),
            None => ResolvedStructDefinition::from_dependency(
                &self.source.loaded_struct_definitions[&origin],
                application,
            ),
        };
        let id = self.allocate_struct_definition(&definition, arguments.clone());
        if let Some(source) = source {
            self.struct_source.insert(id, source);
        }
        self.complete_struct_definition(id, definition, &arguments);
        if let Some(imported) = self.source.loaded_struct_definitions.get(&origin) {
            self.check_loaded_contexts(&imported.context_contracts, &arguments);
        }
        self.type_use_site = previous_site;
        id
    }

    fn allocate_struct_definition(
        &mut self,
        definition: &ResolvedStructDefinition<'_>,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::StructId {
        let key = (definition.origin.declaration_id(), arguments.clone());
        let id = concrete::StructId::from_raw(
            u32::try_from(self.structs.len())
                .expect("concrete struct ids fit in u32")
                .into(),
        );
        let representation = match &definition.representation {
            ResolvedStructRepresentation::Declared {
                attributes, c_abi, ..
            } => concrete::StructRepresentation::Declared {
                attributes: *attributes,
                c_abi: *c_abi,
                fields: Vec::new(),
            },
            ResolvedStructRepresentation::Intrinsic {
                declaration,
                application,
            } => concrete::StructRepresentation::Intrinsic {
                declaration: *declaration,
                application: application.clone(),
            },
        };
        let (kind, gc_free) = match &representation {
            concrete::StructRepresentation::Declared { .. } => {
                (concrete::TypeKind::Struct(id), false)
            }
            concrete::StructRepresentation::Intrinsic { application, .. } => {
                let kind = match application {
                    concrete::IntrinsicTypeRepresentation::Integer(kind) => {
                        concrete::TypeKind::Integer(*kind)
                    }
                    concrete::IntrinsicTypeRepresentation::Char
                    | concrete::IntrinsicTypeRepresentation::MaybeUninit { .. }
                    | concrete::IntrinsicTypeRepresentation::Float(_) => {
                        concrete::TypeKind::Struct(id)
                    }
                    concrete::IntrinsicTypeRepresentation::Unit => concrete::TypeKind::Unit,
                    concrete::IntrinsicTypeRepresentation::Boolean => concrete::TypeKind::Boolean,
                    concrete::IntrinsicTypeRepresentation::Ptr { pointee } => {
                        concrete::TypeKind::Ptr(*pointee)
                    }
                    concrete::IntrinsicTypeRepresentation::FunPtr { signature } => {
                        concrete::TypeKind::FunPtr(*signature)
                    }
                    _ => unreachable!("the registry fixes intrinsic struct representations"),
                };
                let gc_free = match application {
                    concrete::IntrinsicTypeRepresentation::MaybeUninit { value } => {
                        self.types[*value].gc_free
                    }
                    _ => true,
                };
                (kind, gc_free)
            }
        };
        let ty = self.intern_type(kind, gc_free);
        let allocated = self.structs.alloc(concrete::StructDef {
            origin: definition.origin.clone(),
            canonical_type: ty,
            name: definition.name.clone(),
            owner: definition.owner.clone(),
            type_arguments: arguments,
            gc_free,
            representation,
            direct_interfaces: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: definition.span,
        });
        assert_eq!(allocated, id);
        self.struct_by_key.insert(key, id);
        self.struct_type.insert(id, ty);
        id
    }

    fn complete_struct_definition(
        &mut self,
        id: concrete::StructId,
        definition: ResolvedStructDefinition<'_>,
        substitution: &[concrete::TypeId],
    ) {
        if let ResolvedStructRepresentation::Declared { fields, .. } = definition.representation {
            let fields: Vec<_> = fields
                .into_iter()
                .map(|field| concrete::DeclaredStructField {
                    identity: field.identity,
                    name: field.name.to_owned(),
                    ty: self.lower_type(field.ty, substitution),
                })
                .collect();
            let gc_free = fields.iter().all(|field| self.types[field.ty].gc_free);
            let concrete::StructRepresentation::Declared {
                fields: concrete_fields,
                ..
            } = &mut self.structs[id].representation
            else {
                unreachable!("a declared struct retains its field representation")
            };
            *concrete_fields = fields;
            self.structs[id].gc_free = gc_free;
            self.types[self.struct_type[&id]].gc_free = gc_free;
        }
        let no_gc = match &self.structs[id].representation {
            concrete::StructRepresentation::Declared { attributes, .. } => attributes.no_gc,
            concrete::StructRepresentation::Intrinsic { .. } => false,
        };
        self.check_completed_no_gc_type(
            "struct",
            &definition.name,
            no_gc,
            self.structs[id].gc_free,
        );
        let methods =
            self.request_concrete_methods(definition.methods, concrete::MethodOwner::Struct(id));
        let direct_interfaces: Vec<_> = definition
            .interfaces
            .iter()
            .map(|interface| self.lower_type(*interface, substitution))
            .collect();
        let interface_implementations = self
            .lower_interface_implementations(definition.interface_implementations, substitution);
        self.structs[id].interfaces = self.concrete_interface_closure(&direct_interfaces);
        self.structs[id].direct_interfaces = direct_interfaces;
        self.structs[id].interface_implementations = interface_implementations;
        self.structs[id].methods = methods;
        for &constructor in definition.constructors {
            if self.automatic_struct_constructor(constructor) {
                self.request_struct_constructor(constructor, id);
            }
        }
    }

    fn concrete_interface_closure(&self, roots: &[concrete::TypeId]) -> Vec<concrete::TypeId> {
        let mut pending: Vec<_> = roots.iter().rev().copied().collect();
        let mut interfaces = Vec::new();
        while let Some(ty) = pending.pop() {
            if interfaces.contains(&ty) {
                continue;
            }
            let concrete::TypeKind::Interface(interface) = self.types[ty].kind else {
                unreachable!("value supertypes retain their concrete interface representation")
            };
            pending.extend(self.interfaces[interface].parents.iter().rev().copied());
            interfaces.push(ty);
        }
        interfaces
    }
}
