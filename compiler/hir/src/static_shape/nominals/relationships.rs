use super::*;

impl<'a> StaticNominalShape<'a> {
    pub fn companion_host(self) -> Option<SourceNominalId> {
        match self.origin {
            StaticNominalOrigin::Current(NominalOwner::Object(id)) => {
                let ObjectKind::Companion(relation) = self.module.objects[id].kind else {
                    return None;
                };
                let ids = &self.module.nominal_identities;
                Some(match self.module.companion_relations[relation].host {
                    NominalOwner::Struct(id) => ids[id].declaration_id(),
                    NominalOwner::Enum(id) => ids[id].declaration_id(),
                    NominalOwner::Class(id) => ids[id].declaration_id(),
                    NominalOwner::Interface(id) => ids[id].declaration_id(),
                    NominalOwner::Object(id) => ids[id].declaration_id(),
                })
            }
            StaticNominalOrigin::Dependency(owner) if self.kind() == StaticNominalKind::Object => {
                let declaration = &self.module.loaded_class_definitions[&owner].declaration;
                let NominalSourceShapeV1::Object(shape) = declaration.interface.source_shape()
                else {
                    unreachable!()
                };
                if shape.object_kind() != ObjectSourceKindV1::Companion {
                    return None;
                }
                Some(
                    match declaration
                        .identity
                        .declaration()
                        .owners()
                        .owners()
                        .last()
                        .expect("a companion has a host")
                    {
                        scoop_identity::DefinitionOwnerAtom::Type(id) => {
                            SourceNominalId::Concrete(*id)
                        }
                        scoop_identity::DefinitionOwnerAtom::GenericType(id) => {
                            SourceNominalId::GenericTemplate(*id)
                        }
                        _ => unreachable!("a companion host is a nominal declaration"),
                    },
                )
            }
            _ => None,
        }
    }

    pub fn supertype_annotations(
        self,
        ty: StaticShapeTypeUse<'a>,
        world: &ImportedSemanticWorld<'a>,
    ) -> &'a [TypeAnnotationNode] {
        match self.origin {
            StaticNominalOrigin::Current(owner) => {
                let StaticShapeTypeSource::Hir(ty) = ty.source() else {
                    unreachable!()
                };
                self.module
                    .annotations
                    .type_sites
                    .iter()
                    .filter(|site| {
                        site.target == SourceAnnotationTarget::Nominal(owner)
                            && matches!(site.slot, AnnotationTypeSlot::Supertype(_))
                    })
                    .filter_map(|site| self.module.annotations.type_use(&site.type_use))
                    .find(|usage| usage.ty == AnnotationParameterType::Value(ty))
                    .map_or(&[], |usage| usage.nodes.as_slice())
            }
            StaticNominalOrigin::Dependency(owner) => {
                let annotations = world
                    .nominal_source_provider(owner)
                    .unwrap()
                    .source_interface()
                    .annotations();
                let target = match owner {
                    SourceNominalId::Concrete(id) => {
                        scoop_identity::NominalDeclarationOwner::Concrete(id)
                    }
                    SourceNominalId::GenericTemplate(id) => {
                        scoop_identity::NominalDeclarationOwner::GenericTemplate(id)
                    }
                };
                let binders = self
                    .parameters()
                    .iter()
                    .enumerate()
                    .map(|(index, p)| HirSignatureBinder {
                        parameter: p.id,
                        depth: 0,
                        index: index as u32,
                    })
                    .collect::<Vec<_>>();
                let signature = ty
                    .signature(self.module, &binders)
                    .expect("a bound supertype has a signature");
                annotations
                    .type_sites()
                    .iter()
                    .filter(|site| {
                        site.target == AnnotationTargetV1::Nominal(target)
                            && matches!(site.slot, AnnotationTypeSlot::Supertype(_))
                    })
                    .filter_map(|site| annotations.type_use(&site.type_use))
                    .find(|usage| usage.ty == AnnotationParameterType::Value(signature.clone()))
                    .map_or(&[], |usage| usage.nodes.as_slice())
            }
        }
    }
}
