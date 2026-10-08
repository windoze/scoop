use super::*;
use scoop_identity::{NativeLinkRequirementId, NativeLinkRequirementKey};

type Libraries = BTreeMap<NativeLinkRequirementId, (NativeLinkRequirementKey, Vec<String>)>;

pub(in crate::program) fn read(
    closure: &ProgramLinkClosure,
) -> Result<(BTreeMap<String, Declarations<'_>>, Libraries), LinkError> {
    let mut declarations: BTreeMap<String, Declarations<'_>> = BTreeMap::new();
    for (artifact, symbols) in closure.artifacts() {
        for requirement in symbols.native_requirements().contracts() {
            let symbol = String::from_utf8(
                requirement
                    .symbol_key()
                    .native_link_symbol()
                    .as_bytes()
                    .to_vec(),
            )
            .map_err(error)?;
            let origin = format!(
                "{} (native declarations {:?})",
                artifact.manifest().cone().coordinate(),
                requirement.sources()
            );
            match declarations.get_mut(&symbol) {
                Some(existing) => {
                    existing.origins.push(origin);
                    if existing.contract != requirement.contract() && existing.difference.is_none()
                    {
                        existing.difference = Some(contracts::difference(
                            existing.contract,
                            requirement.contract(),
                        ));
                    }
                }
                None => {
                    declarations.insert(
                        symbol,
                        Declarations {
                            contract: requirement.contract(),
                            origins: vec![origin],
                            difference: None,
                        },
                    );
                }
            }
        }
    }
    for (symbol, declarations) in &declarations {
        if let Some(difference) = &declarations.difference {
            return Err(error(format!(
                "native contract conflict for {symbol}: {difference}; origins: {}",
                declarations.origins.join(", ")
            )));
        }
    }
    let mut libraries = BTreeMap::new();
    for (artifact, symbols) in closure.artifacts() {
        for record in symbols.native_requirements().library_requirements() {
            let entry = libraries
                .entry(record.id())
                .or_insert_with(|| (record.key().clone(), Vec::new()));
            entry
                .1
                .push(artifact.manifest().cone().coordinate().to_string());
        }
    }
    Ok((declarations, libraries))
}
