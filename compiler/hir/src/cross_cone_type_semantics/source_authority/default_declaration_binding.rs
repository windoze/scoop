//! Default declaration contracts from artifact-bound complete nominal sources.
use crate::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

mod contracts;
mod data_flow;
mod envelope;
mod errors;
mod sources;
pub use errors::*;
type Error = DefaultSourceDeclarationBindingError;

/// Source declaration, location, type and local data-flow contracts. Override
/// uniqueness, complete inherited substitution, body operations and access still require replay.
/// Every local and body type is checked in the original provider scope,
/// followed by complete local data-flow and binding-schedule validation.
#[derive(Debug)]
pub struct BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f> {
    origins: BoundNominalDefaultOriginsV1<'d, 'p, 's, 'a, 'f>,
    declarations: Vec<DefaultSourceDeclaredContractV1<'d>>,
}
#[derive(Debug)]
pub struct DefaultSourceDeclaredContractV1<'s> {
    key: ProtectedDefaultTemplateKeyV1,
    definition_root: PersistentLexicalRootV1,
    owner: &'s NominalSourceCallablePayloadV1,
    provider: &'s NominalSourceCallablePayloadV1,
    owner_binders: DefaultTemplateProviderShapeV1,
    provider_binders: DefaultTemplateProviderShapeV1,
    provider_parameter: DefaultTemplateProviderParameterV1<'s>,
    provider_receiver: Option<SignatureTypeKey>,
}
impl<'p, 's, 'a, 'f> BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
    pub fn bind_default_declarations<'d>(
        &'d self,
        templates: &'d CanonicalDefaultSourceTemplatesV1,
        dependencies: &[&'d BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f>],
        meter: &mut BudgetMeter,
    ) -> Result<BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f>, Error> {
        let path = WirePath::root();
        let mut foundations = Vec::new();
        meter.try_reserve_collection_slots(&mut foundations, dependencies.len(), &path)?;
        meter.charge_work(dependencies.len() as u64, &path)?;
        foundations.extend(dependencies.iter().map(|p| p.members().nominals.foundation));
        // Reuse the complete location transaction with exactly these artifacts.
        // It also enforces dependency order and one shared identity graph.
        let origins = self.bind_default_origins(templates, &foundations, meter)?;
        let mut declarations = Vec::new();
        meter.try_reserve_collection_slots(&mut declarations, templates.records().len(), &path)?;
        for (index, template) in templates.records().iter().enumerate() {
            let path = path.clone().index(index as u64);
            meter.charge_nodes(1, &path)?;
            let provider = sources::provider(
                self,
                dependencies,
                template.definition_origin().origin().source().cone(),
                meter,
                &path,
            )?;
            let contract = (|| {
                let contract = contracts::validate(self, provider, template, meter, &path)?;
                data_flow::validate(self, dependencies, template, meter, &path)?;
                Ok(contract)
            })()
            .map_err(|error| Error::Record {
                key: template.key(),
                error: Box::new(error),
            })?;
            declarations.push(contract);
        }
        Ok(BoundNominalDefaultDeclarationsV1 {
            origins,
            declarations,
        })
    }
}
impl<'d, 'p, 's, 'a, 'f> BoundNominalDefaultDeclarationsV1<'d, 'p, 's, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.origins.provider()
    }
    pub const fn origins(&self) -> &BoundNominalDefaultOriginsV1<'d, 'p, 's, 'a, 'f> {
        &self.origins
    }
    pub fn declarations(&self) -> &[DefaultSourceDeclaredContractV1<'d>] {
        &self.declarations
    }
    pub fn declaration(
        &self,
        key: ProtectedDefaultTemplateKeyV1,
        meter: &mut BudgetMeter,
    ) -> Result<&DefaultSourceDeclaredContractV1<'d>, Error> {
        sources::query(self.declarations.len(), meter, &WirePath::root())?;
        self.declarations
            .binary_search_by_key(&key, |record| record.key)
            .map(|index| &self.declarations[index])
            .map_err(|_| Error::MissingTemplate(key))
    }
}
impl<'s> DefaultSourceDeclaredContractV1<'s> {
    pub const fn key(&self) -> ProtectedDefaultTemplateKeyV1 {
        self.key
    }
    pub const fn definition_root(&self) -> PersistentLexicalRootV1 {
        self.definition_root
    }
    pub const fn owner(&self) -> &'s NominalSourceCallablePayloadV1 {
        self.owner
    }
    pub const fn provider(&self) -> &'s NominalSourceCallablePayloadV1 {
        self.provider
    }
    pub const fn owner_binders(&self) -> DefaultTemplateProviderShapeV1 {
        self.owner_binders
    }
    pub const fn provider_binders(&self) -> DefaultTemplateProviderShapeV1 {
        self.provider_binders
    }
    pub const fn provider_parameter(&self) -> DefaultTemplateProviderParameterV1<'s> {
        self.provider_parameter
    }
    pub const fn provider_receiver(&self) -> Option<&SignatureTypeKey> {
        self.provider_receiver.as_ref()
    }
}
