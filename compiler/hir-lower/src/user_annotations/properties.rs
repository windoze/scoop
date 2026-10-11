use super::*;
use ast::AnnotationUseSiteTarget as Site;
use hir::{
    AnnotationStorageRole as Storage, SourceAnnotationCallable as Callable,
    SourceAnnotationReceiver as Receiver, SourceAnnotationTarget as Target,
};

impl Lowerer {
    pub(crate) fn annotate_property_declaration(
        &mut self,
        declaration: &ast::PropertyDecl,
        file: usize,
    ) {
        let Some(property) = self.properties.iter().find_map(|(id, p)| {
            (p.span == declaration.span && self.property_source_file(id) == Some(file))
                .then_some(id)
        }) else {
            return;
        };
        let saved_parameters = self.type_params_in_scope.clone();
        if let hir::PropertyOwner::Extension(extension) = self.properties[property].owner {
            self.type_params_in_scope = self.extension_properties[extension].type_params.clone();
        }
        self.annotate_property_targets(property, declaration);
        self.type_params_in_scope = saved_parameters;
    }

    fn annotate_property_targets(
        &mut self,
        property: hir::PropertyId,
        declaration: &ast::PropertyDecl,
    ) {
        self.bind_source_annotations(Target::Property(property), &declaration.annotations);
        let accessors = match &declaration.body {
            ast::PropertyBodySyntax::Initializer { accessors, .. }
            | ast::PropertyBodySyntax::Computed(accessors) => accessors,
            _ => return,
        };
        let capability = self.properties[property].capability;
        if let Some(getter) = &accessors.getter {
            self.bind_source_annotations(
                Target::Callable(Callable::Getter(capability.getter())),
                &getter.annotations,
            );
        }
        if let Some(setter) = &accessors.setter
            && let Some(id) = capability.setter()
        {
            self.bind_source_annotations(
                Target::Callable(Callable::Setter(id)),
                &setter.annotations,
            );
        }
    }

    pub(super) fn property_annotation_site(
        &self,
        property: hir::PropertyId,
        site: Site,
    ) -> Option<Target> {
        let declaration = &self.properties[property];
        let capability = declaration.capability;
        match site {
            Site::Property => Some(Target::Property(property)),
            Site::Get => Some(Target::Callable(Callable::Getter(capability.getter()))),
            Site::Set => capability
                .setter()
                .map(|id| Target::Callable(Callable::Setter(id))),
            Site::SetParam => capability.setter().map(|id| Target::Parameter {
                callable: Callable::Setter(id),
                index: 0,
            }),
            Site::Receiver if !matches!(declaration.owner, hir::PropertyOwner::TopLevel) => {
                Some(Target::Receiver(Receiver::Property(property)))
            }
            Site::Field => match &declaration.representation {
                hir::PropertyRepresentation::Stored(storage) => match storage.backing {
                    hir::PropertyBacking::StructField { owner, index } => {
                        hir::StructFieldRef::checked(&self.structs, owner, index).map(Target::Field)
                    }
                    hir::PropertyBacking::ClassField { field, .. } => {
                        Some(Target::ClassField(field))
                    }
                    hir::PropertyBacking::TopLevelGlobal { .. } => Some(Target::PropertyStorage {
                        property,
                        role: Storage::Backing,
                    }),
                },
                hir::PropertyRepresentation::NativeStorage { .. } => {
                    Some(Target::PropertyStorage {
                        property,
                        role: Storage::Backing,
                    })
                }
                _ => None,
            },
            Site::Delegate => match declaration.representation {
                hir::PropertyRepresentation::Delegated { storage } => {
                    match self.delegate_storages[storage].location {
                        hir::DelegateStorageLocation::ClassField(field) => {
                            Some(Target::ClassField(field))
                        }
                        hir::DelegateStorageLocation::ManagedGlobal(_) => {
                            Some(Target::PropertyStorage {
                                property,
                                role: Storage::Delegate,
                            })
                        }
                    }
                }
                hir::PropertyRepresentation::GenericDelegated { .. } => {
                    Some(Target::PropertyStorage {
                        property,
                        role: Storage::Delegate,
                    })
                }
                _ => None,
            },
            Site::Param => match &declaration.representation {
                hir::PropertyRepresentation::Stored(hir::StoredProperty {
                    backing:
                        hir::PropertyBacking::ClassField {
                            initializer: hir::ClassPropertyInitializer::PrimaryParameter(parameter),
                            ..
                        },
                }) => self
                    .class_constructors
                    .iter()
                    .find_map(|(id, constructor)| {
                        constructor
                            .parameters
                            .iter()
                            .position(|p| p.id == *parameter)
                            .map(|index| Target::Parameter {
                                callable: Callable::ClassConstructor(id),
                                index: index as u32,
                            })
                    }),
                hir::PropertyRepresentation::Stored(hir::StoredProperty {
                    backing: hir::PropertyBacking::StructField { owner, index },
                }) => self.structs[*owner]
                    .constructors
                    .iter()
                    .copied()
                    .find(|id| {
                        matches!(
                            self.struct_constructors[*id].kind,
                            hir::StructConstructorKind::Primary
                        )
                    })
                    .map(|id| Target::Parameter {
                        callable: Callable::StructConstructor(id),
                        index: *index,
                    }),
                _ => None,
            },
            Site::Receiver => None,
        }
    }
}
