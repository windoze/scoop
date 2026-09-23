use super::*;
use std::sync::Arc;

impl DefaultSourceValueTargetV1<'_> {
    /// Routes by typed identity before querying a provider's actual foundation.
    /// This does not validate the target's declaration or its access domain.
    pub fn source_provider(
        self,
        current: ConeIdentity,
        identities: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<ConeIdentity, Error> {
        let mut route = Route {
            identities,
            meter,
            path,
        };
        match self {
            Self::Global(id) => route.source(id),
            Self::Singleton(id) => route.source(id),
            Self::Constructor(DefaultConstructorRefV1::Struct { declaration, .. }) => {
                route.source(*declaration)
            }
            Self::Constructor(DefaultConstructorRefV1::Class { declaration, .. }) => {
                let declaration = match declaration {
                    DefaultClassConstructorIdV1::Source(id) => *id,
                    DefaultClassConstructorIdV1::Generated(id) => {
                        let key = route.key::<_, GeneratedCallableKey>(*id)?;
                        let GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } =
                            key.as_ref()
                        else {
                            return Err(Error::AdapterRole(*id));
                        };
                        *constructor
                    }
                };
                route.source(declaration)
            }
            Self::Constructor(DefaultConstructorRefV1::Variant { declaration, .. }) => {
                let key = route.key::<_, EnumVariantIdentityKey>(*declaration)?;
                let owner = key
                    .source_owner()
                    .ok_or(Error::Role(Target::EnumVariant(*declaration)))?;
                route.nominal(owner)
            }
            Self::Field(DefaultFieldRefV1::Tuple { .. }) => Ok(current),
            Self::Field(DefaultFieldRefV1::Struct { declaration, .. }) => {
                route.field(*declaration, Target::StructField(*declaration))
            }
            Self::Field(DefaultFieldRefV1::Class { declaration, .. }) => {
                route.field(*declaration, Target::ClassField(*declaration))
            }
        }
    }
}

struct Route<'a, 'm> {
    identities: &'a ValidatedIdentityGraph,
    meter: &'m mut BudgetMeter,
    path: &'m WirePath,
}

impl Route<'_, '_> {
    fn field(&mut self, id: PersistentFieldId, target: Target) -> Result<ConeIdentity, Error> {
        let key = self.key::<_, FieldIdentityKey>(id)?;
        let owner = match key.view() {
            FieldIdentityView::SourceDeclared { owner, .. }
            | FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => owner,
            FieldIdentityView::Generated { owner, .. } => {
                let key = self.key::<_, GeneratedNominalKey>(owner)?;
                let GeneratedNominalKey::ObjectBackingClass { object } = key.as_ref() else {
                    return Err(Error::Role(target));
                };
                SourceNominalId::Concrete(*object)
            }
        };
        self.nominal(owner)
    }

    fn nominal(&mut self, owner: SourceNominalId) -> Result<ConeIdentity, Error> {
        match owner {
            SourceNominalId::Concrete(id) => self.source(id),
            SourceNominalId::GenericTemplate(id) => self.source(id),
        }
    }

    fn source<I: PersistentId + 'static>(&mut self, id: I) -> Result<ConeIdentity, Error> {
        let key = self.key::<_, SourceDeclarationKey>(id)?;
        NominalRepresentationSupportV1::charge_source_key_resources(&key, self.meter, self.path)?;
        Ok(key.origin())
    }

    fn key<I: PersistentId + 'static, K: Send + Sync + 'static>(
        &mut self,
        id: I,
    ) -> Result<Arc<K>, Error> {
        self.meter.charge_edges(1, self.path)?;
        self.meter.charge_nodes(1, self.path)?;
        self.meter.charge_work(
            (u64::from(self.identities.identity_count().max(1).ilog2()) + 1) * 65,
            self.path,
        )?;
        self.identities
            .canonical_key(id)
            .map_err(Error::IdentityLookup)
    }
}
