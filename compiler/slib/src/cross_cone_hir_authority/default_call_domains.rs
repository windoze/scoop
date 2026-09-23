//! Reconstructs source call regions without trusting a reference's witness.
use std::{collections::BTreeMap, rc::Rc};

use scoop_hir::{
    CallableDeclarationRecordV1, CallableModalityV1, CanonicalBinderUseListV1,
    DeclaredVisibilityV1, DefaultTargetIdentityQueriesV1, DefaultTemplateProviderShapeV1,
    ExportDefaultTemplateV1, NominalInterfaceRecordV1, PublicDeclarationOwnerV1,
    PublicLookupAccessV1, PublicNominalKindV1, SourceAccessDomainV1, SourceNominalId,
};
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOriginSubject, DispatchSlotKey,
    PersistentDispatchSlotId, SignatureTypeKey, SourceDeclarationKey,
};
use scoop_wire::WirePath;

use super::CanonicalCrossConeHirSurfaceAuthority;
mod ancestors;
mod dispatch;
mod errors;
mod signatures;
mod sources;
mod witnesses;
pub use errors::CrossConeHirDefaultCallDomainError;
type Error = CrossConeHirDefaultCallDomainError;

#[derive(Clone, Copy)]
struct Source<'a> {
    declaration: &'a CallableDeclarationRecordV1,
    key: &'a SourceDeclarationKey,
    subject: DefinitionOriginSubject,
}

struct Inherited<'a> {
    source: Source<'a>,
    arguments: CanonicalBinderUseListV1,
    distance: usize,
    kind: PublicNominalKindV1,
}

struct Domains<'a> {
    direct: Rc<SourceAccessDomainV1>,
    slot: Option<Rc<SourceAccessDomainV1>>,
    inherited: Vec<Inherited<'a>>,
}

struct Query<'q, 'a> {
    authority: &'q mut CanonicalCrossConeHirSurfaceAuthority<'a>,
    completed: BTreeMap<CallableTemplateOrigin, Rc<Domains<'a>>>,
    active: Vec<CallableTemplateOrigin>,
}

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_call_domains(&mut self) -> Result<(), Error> {
        let templates = self.current_interface.default_templates().records();
        let path = WirePath::root().field(7);
        self.meter
            .check_table_entries(templates.len() as u64, &path)?;
        let mut query = Query {
            authority: self,
            completed: BTreeMap::new(),
            active: Vec::new(),
        };
        for (index, template) in templates.iter().enumerate() {
            query
                .validate_template(template, &path.clone().index(index as u64))
                .map_err(|source| Error::Template {
                    key: template.key(),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }
}

impl<'a> Query<'_, 'a> {
    fn resolve(
        &mut self,
        declaration: CallableTemplateOrigin,
        path: &WirePath,
    ) -> Result<Rc<Domains<'a>>, Error> {
        self.authority.meter.charge_work(
            (u64::from(self.completed.len().max(1).ilog2()) + self.active.len() as u64 + 1) * 65,
            path,
        )?;
        if let Some(domains) = self.completed.get(&declaration) {
            return Ok(Rc::clone(domains));
        }
        if self.active.contains(&declaration) {
            return Err(Error::CallableCycle(declaration));
        }
        self.authority
            .meter
            .check_semantic_depth(self.active.len() as u64 + 1, path)?;
        self.authority
            .meter
            .try_reserve_collection_slots(&mut self.active, 1, path)?;
        self.active.push(declaration);
        let result = self.build(declaration, path);
        self.active.pop();
        let domains = result.map_err(|source| Error::Declaration {
            declaration,
            source: Box::new(source),
        })?;
        self.authority.meter.charge_collection_slots(1, path)?;
        self.authority
            .meter
            .charge_owned_bytes(std::mem::size_of::<Domains<'a>>() as u64 + 16, path)?;
        let domains = Rc::new(domains);
        self.completed.insert(declaration, Rc::clone(&domains));
        Ok(domains)
    }

    fn build(
        &mut self,
        declaration: CallableTemplateOrigin,
        path: &WirePath,
    ) -> Result<Domains<'a>, Error> {
        let source = self.source(declaration, path)?;
        let direct =
            self.authority
                .source_declaration_access_domain(source.subject, source.key, path)?;
        let direct = self.share_domain(direct, path)?;
        let inherited = self.inherited(source, path)?;
        let slot = self.slot_domain(source, &direct, &inherited, path)?;
        let domains = Domains {
            direct,
            slot,
            inherited,
        };
        self.validate_public_lookup(source, &domains, path)?;
        Ok(domains)
    }

    fn share_domain(
        &mut self,
        domain: SourceAccessDomainV1,
        path: &WirePath,
    ) -> Result<Rc<SourceAccessDomainV1>, Error> {
        self.authority.meter.charge_owned_bytes(
            std::mem::size_of::<SourceAccessDomainV1>() as u64 + 16,
            path,
        )?;
        Ok(Rc::new(domain))
    }

    fn validate_public_lookup(
        &mut self,
        source: Source<'_>,
        domains: &Domains<'_>,
        path: &WirePath,
    ) -> Result<(), Error> {
        let interface = self.authority.provider_interface(source.key.origin())?;
        self.authority.meter.charge_work(
            u64::from(
                interface
                    .callable_interfaces()
                    .records()
                    .len()
                    .max(1)
                    .ilog2(),
            ) + 1,
            path,
        )?;
        let Some(public) = interface
            .callable_interfaces()
            .get(source.declaration.declaration())
        else {
            return Ok(());
        };
        let has_slot = public.access() == PublicLookupAccessV1::PublicSlot;
        if !domains.direct.is_universal()
            || has_slot != domains.slot.is_some()
            || domains
                .slot
                .as_ref()
                .is_some_and(|slot| !slot.is_universal())
        {
            return Err(Error::PublicLookup);
        }
        Ok(())
    }
}
