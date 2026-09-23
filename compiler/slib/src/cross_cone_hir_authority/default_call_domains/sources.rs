use super::*;
use scoop_hir::{DefaultSourceIndirectTargetV1, OdrFreeHirFoundation};
use scoop_identity::DispatchDeclarationOwner;

impl<'a> Query<'_, 'a> {
    pub(super) fn source(
        &mut self,
        declaration: CallableTemplateOrigin,
        path: &WirePath,
    ) -> Result<Source<'a>, Error> {
        let provider = DefaultTargetIdentityQueriesV1::source_callable_provider(
            declaration,
            self.authority.identities,
            self.authority.meter,
            path,
        )?;
        let foundation = self.foundation(provider, path)?;
        let query =
            DefaultTargetIdentityQueriesV1::new(provider, foundation, self.authority.identities);
        let subject = match declaration {
            CallableTemplateOrigin::Function(id) => DefinitionOriginSubject::Function(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                DefinitionOriginSubject::GenericFunction(id)
            }
            CallableTemplateOrigin::Constructor(id) => DefinitionOriginSubject::Constructor(id),
            CallableTemplateOrigin::VariantConstructor(id) => query
                .default_indirect_access_subject(
                    DefaultSourceIndirectTargetV1::EnumVariant(id),
                    self.authority.meter,
                )?,
            CallableTemplateOrigin::Accessor(_) => return Err(Error::CallableRole(declaration)),
        };
        let key = query.source_declaration_key(subject, self.authority.meter)?;
        let interface = self.authority.provider_interface(provider)?;
        self.authority.meter.charge_work(
            u64::from(
                interface
                    .callable_interfaces()
                    .declaration_count()
                    .max(1)
                    .ilog2(),
            ) + 1,
            path,
        )?;
        let record = interface
            .callable_interfaces()
            .declaration(declaration)
            .ok_or(Error::MissingDeclaration(declaration))?;
        let expected_owner = match subject {
            DefinitionOriginSubject::Type(id) => {
                PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(id))
            }
            DefinitionOriginSubject::GenericType(id) => {
                PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(id))
            }
            _ => self.authority.source_key_owner("default callable", key)?,
        };
        if record.owner() != expected_owner {
            return Err(Error::CallableRole(declaration));
        }
        Ok(Source {
            declaration: record,
            key,
            subject,
        })
    }

    fn foundation(
        &mut self,
        provider: ConeIdentity,
        path: &WirePath,
    ) -> Result<&'a OdrFreeHirFoundation, Error> {
        self.authority
            .meter
            .charge_work(self.authority.dependencies.len() as u64 + 1, path)?;
        if provider == self.authority.current {
            return Ok(self.authority.current_foundation);
        }
        self.authority
            .dependencies
            .iter()
            .find(|entry| entry.identity == provider)
            .map(|entry| entry.foundation)
            .ok_or_else(|| {
                super::super::CrossConeHirNominalAuthorityError::UnreachableProvider {
                    origin: provider,
                }
                .into()
            })
    }

    pub(super) fn slot_key(
        &mut self,
        slot: PersistentDispatchSlotId,
        path: &WirePath,
    ) -> Result<DispatchSlotKey, Error> {
        self.authority.meter.charge_work(
            (u64::from(self.authority.identities.identity_count().max(1).ilog2()) + 1) * 65,
            path,
        )?;
        let key = self
            .authority
            .identities
            .canonical_key::<_, DispatchSlotKey>(slot)
            .map_err(scoop_hir::DefaultSourceTargetSubjectError::IdentityLookup)?;
        let declaration = match key.owner() {
            DispatchDeclarationOwner::Function(id) => CallableTemplateOrigin::Function(id),
            DispatchDeclarationOwner::Accessor(id) => CallableTemplateOrigin::Accessor(id),
        };
        let provider = DefaultTargetIdentityQueriesV1::source_callable_provider(
            declaration,
            self.authority.identities,
            self.authority.meter,
            path,
        )?;
        let foundation = self.foundation(provider, path)?;
        let query =
            DefaultTargetIdentityQueriesV1::new(provider, foundation, self.authority.identities);
        Ok(*query.source_dispatch_slot_key(slot, self.authority.meter, path)?)
    }
}
