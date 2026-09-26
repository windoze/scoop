use super::*;

pub(super) fn two_records<T: scoop_wire::WireEncode>(left: &T, right: &T) -> Vec<u8> {
    [vec![0x82], encode(left).unwrap(), encode(right).unwrap()].concat()
}

pub(super) struct Resolver {
    pub reject_references: bool,
    concrete: [PersistentTypeId; 2],
    generic: PersistentGenericTypeId,
}

impl Resolver {
    pub fn new() -> Self {
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
        Self {
            reject_references: false,
            concrete,
            generic,
        }
    }

    pub fn roots(&self) -> Vec<SourceNominalId> {
        vec![
            SourceNominalId::Concrete(self.concrete[0]),
            SourceNominalId::Concrete(self.concrete[1]),
            SourceNominalId::GenericTemplate(self.generic),
        ]
    }

    fn verify<I: PersistentId>(
        &mut self,
        id: DecodedPersistentId<I>,
        known: &[I],
    ) -> Result<I, &'static str> {
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
