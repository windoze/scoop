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
        for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
            append(&mut leaves, builtin.identity_record().id(), path)?;
        }
        for nominal in nominals {
            if let SourceNominalId::Concrete(source) = nominal.declaration() {
                append(&mut leaves, source, path)?;
            }
        }

        leaves.sort_unstable_by_key(|(source, _)| *source);
        leaves.dedup_by_key(|(source, _)| *source);
        Ok(Self { leaves })
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
