use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn type_reference_data(
        &mut self,
        target: concrete::TypeId,
        result: concrete::TypeId,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        let fields = self.metadata_fields(result);
        let identity = value(
            concrete::ExprKind::TypeIdentityLiteral(target),
            fields[0],
            origin,
        );
        let name = self.metadata_string(self.type_display_name(target), origin);
        let concrete::TypeKind::Enum(option) = self.types[fields[2]].kind else {
            unreachable!("TypeReferenceData.nominal has an Option type")
        };
        let nominal = self.type_nominal(target).map(|nominal| {
            let id = self.metadata_string(nominal_identity(nominal), origin);
            self.metadata_record(self.enums[option].type_arguments[0], vec![id], origin)
        });
        let nominal = self.metadata_option(fields[2], nominal, origin);
        let concrete::TypeKind::Class(array) = self.types[fields[3]].kind else {
            unreachable!("TypeReferenceData.arguments has an Array type")
        };
        let reference = self.classes[array].type_arguments[0];
        let arguments = self
            .type_arguments(target)
            .into_iter()
            .map(|ty| self.type_reference_value(ty, reference, origin))
            .collect();
        let arguments = value(
            concrete::ExprKind::ArrayLiteral(arguments),
            fields[3],
            origin,
        );
        self.metadata_record(result, vec![identity, name, nominal, arguments], origin)
    }

    pub(super) fn type_arguments(&self, ty: concrete::TypeId) -> Vec<concrete::TypeId> {
        match self.types[ty].kind {
            concrete::TypeKind::Struct(id) => self.structs[id].type_arguments.clone(),
            concrete::TypeKind::Enum(id) => self.enums[id].type_arguments.clone(),
            concrete::TypeKind::Class(id) => self.classes[id].type_arguments.clone(),
            concrete::TypeKind::Interface(id) => self.interfaces[id].type_arguments.clone(),
            concrete::TypeKind::Ptr(pointee) => vec![pointee],
            concrete::TypeKind::FunPtr(signature) => {
                vec![self.function_types[signature].canonical_type]
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn type_nominal(&self, ty: concrete::TypeId) -> Option<export::SourceNominalId> {
        use concrete::TypeKind as T;
        let nominal = match self.types[ty].kind {
            T::Unit => export::SourceNominalId::Concrete(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            ),
            T::Struct(id) => self.structs[id].origin.declaration_id(),
            T::Enum(id) => self.enums[id].origin.declaration_id(),
            T::Class(id) => self
                .objects
                .iter()
                .find(|(_, object)| object.backing_class == id)
                .map(|(_, object)| object.origin.declaration_id())
                .unwrap_or_else(|| self.classes[id].origin.declaration_id()),
            T::Interface(id) => self.interfaces[id].origin.declaration_id(),
            T::Tuple(_) | T::Function(_) => return None,
            T::Integer(_) | T::Boolean | T::String | T::Any | T::Ptr(_) | T::FunPtr(_) => {
                match self.core {
                    export::CoreProtocols::Defined(core) => match self.types[ty].kind {
                        T::Integer(kind) => self.source.nominal_identities
                            [core.fundamental_types.integers.owner(kind)]
                        .declaration_id(),
                        T::Boolean => self.source.nominal_identities
                            [core.fundamental_types.boolean]
                            .declaration_id(),
                        T::String => self.source.nominal_identities[core.fundamental_types.string]
                            .declaration_id(),
                        T::Any => self.source.nominal_identities[core.fundamental_types.any]
                            .declaration_id(),
                        T::Ptr(_) => self.source.nominal_identities[core.ffi.ptr].declaration_id(),
                        T::FunPtr(_) => {
                            self.source.nominal_identities[core.ffi.fun_ptr].declaration_id()
                        }
                        _ => unreachable!("primitive branch selects a core nominal"),
                    },
                    export::CoreProtocols::Imported(core) => {
                        let core = core.fundamental_types();
                        match self.types[ty].kind {
                            T::Integer(kind) => {
                                export::SourceNominalId::Concrete(core.integer(kind).persistent())
                            }
                            T::Boolean => {
                                export::SourceNominalId::Concrete(core.boolean().persistent())
                            }
                            T::String => {
                                export::SourceNominalId::Concrete(core.string().persistent())
                            }
                            T::Any => export::SourceNominalId::Concrete(core.any().persistent()),
                            T::Ptr(_) => {
                                export::SourceNominalId::GenericTemplate(core.ptr().persistent())
                            }
                            T::FunPtr(_) => export::SourceNominalId::GenericTemplate(
                                core.fun_ptr().persistent(),
                            ),
                            _ => unreachable!("primitive branch selects a core nominal"),
                        }
                    }
                }
            }
        };
        Some(nominal)
    }

    pub(super) fn type_display_name(&self, ty: concrete::TypeId) -> String {
        use concrete::TypeKind as T;
        let name = match &self.types[ty].kind {
            T::Unit => "Unit".into(),
            T::Integer(kind) => kind.canonical_name().into(),
            T::Boolean => "Boolean".into(),
            T::String => "String".into(),
            T::Any => "Any".into(),
            T::Struct(id) => self.structs[*id].name.clone(),
            T::Enum(id) => self.enums[*id].name.clone(),
            T::Class(id) => self.classes[*id].name.clone(),
            T::Interface(id) => self.interfaces[*id].name.clone(),
            T::Ptr(_) => "Ptr".into(),
            T::FunPtr(_) => "FunPtr".into(),
            T::Tuple(elements) => {
                return format!(
                    "({})",
                    elements
                        .iter()
                        .map(|ty| self.type_display_name(*ty))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            T::Function(id) => {
                let signature = &self.function_types[*id];
                let receiver = signature
                    .receiver
                    .map(|ty| format!("{}.", self.type_display_name(ty)))
                    .unwrap_or_default();
                let parameters = signature
                    .parameter_types
                    .iter()
                    .map(|ty| self.type_display_name(*ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                return format!(
                    "{}{}({parameters}) -> {}",
                    if signature.is_suspend { "suspend " } else { "" },
                    receiver,
                    self.type_display_name(signature.return_type)
                );
            }
        };
        let arguments = self.type_arguments(ty);
        if arguments.is_empty() {
            name
        } else {
            format!(
                "{name}<{}>",
                arguments
                    .iter()
                    .map(|ty| self.type_display_name(*ty))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}
