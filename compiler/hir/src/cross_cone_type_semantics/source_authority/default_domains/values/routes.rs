use super::*;
use scoop_identity::{
    EnumVariantIdentityKey, FieldIdentityKey, FieldIdentityView, GeneratedCallableKey,
    GeneratedNominalKey, PersistentId,
};
use std::sync::Arc;

impl<'b, 's, 'a, 'f> DefaultSourceDomainsV1<'b, 's, 'a, 'f> {
    pub(super) fn value_provider(
        &self,
        target: Target<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'b Declarations<'s, 'a, 'f>, Error> {
        let provider = match target {
            Target::Global(id) => self.source_provider(id, meter, path)?,
            Target::Singleton(id) => self.source_provider(id, meter, path)?,
            Target::Constructor(DefaultConstructorRefV1::Struct { declaration, .. }) => {
                self.source_provider(*declaration, meter, path)?
            }
            Target::Constructor(DefaultConstructorRefV1::Class { declaration, .. }) => {
                let declaration = match declaration {
                    DefaultClassConstructorIdV1::Source(id) => *id,
                    DefaultClassConstructorIdV1::Generated(id) => {
                        let key = self.value_key::<_, GeneratedCallableKey>(*id, meter, path)?;
                        let GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } =
                            key.as_ref()
                        else {
                            return Err(Error::target(
                                DefaultSourceTargetSubjectError::AdapterRole(*id),
                            ));
                        };
                        *constructor
                    }
                };
                self.source_provider(declaration, meter, path)?
            }
            Target::Constructor(DefaultConstructorRefV1::Variant { declaration, .. }) => {
                let key = self.value_key::<_, EnumVariantIdentityKey>(*declaration, meter, path)?;
                let owner = key.source_owner().ok_or_else(|| {
                    Error::target(DefaultSourceTargetSubjectError::Role(
                        DefaultSourceIndirectTargetV1::EnumVariant(*declaration),
                    ))
                })?;
                self.value_nominal_provider(owner, meter, path)?
            }
            Target::Field(field) => self.field_provider(field, meter, path)?,
        };
        self.provider(provider, meter, path)
    }

    fn field_provider(
        &self,
        field: &DefaultFieldRefV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<ConeIdentity, Error> {
        let (id, target) = match field {
            DefaultFieldRefV1::Struct { declaration, .. } => (
                *declaration,
                DefaultSourceIndirectTargetV1::StructField(*declaration),
            ),
            DefaultFieldRefV1::Class { declaration, .. } => (
                *declaration,
                DefaultSourceIndirectTargetV1::ClassField(*declaration),
            ),
            DefaultFieldRefV1::Tuple { .. } => return Ok(self.current.provider()),
        };
        let key = self.value_key::<_, FieldIdentityKey>(id, meter, path)?;
        let owner = match key.view() {
            FieldIdentityView::SourceDeclared { owner, .. }
            | FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => owner,
            FieldIdentityView::Generated { owner, .. } => {
                let key = self.value_key::<_, GeneratedNominalKey>(owner, meter, path)?;
                let GeneratedNominalKey::ObjectBackingClass { object } = key.as_ref() else {
                    return Err(Error::target(DefaultSourceTargetSubjectError::Role(target)));
                };
                SourceNominalId::Concrete(*object)
            }
        };
        self.value_nominal_provider(owner, meter, path)
    }

    fn value_nominal_provider(
        &self,
        owner: SourceNominalId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<ConeIdentity, Error> {
        match owner {
            SourceNominalId::Concrete(id) => self.source_provider(id, meter, path),
            SourceNominalId::GenericTemplate(id) => self.source_provider(id, meter, path),
        }
    }

    fn source_provider<I: PersistentId + 'static>(
        &self,
        id: I,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<ConeIdentity, Error> {
        let key = self.value_key::<_, SourceDeclarationKey>(id, meter, path)?;
        NominalRepresentationSupportV1::charge_source_key_resources(&key, meter, path)?;
        Ok(key.origin())
    }

    fn value_key<I: PersistentId + 'static, K: Send + Sync + 'static>(
        &self,
        id: I,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Arc<K>, Error> {
        let identities = self.current.foundation.identities;
        meter.charge_edges(1, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_work(
            (u64::from(identities.identity_count().max(1).ilog2()) + 1) * 65,
            path,
        )?;
        identities
            .canonical_key(id)
            .map_err(|error| Error::Identity(error.to_string()))
    }
}
