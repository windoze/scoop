use super::*;

impl Lowerer {
    pub(crate) fn validate_core_equality_conformances(&mut self) {
        if self.equality_core.is_none() {
            return;
        }
        let owners = self
            .intrinsic_type_owners
            .iter()
            .filter_map(|(kind, (owner, _))| {
                matches!(
                    kind,
                    hir::IntrinsicTypeKind::Unit
                        | hir::IntrinsicTypeKind::Integer(_)
                        | hir::IntrinsicTypeKind::Boolean
                        | hir::IntrinsicTypeKind::Char
                        | hir::IntrinsicTypeKind::Float(_)
                        | hir::IntrinsicTypeKind::String
                        | hir::IntrinsicTypeKind::Ptr
                )
                .then_some(*owner)
            })
            .collect::<Vec<_>>();
        for owner in owners {
            let (owner, file, span) = match owner {
                IntrinsicTypeOwner::Struct(id) => (
                    Owner::Struct(id),
                    self.struct_files[&id],
                    self.structs[id].span,
                ),
                IntrinsicTypeOwner::Class(id) => (
                    Owner::Class(id),
                    self.class_files[&id],
                    self.classes[id].span,
                ),
            };
            let ty = self.owner_ty(owner);
            let expected = self.equality_interface(ty).expect("Equality was checked");
            if !self
                .type_interfaces(ty)
                .into_iter()
                .any(|interface| self.types_equal(interface, expected))
            {
                self.current_file = file;
                self.error(
                    span,
                    format!(
                        "core type `{}` must implement `Equality<{}>`",
                        self.type_name(ty),
                        self.type_name(ty)
                    ),
                );
            }
        }
    }

    pub(crate) fn validate_equality_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::EqualityCore> {
        let interface = self.require_core_interface("Equality", files)?;
        let declaration = &self.interfaces[interface];
        let checked = match (
            declaration.type_params.as_slice(),
            declaration.methods.as_slice(),
        ) {
            ([parameter], [member]) => {
                let relation = &self.interface_method_entities[*member];
                let function = &self.functions[relation.function];
                let signature = &self.signatures[&relation.function];
                (parameter.bounds == hir::TypeParamBounds::Unconstrained
                    && declaration.parents.is_empty()
                    && relation.implementation == hir::InterfaceMemberImplementation::AbstractSlot
                    && relation.role == hir::InterfaceMemberRole::Function
                    && function.name.rsplit('.').next() == Some("equals")
                    && function.access.declared == hir::DeclaredVisibility::Public
                    && !signature.is_suspend
                    && signature.attributes == hir::FunctionAttributes::default()
                    && signature.modifiers.operator == Some(hir::OperatorKind::Equals)
                    && signature.modifiers.property_delegate_operator.is_none()
                    && !signature.modifiers.is_infix
                    && signature.context_parameters.is_empty()
                    && function.method_type_param_count() == 0
                    && signature.return_ty == self.boolean
                    && matches!(signature.params.as_slice(), [other]
                        if matches!(other.calling, FnParamCalling::Required)
                            && matches!(self.types[other.ty], Type::Param(id) if id == parameter.id)))
                .then_some(hir::EqualityCore {
                    interface,
                    equals: *member,
                })
            }
            _ => None,
        };
        if checked.is_none() {
            self.current_file = self.interface_files[&interface];
            self.error(
                self.interfaces[interface].span,
                "interface `Equality<T>` in scoop.core must declare exactly `public operator fun equals(other: T): Boolean`"
                    .into(),
            );
        }
        checked
    }
}
