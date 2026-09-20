use super::*;
use scoop_identity::DefinitionOrigin;
use std::sync::Arc;

pub(super) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

pub(super) fn two_records<T: scoop_wire::WireEncode>(left: &T, right: &T) -> Vec<u8> {
    [vec![0x82], encode(left).unwrap(), encode(right).unwrap()].concat()
}

pub(super) struct Resolver {
    pub queries: usize,
    pub reject_references: bool,
    concrete: [PersistentTypeId; 2],
    generic: PersistentGenericTypeId,
    exacts: [PersistentExactTypeId; 2],
    context: SourceContextKey,
    origin: ExportDefinitionSourceV1,
}

impl Resolver {
    pub fn new() -> Self {
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("inventory.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let origin = ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(0, 20).unwrap(), &context).unwrap(),
        );
        let mut concrete = [
            CoreBuiltinNominal::Unit.identity_record().id(),
            CoreBuiltinNominal::Any.identity_record().id(),
        ];
        concrete.sort_unstable();
        let site = SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let generic =
            PersistentGenericTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
                site,
                CanonicalIdentifier::new("Generic").unwrap(),
                SourceNominalKind::Struct,
                1,
            ))
            .unwrap();
        let exacts =
            concrete.map(|id| PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(id)).unwrap());
        Self {
            queries: 0,
            reject_references: false,
            concrete,
            generic,
            exacts,
            context,
            origin,
        }
    }

    pub fn roots(&self) -> Vec<SourceNominalId> {
        vec![
            SourceNominalId::Concrete(self.concrete[0]),
            SourceNominalId::Concrete(self.concrete[1]),
            SourceNominalId::GenericTemplate(self.generic),
        ]
    }

    pub fn snapshot(&self) -> TypeSourceNominalV1 {
        TypeSourceNominalV1::new(
            self.roots()[2],
            DeclarationAccessSourceV1::try_new(
                DeclaredVisibilityV1::Protected,
                vec![self.roots()[0]],
                self.origin.clone(),
            )
            .unwrap(),
        )
    }

    pub fn dependencies(&self) -> Vec<TypeSectionDependencyFactV1> {
        vec![
            TypeSectionDependencyFactV1 {
                provider: ConeIdentity::CORE,
                exact: self.exacts[0],
            },
            TypeSectionDependencyFactV1 {
                provider: ConeIdentity::SINGLE_FILE,
                exact: self.exacts[1],
            },
        ]
    }

    pub fn edge(&self) -> NominalInheritanceEdgesV1 {
        NominalInheritanceEdgesV1::try_new(
            self.exacts[0],
            NominalInheritanceModalityV1::Open,
            DirectClassBaseV1::ClassBase {
                exact: self.exacts[1],
            },
            vec![self.exacts[1]],
        )
        .unwrap()
    }

    fn verify<I: PersistentId>(
        &mut self,
        id: DecodedPersistentId<I>,
        known: &[I],
    ) -> Result<I, &'static str> {
        self.queries += 1;
        if self.reject_references {
            return Err("rejected identity reference");
        }
        known
            .iter()
            .copied()
            .find(|value| id.verify(*value).is_ok())
            .ok_or("unknown identity reference")
    }
}

impl PersistentIdResolver<ConeIdentity> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        self.verify(id, &[ConeIdentity::CORE, ConeIdentity::SINGLE_FILE])
    }
}
impl PersistentIdResolver<PersistentTypeId> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        let known = self.concrete;
        self.verify(id, &known)
    }
}
impl PersistentIdResolver<PersistentGenericTypeId> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        self.verify(id, &[self.generic])
    }
}
impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        let known = self.exacts;
        self.verify(id, &known)
    }
}
impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Resolver {
    type Error = &'static str;
    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        self.verify(
            id,
            &[PersistentSourceContextId::from_key(&self.context).unwrap()],
        )?;
        Ok(Arc::new(self.context.clone()))
    }
}
