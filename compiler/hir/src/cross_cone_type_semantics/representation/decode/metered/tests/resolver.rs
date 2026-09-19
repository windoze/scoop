use super::*;

pub(super) struct Counting<'a> {
    pub inner: &'a mut Fixture,
    pub calls: Vec<&'static str>,
    pub source_override: Option<Arc<SourceDeclarationKey>>,
    pub extra_source: Option<Arc<SourceDeclarationKey>>,
    pub field_override: Option<Arc<FieldIdentityKey>>,
}
impl<'a> Counting<'a> {
    pub fn new(inner: &'a mut Fixture) -> Self {
        Self {
            inner,
            calls: vec![],
            source_override: None,
            extra_source: None,
            field_override: None,
        }
    }
}
macro_rules! identity {
    ($id:ty, $name:literal) => {
        impl PersistentIdResolver<$id> for Counting<'_> {
            type Error = &'static str;
            fn resolve(&mut self, id: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                self.calls.push($name);
                self.inner.resolve(id)
            }
        }
    };
}
identity!(ConeIdentity, "cone");
identity!(PersistentTypeId, "type");
identity!(PersistentGenericTypeId, "generic type");
macro_rules! key {
    ($id:ty, $key:ty, $name:literal) => {
        impl PersistentKeyResolver<$id, $key> for Counting<'_> {
            type Error = &'static str;
            fn resolve_key(
                &mut self,
                id: DecodedPersistentId<$id>,
            ) -> Result<Arc<$key>, Self::Error> {
                self.calls.push($name);
                self.inner.resolve_key(id)
            }
        }
    };
}
key!(PersistentSourceContextId, SourceContextKey, "context");
key!(PersistentEnumVariantId, EnumVariantIdentityKey, "variant");
key!(
    PersistentEnumVariantFieldId,
    EnumVariantFieldKey,
    "enum field"
);
impl PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey> for Counting<'_> {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<Arc<SourceDeclarationKey>, Self::Error> {
        self.calls.push("source key");
        if let Some(key) = &self.source_override {
            return Ok(Arc::clone(key));
        }
        if let Some(key) = &self.extra_source
            && id
                .verify(PersistentTypeId::from_source_declaration(key).unwrap())
                .is_ok()
        {
            return Ok(Arc::clone(key));
        }
        self.inner.resolve_key(id)
    }
}
impl PersistentKeyResolver<PersistentFieldId, FieldIdentityKey> for Counting<'_> {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentFieldId>,
    ) -> Result<Arc<FieldIdentityKey>, Self::Error> {
        self.calls.push("field");
        if let Some(key) = &self.field_override {
            return Ok(Arc::clone(key));
        }
        self.inner.resolve_key(id)
    }
}
