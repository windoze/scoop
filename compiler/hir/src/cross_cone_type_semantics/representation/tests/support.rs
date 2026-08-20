use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DecodedPersistentId,
    DefinitionOrigin, DefinitionOwnerChain, EnumVariantFieldKey, EnumVariantFieldSelector,
    EnumVariantIdentityKey, FieldIdentityKey, GeneratedNominalKey, NormalizedSourcePath,
    PackagePath, PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentKeyResolver, PersistentPropertyId,
    PersistentSourceContextId, PersistentTypeId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};

use crate::{
    ClassRepresentationFieldV1, DeclarationAccessSourceV1, DeclaredVisibilityV1,
    EnumRepresentationFieldV1, EnumRepresentationVariantV1, ExactTypeGcV1,
    ExportDefinitionSourceV1, StructRepresentationFieldV1,
};

pub(in crate::cross_cone_type_semantics::representation) struct Fixture {
    pub key: SourceDeclarationKey,
    pub access: DeclarationAccessSourceV1,
    context: SourceContextKey,
    types: BTreeSet<PersistentTypeId>,
    fields: BTreeMap<PersistentFieldId, Arc<FieldIdentityKey>>,
    variants: BTreeMap<PersistentEnumVariantId, Arc<EnumVariantIdentityKey>>,
    enum_fields: BTreeMap<PersistentEnumVariantFieldId, Arc<EnumVariantFieldKey>>,
}

pub(super) fn source_key(kind: SourceNominalKind, arity: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        site(),
        CanonicalIdentifier::new("Subject").unwrap(),
        kind,
        arity,
    )
}
fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
pub(super) fn unit() -> SignatureTypeKey {
    SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
}

impl Fixture {
    pub fn new(kind: SourceNominalKind) -> Self {
        let key = source_key(kind, 0);
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("types.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let origin = ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(0, 10).unwrap(), &context).unwrap(),
        );
        let access =
            DeclarationAccessSourceV1::try_new(DeclaredVisibilityV1::Public, vec![], origin)
                .unwrap();
        let types = BTreeSet::from([
            PersistentTypeId::from_source_declaration(&key).unwrap(),
            CoreBuiltinNominal::Unit.identity_record().id(),
        ]);
        Self {
            key,
            access,
            context,
            types,
            fields: BTreeMap::new(),
            variants: BTreeMap::new(),
            enum_fields: BTreeMap::new(),
        }
    }
    pub fn owner(&self) -> PersistentTypeId {
        PersistentTypeId::from_source_declaration(&self.key).unwrap()
    }
    pub fn struct_field(
        &mut self,
        name: &str,
        ty: SignatureTypeKey,
    ) -> StructRepresentationFieldV1 {
        let key =
            FieldIdentityKey::source_declared(&self.key, CanonicalIdentifier::new(name).unwrap())
                .unwrap();
        let field = StructRepresentationFieldV1::try_new(&key, ty).unwrap();
        self.fields.insert(field.field(), Arc::new(key));
        field
    }
    pub fn class_field(&mut self, name: &str) -> ClassRepresentationFieldV1 {
        let property = PersistentPropertyId::from_source_declaration(
            &SourceDeclarationKey::property(site(), CanonicalIdentifier::new(name).unwrap()),
        )
        .unwrap();
        let key = if self.key.declaration_kind() == scoop_identity::SourceDeclarationKind::Object {
            FieldIdentityKey::object_backing_property(
                &GeneratedNominalKey::ObjectBackingClass {
                    object: self.owner(),
                },
                property,
            )
            .unwrap()
        } else {
            FieldIdentityKey::source_property_backing(&self.key, property).unwrap()
        };
        let field = ClassRepresentationFieldV1::try_new(&key, unit()).unwrap();
        self.fields.insert(field.field(), Arc::new(key));
        field
    }
    pub fn backing(&mut self) -> PersistentTypeId {
        let backing =
            PersistentTypeId::from_generated_key(&GeneratedNominalKey::ObjectBackingClass {
                object: self.owner(),
            })
            .unwrap();
        self.types.insert(backing);
        backing
    }
    pub fn variant(&mut self, name: &str, payload: bool) -> EnumRepresentationVariantV1 {
        let key =
            EnumVariantIdentityKey::source(&self.key, CanonicalIdentifier::new(name).unwrap())
                .unwrap();
        let variant = PersistentEnumVariantId::from_key(&key).unwrap();
        let fields = if payload {
            let field_key = EnumVariantFieldKey::new(
                variant,
                EnumVariantFieldSelector::Positional {
                    declaration_index: 0,
                },
            );
            let field = EnumRepresentationFieldV1::try_new(&field_key, unit()).unwrap();
            self.enum_fields.insert(field.field(), Arc::new(field_key));
            vec![field]
        } else {
            vec![]
        };
        let result =
            EnumRepresentationVariantV1::try_new(&key, fields, ExactTypeGcV1::GcFree).unwrap();
        self.variants.insert(variant, Arc::new(key));
        result
    }
}

impl PersistentIdResolver<ConeIdentity> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE).map_err(|_| "unknown Cone")
    }
}
impl PersistentIdResolver<PersistentTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        self.types
            .iter()
            .copied()
            .find(|known| id.as_array() == known.as_array())
            .ok_or("unknown type")
    }
}
impl PersistentIdResolver<PersistentGenericTypeId> for Fixture {
    type Error = &'static str;
    fn resolve(
        &mut self,
        _: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err("unknown generic type")
    }
}
impl PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey> for Fixture {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<Arc<SourceDeclarationKey>, Self::Error> {
        id.verify(self.owner())
            .map_err(|_| "unknown source owner")?;
        Ok(Arc::new(self.key.clone()))
    }
}
impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Fixture {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        id.verify(PersistentSourceContextId::from_key(&self.context).unwrap())
            .map_err(|_| "unknown source context")?;
        Ok(Arc::new(self.context.clone()))
    }
}
macro_rules! key_resolver {
    ($id:ty, $key:ty, $map:ident) => {
        impl PersistentKeyResolver<$id, $key> for Fixture {
            type Error = &'static str;
            fn resolve_key(
                &mut self,
                id: DecodedPersistentId<$id>,
            ) -> Result<Arc<$key>, Self::Error> {
                self.$map
                    .iter()
                    .find(|(known, _)| id.as_array() == known.as_array())
                    .map(|(_, key)| Arc::clone(key))
                    .ok_or("unknown representation key")
            }
        }
    };
}
key_resolver!(PersistentFieldId, FieldIdentityKey, fields);
key_resolver!(PersistentEnumVariantId, EnumVariantIdentityKey, variants);
key_resolver!(
    PersistentEnumVariantFieldId,
    EnumVariantFieldKey,
    enum_fields
);
