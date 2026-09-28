use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_imported_interface(
        &mut self,
        source: &export::ImportedInterfaceType,
        substitution: &[concrete::TypeId],
    ) -> concrete::TypeId {
        let declaration = &source.declaration;
        let arguments = source
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let identity = (declaration.owner(), arguments.clone());
        if let Some(id) = self.imported_interfaces.get(&identity) {
            return self.interface_type[id];
        }
        let id = concrete::InterfaceId::from_raw(
            u32::try_from(self.interfaces.len())
                .expect("concrete interface ids fit in u32")
                .into(),
        );
        let next_family = self.source.interfaces.len() + self.imported_interface_families.len();
        let family = *self
            .imported_interface_families
            .entry(declaration.owner())
            .or_insert_with(|| {
                concrete::InterfaceFamilyId::from_raw(
                    u32::try_from(next_family).expect("concrete interface families fit in u32"),
                )
            });
        let ty = self.intern_type(concrete::TypeKind::Interface(id), false);
        let allocated = self.interfaces.alloc(concrete::InterfaceDef {
            origin: export::HirNominalIdentity::Source(declaration.identity.clone()),
            canonical_type: ty,
            name: declaration.name().to_owned(),
            owner: None,
            family,
            type_arguments: arguments,
            parents: Vec::new(),
            methods: Vec::new(),
            span: scoop_ast::Span::new(0, 0),
        });
        assert_eq!(allocated, id);
        self.imported_interfaces.insert(identity, id);
        self.interface_type.insert(id, ty);
        self.interfaces[id].parents = source
            .parents
            .iter()
            .map(|parent| self.lower_type(*parent, substitution))
            .collect();
        let methods = source
            .methods
            .iter()
            .enumerate()
            .map(|(index, method)| {
                self.interface_slot_by_source.insert(
                    (id, method.slot.id()),
                    concrete::InterfaceMethodSlot::from_raw(index as u32),
                );
                self.lower_imported_interface_method(method, substitution)
            })
            .collect();
        self.interfaces[id].methods = methods;
        ty
    }
    pub(in crate::concretize) fn lower_imported_interface_method(
        &mut self,
        method: &export::ImportedInterfaceMethod,
        substitution: &[concrete::TypeId],
    ) -> concrete::MethodSig {
        let effects = method.declaration.effects();
        concrete::MethodSig {
            name: method.name.clone(),
            is_suspend: effects.execution() == scoop_identity::Effect::Suspend,
            attributes: export::FunctionAttributes {
                safety: match effects.safety() {
                    export::CallableSafetyV1::Safe => export::Safety::Safe,
                    export::CallableSafetyV1::Unsafe => export::Safety::Unsafe,
                },
                gc_effect: match effects.gc_effect() {
                    scoop_identity::GcEffect::Managed => export::GcEffect::Managed,
                    scoop_identity::GcEffect::NoGc => export::GcEffect::NoGc,
                },
                calling_convention: export::CallingConvention::Cdecl,
            },
            implementation: match method.declaration.modality() {
                export::CallableModalityV1::Abstract => {
                    concrete::InterfaceMemberImplementation::AbstractSlot
                }
                _ => concrete::InterfaceMemberImplementation::Body,
            },
            params: method
                .parameters
                .iter()
                .enumerate()
                .map(|(index, (name, ty))| concrete::Param {
                    name: name.clone(),
                    ty: self.lower_type(*ty, substitution),
                    local: concrete::LocalId::from_raw(
                        u32::try_from(index + 1)
                            .expect("method parameter index fits in u32")
                            .into(),
                    ),
                })
                .collect(),
            return_ty: self.lower_type(method.return_type, substitution),
            span: scoop_ast::Span::new(0, 0),
        }
    }
}
