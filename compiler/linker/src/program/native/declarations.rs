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
        if symbols.native_requirements().cxx() {
            use scoop_identity::{
                CanonicalNativeLibraryName, NativeLibraryGrouping, NativeLibraryKind,
            };
            let target = symbols.native_requirements().target();
            let name = match target.id() {
                scoop_lir::TargetProfileId::DarwinAarch64 => "c++",
                scoop_lir::TargetProfileId::LinuxX86_64Gnu => "stdc++",
                scoop_lir::TargetProfileId::LinuxX86_64Musl => {
                    return Err(error("C++ native runtime is not supported for Linux musl"));
                }
            };
            let key = NativeLinkRequirementKey::for_target(
                target.wire_id(),
                CanonicalNativeLibraryName::new(name).map_err(error)?,
                NativeLibraryKind::Dynamic,
                NativeLibraryGrouping::Independent,
            );
            libraries
                .entry(NativeLinkRequirementId::from_key(&key).map_err(error)?)
                .or_insert_with(|| (key, Vec::new()))
                .1
                .push(format!(
                    "{} (C++ runtime)",
                    artifact.manifest().cone().coordinate()
                ));
        }
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
