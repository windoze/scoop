use crate::{DefaultTemplateProviderShapeV1, PersistentLexicalRootV1, SourceNominalId};
use scoop_identity::{NonEmptyVec, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

impl DefaultTemplateProviderShapeV1 {
    /// Reconstructs a source receiver from declared binders and a nominal owner.
    /// This does not establish a provider/override relation.
    pub fn nominal_source_receiver(
        self,
        root: PersistentLexicalRootV1,
        owner: SourceNominalId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Option<SignatureTypeKey>, DefaultNominalReceiverBuildError> {
        use DefaultNominalReceiverBuildError as Error;
        let arity = self.nominal_owner_binder_arity();
        meter.charge_work(1, path)?;
        if matches!(owner, SourceNominalId::Concrete(_)) != (arity == 0) {
            return Err(Error::OwnerShape);
        }
        if matches!(
            root,
            PersistentLexicalRootV1::Constructor(_)
                | PersistentLexicalRootV1::EnumVariantConstructor(_)
        ) {
            return Ok(None);
        }
        meter.check_semantic_depth(1, path)?;
        meter.charge_nodes(1, path)?;
        Ok(Some(match owner {
            SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
            SourceNominalId::GenericTemplate(origin) => {
                meter.check_table_entries(u64::from(arity), path)?;
                meter.check_semantic_depth(2, path)?;
                meter.charge_work(u64::from(arity), path)?;
                meter.charge_nodes(u64::from(arity), path)?;
                meter.charge_edges(u64::from(arity), path)?;
                let mut arguments = Vec::new();
                meter.try_reserve_collection_slots(&mut arguments, arity as usize, path)?;
                arguments.extend((0..arity).map(|index| SignatureTypeKey::Binder {
                    depth: u32::from(self.callable_own_binder_arity() != 0),
                    index,
                }));
                SignatureTypeKey::NominalApplication {
                    origin,
                    arguments: NonEmptyVec::new(arguments).map_err(|_| Error::OwnerShape)?,
                }
            }
        }))
    }
}
#[derive(Debug)]
pub enum DefaultNominalReceiverBuildError {
    Resource(WireError),
    OwnerShape,
}
impl From<WireError> for DefaultNominalReceiverBuildError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for DefaultNominalReceiverBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::OwnerShape => {
                f.write_str("default nominal receiver disagrees with the declared binder shape")
            }
        }
    }
}
impl std::error::Error for DefaultNominalReceiverBuildError {}
