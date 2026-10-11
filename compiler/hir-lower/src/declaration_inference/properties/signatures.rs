use super::*;

impl Lowerer {
    pub(crate) fn mark_pending_property_accessor(
        &mut self,
        source: &crate::properties::PropertyAccessorSource,
    ) {
        if self.property_type_pending(source.property) {
            let signature = self
                .signatures
                .get_mut(&source.function)
                .expect("resolved accessor");
            signature.result_pending = source.kind == PropertyAccessorKind::Getter;
            signature.parameter_pending = source.kind == PropertyAccessorKind::Setter;
        }
    }

    fn property_type_parameters(&self, property: hir::PropertyId) -> Vec<hir::TypeParamDecl> {
        match self.properties[property].owner {
            hir::PropertyOwner::TopLevel => Vec::new(),
            hir::PropertyOwner::Extension(id) => self.extension_properties[id].type_params.clone(),
            hir::PropertyOwner::Class(id) => self.classes[id].type_params.clone(),
            hir::PropertyOwner::Object(id) => self.classes[self.objects[id].backing_class]
                .type_params
                .clone(),
            hir::PropertyOwner::Struct(id) => self.structs[id].type_params.clone(),
            hir::PropertyOwner::Enum(id) => self.enums[id].type_params.clone(),
            hir::PropertyOwner::Interface(id) => self.interfaces[id].type_params.clone(),
        }
    }

    pub(crate) fn set_inferred_property_type(&mut self, property: hir::PropertyId, ty: TypeId) {
        self.properties[property].ty = ty;
        match self.properties[property].representation {
            hir::PropertyRepresentation::Stored(ref stored) => match stored.backing {
                hir::PropertyBacking::ClassField { field, .. } => {
                    let field = &self.class_fields[field];
                    self.classes[field.owner].definition.fields[field.definition_index].ty = ty;
                }
                hir::PropertyBacking::TopLevelGlobal { storage, .. } => {
                    self.globals[storage].ty = ty
                }
                hir::PropertyBacking::StructField { .. } => {
                    unreachable!("primary struct fields always have explicit types")
                }
            },
            hir::PropertyRepresentation::NativeStorage { storage } => self.globals[storage].ty = ty,
            _ => {}
        }
        let parameters = self.property_type_parameters(property);
        let sources = self
            .property_accessor_sources
            .iter()
            .filter(|source| source.property == property)
            .cloned()
            .collect::<Vec<_>>();
        let image_no_gc = sources.iter().any(|source| {
            matches!(
                source.backing,
                Some(hir::PropertyBacking::TopLevelGlobal {
                    initialization: hir::TopLevelInitialization::Image,
                    ..
                })
            )
        }) && self.is_gc_free(ty);
        for source in sources {
            if image_no_gc && source.body_kind == PropertyAccessorBodyKind::Storage {
                self.functions[source.function].attributes.gc_effect = hir::GcEffect::NoGc;
                self.signatures
                    .get_mut(&source.function)
                    .expect("resolved accessor")
                    .attributes
                    .gc_effect = hir::GcEffect::NoGc;
                let capability = self.properties[property].capability;
                match source.kind {
                    PropertyAccessorKind::Getter => {
                        self.property_getters[capability.getter()]
                            .attributes
                            .gc_effect = hir::GcEffect::NoGc
                    }
                    PropertyAccessorKind::Setter => {
                        self.property_setters[capability.setter().expect("mutable property")]
                            .attributes
                            .gc_effect = hir::GcEffect::NoGc
                    }
                }
            }
            let target = self.signatures[&source.function].type_params.clone();
            let bindings = parameters
                .iter()
                .zip(target)
                .map(|(from, to)| (from.id, self.intern_type(hir::Type::Param(to.id))))
                .collect::<Vec<_>>();
            let result = self.instantiate_method_ty(ty, &bindings);
            let signature = self
                .signatures
                .get_mut(&source.function)
                .expect("resolved accessor");
            signature.result_pending = false;
            signature.parameter_pending = false;
            match source.kind {
                PropertyAccessorKind::Getter => {
                    signature.return_ty = result;
                    self.functions[source.function].return_ty = result;
                }
                PropertyAccessorKind::Setter => {
                    signature
                        .params
                        .last_mut()
                        .expect("a setter has a value parameter")
                        .ty = result;
                }
            }
        }
    }

    pub(super) fn property_type_from_accessor(
        &mut self,
        property: hir::PropertyId,
        function: FunctionId,
    ) -> TypeId {
        let parameters = self.signatures[&function].type_params.clone();
        let targets = self.property_type_parameters(property);
        let bindings = parameters
            .iter()
            .zip(targets)
            .map(|(from, to)| (from.id, self.intern_type(hir::Type::Param(to.id))))
            .collect::<Vec<_>>();
        self.instantiate_method_ty(self.signatures[&function].return_ty, &bindings)
    }
}
