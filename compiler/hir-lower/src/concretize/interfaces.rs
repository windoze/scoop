use super::*;

mod definition;
mod members;

use definition::{ResolvedInterfaceDefinition, ResolvedInterfaceMethod};

impl Concretizer<'_> {
    pub(super) fn ensure_interface(
        &mut self,
        source_id: export::InterfaceId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::InterfaceId {
        let origin = self.source.nominal_identities[source_id].declaration_id();
        self.ensure_interface_definition(origin, arguments)
    }

    pub(super) fn ensure_interface_definition(
        &mut self,
        origin: export::SourceNominalId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::InterfaceId {
        if let Some(&id) = self.interface_by_key.get(&(origin, arguments.clone())) {
            return id;
        }
        let source = self.source;
        let definition = if let Some(id) = source.nominal_identities.interface_id(origin) {
            let declaration = &source.interfaces[id];
            ResolvedInterfaceDefinition {
                origin: source.nominal_identities[id].clone(),
                name: self.source_nominal_name(&declaration.name, declaration.owner),
                owner: self.lower_nominal_owner(declaration.owner),
                parents: &declaration.parents,
                members: source.interface_applications[declaration.self_application].canonical_type,
                automatic_methods: if declaration.type_params.is_empty() {
                    &declaration.methods
                } else {
                    &[]
                },
                span: declaration.span,
            }
        } else {
            let declaration = &source.loaded_interface_definitions[&origin];
            let definition = &declaration.definition;
            let span = declaration.declaration.origin.origin().span();
            ResolvedInterfaceDefinition {
                origin: export::HirNominalIdentity::Source(
                    declaration.declaration.identity.clone(),
                ),
                name: declaration.declaration.name().to_owned(),
                owner: None,
                parents: &definition.parents,
                members: source.interface_applications[definition.self_application].canonical_type,
                automatic_methods: &[],
                span: scoop_ast::Span::new(
                    u32::try_from(span.start_byte()).expect("decoded source spans fit HIR"),
                    u32::try_from(span.end_byte()).expect("decoded source spans fit HIR"),
                ),
            }
        };
        let id = self.allocate_interface_definition(&definition, arguments.clone());
        self.complete_interface_definition(id, definition, &arguments);
        id
    }

    fn allocate_interface_definition(
        &mut self,
        definition: &ResolvedInterfaceDefinition<'_>,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::InterfaceId {
        let origin = definition.origin.declaration_id();
        let id = concrete::InterfaceId::from_raw(
            u32::try_from(self.interfaces.len())
                .expect("concrete interface ids fit in u32")
                .into(),
        );
        let next_family = self.interface_families.len();
        let family = *self.interface_families.entry(origin).or_insert_with(|| {
            concrete::InterfaceFamilyId::from_raw(
                u32::try_from(next_family).expect("concrete interface families fit in u32"),
            )
        });
        let ty = self.intern_type(concrete::TypeKind::Interface(id), false);
        let allocated = self.interfaces.alloc(concrete::InterfaceDef {
            origin: definition.origin.clone(),
            canonical_type: ty,
            name: definition.name.clone(),
            owner: definition.owner.clone(),
            family,
            type_arguments: arguments.clone(),
            parents: Vec::new(),
            methods: Vec::new(),
            span: definition.span,
        });
        assert_eq!(allocated, id);
        self.interface_by_key.insert((origin, arguments), id);
        self.interface_type.insert(id, ty);
        id
    }

    fn complete_interface_definition(
        &mut self,
        id: concrete::InterfaceId,
        definition: ResolvedInterfaceDefinition<'_>,
        substitution: &[concrete::TypeId],
    ) {
        self.interfaces[id].parents = definition
            .parents
            .iter()
            .map(|parent| self.lower_type(*parent, substitution))
            .collect();
        let methods = self.interface_method_instances(definition.members, substitution);
        self.interfaces[id].methods = methods
            .iter()
            .enumerate()
            .map(|(index, instance)| {
                self.interface_slot_by_source.insert(
                    (id, instance.method.slot),
                    concrete::InterfaceMethodSlot::from_raw(
                        u32::try_from(index).expect("concrete interface slots fit in u32"),
                    ),
                );
                self.lower_interface_method(&instance.method, &instance.arguments)
            })
            .collect();
        for &member in definition.automatic_methods {
            self.request_method(
                self.source.interface_methods[member].function,
                concrete::MethodOwner::Interface(id),
                Vec::new(),
            );
        }
    }

    fn lower_interface_method(
        &mut self,
        method: &ResolvedInterfaceMethod<'_>,
        substitution: &[concrete::TypeId],
    ) -> concrete::MethodSig {
        concrete::MethodSig {
            name: method.name.to_owned(),
            is_suspend: method.is_suspend,
            attributes: method.attributes,
            implementation: method.implementation,
            params: method
                .parameters
                .iter()
                .map(|&(name, ty, local)| concrete::Param {
                    name: name.to_owned(),
                    ty: self.lower_type(ty, substitution),
                    local,
                })
                .collect(),
            return_ty: self.lower_type(method.return_type, substitution),
            span: method.span,
        }
    }
}
