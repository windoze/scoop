use super::*;
use scoop_identity::SignatureTypeKey;

impl<'a> Concretizer<'a> {
    pub(super) fn metadata_nominal_source(
        &self,
        declaration: export::SourceNominalId,
    ) -> export::TypeId {
        if declaration
            == export::SourceNominalId::Concrete(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            )
        {
            return self.source.unit;
        }
        if let Some(owner) = self.source.nominal_identities.declaration(declaration) {
            return match owner {
                export::NominalOwner::Struct(id) => {
                    self.source.struct_applications[self.source.structs[id].self_application]
                        .canonical_type
                }
                export::NominalOwner::Enum(id) => {
                    self.source.enum_applications[self.source.enums[id].self_application]
                        .canonical_type
                }
                export::NominalOwner::Class(id) => {
                    self.source.class_applications[self.source.classes[id].self_application]
                        .canonical_type
                }
                export::NominalOwner::Interface(id) => {
                    self.source.interface_applications[self.source.interfaces[id].self_application]
                        .canonical_type
                }
                export::NominalOwner::Object(id) => {
                    let class = self.source.objects[id].backing_class;
                    self.source.class_applications[self.source.classes[class].self_application]
                        .canonical_type
                }
            };
        }
        if let Some(definition) = self.source.loaded_struct_definitions.get(&declaration) {
            return self.source.struct_applications[definition.definition.self_application]
                .canonical_type;
        }
        if let Some(definition) = self.source.loaded_enum_definitions.get(&declaration) {
            return self.source.enum_applications[definition.definition.self_application]
                .canonical_type;
        }
        if let Some(definition) = self.source.loaded_class_definitions.get(&declaration) {
            return self.source.class_applications[definition.definition.self_application]
                .canonical_type;
        }
        if let Some(definition) = self.source.loaded_interface_definitions.get(&declaration) {
            return self.source.interface_applications[definition.definition.self_application]
                .canonical_type;
        }
        if let export::CoreProtocols::Imported(core) = self.core {
            let fundamental = core.fundamental_types();
            if declaration == export::SourceNominalId::Concrete(fundamental.any().persistent()) {
                return self
                    .source
                    .types
                    .iter()
                    .find(|(_, ty)| matches!(ty, export::Type::Any))
                    .unwrap()
                    .0;
            }
            if declaration == export::SourceNominalId::Concrete(fundamental.boolean().persistent())
            {
                return self.source.boolean;
            }
            if declaration == export::SourceNominalId::Concrete(fundamental.string().persistent()) {
                return self.source.string;
            }
            for (id, ty) in self.source.types.iter() {
                if let export::Type::Integer(kind) = ty
                    && declaration
                        == export::SourceNominalId::Concrete(
                            fundamental.integer(*kind).persistent(),
                        )
                {
                    return id;
                }
            }
        }
        panic!("metadata uses a retained source nominal: {declaration:?}")
    }

    pub(super) fn metadata_shape(
        &self,
        ty: concrete::TypeId,
    ) -> Option<export::StaticNominalShape<'a>> {
        let nominal = self.type_nominal(ty)?;
        if matches!(
            self.types[ty].kind,
            concrete::TypeKind::Unit
                | concrete::TypeKind::Any
                | concrete::TypeKind::Ptr(_)
                | concrete::TypeKind::FunPtr(_)
        ) {
            return None;
        }
        let source = self.metadata_nominal_source(nominal);
        match self
            .source
            .static_type_shape(source, &[])
            .expect("nominal self applications retain their binders")
        {
            export::StaticTypeShape::Nominal(shape) => Some(shape),
            _ => None,
        }
    }

    pub(super) fn metadata_source_type(
        &mut self,
        ty: export::StaticShapeTypeSource<'_>,
        arguments: &[concrete::TypeId],
    ) -> concrete::TypeId {
        match ty {
            export::StaticShapeTypeSource::Hir(ty) => self.lower_type(ty, arguments),
            export::StaticShapeTypeSource::Published(ty) => {
                self.metadata_signature_type(ty, arguments)
            }
        }
    }

    pub(super) fn metadata_signature_type(
        &mut self,
        ty: &SignatureTypeKey,
        arguments: &[concrete::TypeId],
    ) -> concrete::TypeId {
        match ty {
            SignatureTypeKey::Nominal(id) => {
                let source = self.metadata_nominal_source(export::SourceNominalId::Concrete(*id));
                self.lower_type(source, &[])
            }
            SignatureTypeKey::NominalApplication {
                origin,
                arguments: parameters,
            } => {
                let source =
                    self.metadata_nominal_source(export::SourceNominalId::GenericTemplate(*origin));
                let parameters = parameters
                    .as_slice()
                    .iter()
                    .map(|ty| self.metadata_signature_type(ty, arguments))
                    .collect::<Vec<_>>();
                self.lower_type(source, &parameters)
            }
            SignatureTypeKey::Binder { depth, index } => {
                assert_eq!(*depth, 0, "nominal source members have one binder frame");
                arguments[*index as usize]
            }
            SignatureTypeKey::Tuple(elements) => {
                let elements = elements
                    .as_slice()
                    .iter()
                    .map(|ty| self.metadata_signature_type(ty, arguments))
                    .collect::<Vec<_>>();
                let gc_free = elements.iter().all(|ty| self.types[*ty].gc_free);
                self.intern_type(concrete::TypeKind::Tuple(elements), gc_free)
            }
            SignatureTypeKey::Function {
                effect,
                receiver,
                parameters,
                result,
            } => {
                let receiver = receiver
                    .as_ref()
                    .map(|ty| self.metadata_signature_type(ty, arguments));
                let parameters = parameters
                    .as_slice()
                    .iter()
                    .map(|ty| self.metadata_signature_type(ty, arguments))
                    .collect();
                let result = self.metadata_signature_type(result, arguments);
                let signature = self.intern_receiver_function_type(
                    *effect == scoop_identity::Effect::Suspend,
                    receiver,
                    parameters,
                    result,
                );
                self.function_types[signature].canonical_type
            }
            SignatureTypeKey::RawPointer(pointee) => {
                let pointee = self.metadata_signature_type(pointee, arguments);
                self.intern_type(concrete::TypeKind::Ptr(pointee), true)
            }
            SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                let parameters = parameters
                    .as_slice()
                    .iter()
                    .map(|ty| self.metadata_signature_type(ty, arguments))
                    .collect();
                let result = self.metadata_signature_type(result, arguments);
                let signature = self.intern_function_type(false, parameters, result);
                self.intern_type(concrete::TypeKind::FunPtr(signature), true)
            }
        }
    }
}
