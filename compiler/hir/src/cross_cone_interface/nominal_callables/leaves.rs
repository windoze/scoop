use scoop_identity::{CoreBuiltinNominal, ExactTypeKey};
use scoop_wire::WirePath;

use super::{NominalExactLeafClassifierBuildError as Error, *};
use crate::{NominalInterfaceRecordV1, SourceNominalId};

impl NominalExactLeafClassifierV1 {
    pub fn try_from_nominal_interfaces<'a>(
        nominals: impl IntoIterator<Item = &'a NominalInterfaceRecordV1>,
    ) -> Result<Self, Error> {
        let path = &WirePath::root();
        let mut leaves = Vec::new();
        let mut generic_sources = Vec::new();
        for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
            append(&mut leaves, builtin.identity_record().id(), path)?;
        }
        for nominal in nominals {
            match nominal.declaration() {
                SourceNominalId::Concrete(source) => append(&mut leaves, source, path)?,
                SourceNominalId::GenericTemplate(source) => {
                    scoop_wire::allocation::try_reserve(&mut generic_sources, 1, path)
                        .map_err(Error::Resource)?;
                    generic_sources.push(source);
                }
            }
        }

        leaves.sort_unstable_by_key(|(source, _)| *source);
        leaves.dedup_by_key(|(source, _)| *source);
        generic_sources.sort_unstable();
        generic_sources.dedup();
        Ok(Self {
            leaves,
            generic_sources,
        })
    }
}

fn append(
    leaves: &mut Vec<(PersistentTypeId, PersistentExactTypeId)>,
    source: PersistentTypeId,

    path: &WirePath,
) -> Result<(), Error> {
    scoop_wire::allocation::try_reserve(leaves, 1, path).map_err(Error::Resource)?;
    let exact =
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source)).map_err(Error::Identity)?;
    leaves.push((source, exact));
    Ok(())
}
