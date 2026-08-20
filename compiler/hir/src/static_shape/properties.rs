use super::*;
use scoop_identity::{DeclarationName, PersistentFieldId, PersistentPropertyId, SignatureTypeKey};

#[derive(Clone, Copy)]
pub enum StaticPropertyDeclaration<'a> {
    Current {
        id: PropertyId,
        declaration: &'a Property,
    },
    Dependency(&'a PropertyDeclarationRecordV1),
}

#[derive(Clone, Copy)]
pub struct StaticPropertyShape<'a> {
    owner: StaticNominalShape<'a>,
    identity: PersistentPropertyId,
    name: &'a str,
    source: StaticPropertyDeclaration<'a>,
}

impl<'a> StaticPropertyShape<'a> {
    pub const fn identity(self) -> PersistentPropertyId {
        self.identity
    }
    pub const fn name(self) -> &'a str {
        self.name
    }
    pub const fn declaration(self) -> StaticPropertyDeclaration<'a> {
        self.source
    }

    pub fn value_type(
        self,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        match self.source {
            StaticPropertyDeclaration::Current { declaration, .. } => self
                .owner
                .type_use(declaration.ty)
                .signature(self.owner.module, binders),
            StaticPropertyDeclaration::Dependency(declaration) => self
                .owner
                .substitute_signature(declaration.value_type(), binders),
        }
    }

    /// Physical storage remains separate from the logical property. A
    /// computed or abstract property has no field, and a delegate is explicit.
    pub fn field_storage(self) -> Option<(PersistentFieldId, NominalFieldStorage)> {
        self.owner
            .fields()
            .find_map(|field| match (field.identity(), field.storage()) {
                (
                    StaticFieldIdentity::Field(id),
                    storage @ (NominalFieldStorage::PropertyBacking(property)
                    | NominalFieldStorage::PropertyDelegate(property)),
                ) if property == self.identity => Some((id, storage)),
                _ => None,
            })
    }

    pub fn annotations(self, dependencies: &ImportedSemanticWorld<'a>) -> StaticAnnotations<'a> {
        let target = match self.source {
            StaticPropertyDeclaration::Current { id, .. } => {
                StaticAnnotationTarget::Current(SourceAnnotationTarget::Property(id))
            }
            StaticPropertyDeclaration::Dependency(_) => {
                StaticAnnotationTarget::Dependency(AnnotationTargetV1::Property(self.identity))
            }
        };
        target.resolve(self.owner, dependencies)
    }
}

impl<'a> StaticNominalShape<'a> {
    pub fn properties(
        self,
        dependencies: &ImportedSemanticWorld<'a>,
    ) -> Vec<StaticPropertyShape<'a>> {
        match self.origin {
            StaticNominalOrigin::Current(owner) => {
                let properties = match owner {
                    NominalOwner::Struct(id) => &self.module.structs[id].properties,
                    NominalOwner::Enum(id) => &self.module.enums[id].properties,
                    NominalOwner::Class(id) => &self.module.classes[id].properties,
                    NominalOwner::Interface(id) => &self.module.interfaces[id].properties,
                    NominalOwner::Object(id) => {
                        &self.module.classes[self.module.objects[id].backing_class].properties
                    }
                };
                properties
                    .iter()
                    .map(|id| {
                        let declaration = &self.module.properties[*id];
                        StaticPropertyShape {
                            owner: self,
                            identity: self.module.property_identities[*id]
                                .ordinary_id()
                                .expect("nominal members are ordinary properties"),
                            name: &declaration.name,
                            source: StaticPropertyDeclaration::Current {
                                id: *id,
                                declaration,
                            },
                        }
                    })
                    .collect()
            }
            StaticNominalOrigin::Dependency(owner) => {
                self.dependency_properties(owner, dependencies)
            }
        }
    }

    fn dependency_properties(
        self,
        owner: SourceNominalId,
        dependencies: &ImportedSemanticWorld<'a>,
    ) -> Vec<StaticPropertyShape<'a>> {
        let provider = dependencies
            .nominal_source_provider(owner)
            .expect("a loaded nominal retains its source provider");
        let interface = provider.source_interface();
        let foundation = provider.source_foundation();
        let nominal = interface
            .nominal_interfaces()
            .declaration(owner)
            .expect("the provider was selected by this nominal");
        let mut properties = nominal
            .declaration_details()
            .members()
            .values()
            .iter()
            .filter_map(|member| match member {
                NestedSourceMemberRefV1::Property(id) => Some(*id),
                _ => None,
            })
            .map(|id| {
                let declaration = interface
                    .property_interfaces()
                    .declaration(scoop_identity::PropertyOwner::Property(id))
                    .expect("the checked nominal has complete member declarations");
                let (_, key) = foundation
                    .property_by_bytes(id.as_array())
                    .expect("a source property retains its key");
                let DeclarationName::Named(name) = key.name() else {
                    unreachable!("source properties have names")
                };
                let origin = foundation
                    .definition_origin(scoop_identity::DefinitionOriginSubject::Property(id))
                    .expect("a source property retains its origin");
                (
                    origin.origin().span().start_byte(),
                    StaticPropertyShape {
                        owner: self,
                        identity: id,
                        name: name.as_str(),
                        source: StaticPropertyDeclaration::Dependency(declaration),
                    },
                )
            })
            .collect::<Vec<_>>();
        properties.sort_by_key(|(position, _)| *position);
        properties
            .into_iter()
            .map(|(_, property)| property)
            .collect()
    }
}
