use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalNativeLibraryName, CborIdentityRecord, NativeLibraryGrouping, NativeLibraryKind,
    NativeLinkRequirementId, NativeLinkRequirementKey, TargetProfileId, TargetProfileWireId,
};
use serde::Deserialize;
use toml::Spanned;

use super::NativeConfig;
use crate::{ManifestParseError, ManifestParseErrorKind, TargetPredicate};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLibrary {
    name: CanonicalNativeLibraryName,
    kind: NativeLibraryKind,
    predicate: TargetPredicate,
}

impl NativeConfig {
    pub fn library_requirements(
        &self,
        target: TargetProfileId,
    ) -> Result<
        Vec<CborIdentityRecord<NativeLinkRequirementId, NativeLinkRequirementKey>>,
        scoop_wire::HashError,
    > {
        let mut selected = BTreeMap::new();
        for library in self
            .libraries
            .iter()
            .filter(|library| library.predicate.matches(target))
        {
            let record = CborIdentityRecord::<NativeLinkRequirementId, _>::from_key(
                NativeLinkRequirementKey::for_target(
                    TargetProfileWireId::new(target),
                    library.name.clone(),
                    library.kind,
                    NativeLibraryGrouping::Independent,
                ),
            )?;
            selected.insert(record.id(), record);
        }
        Ok(selected.into_values().collect())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawNativeLibrary {
    name: Spanned<String>,
    kind: Option<Spanned<String>>,
    when: Option<BTreeMap<String, Spanned<toml::Value>>>,
}

pub(super) fn parse(raw: RawNativeLibrary) -> Result<NativeLibrary, ManifestParseError> {
    let name = CanonicalNativeLibraryName::new(raw.name.get_ref())
        .map_err(|reason| error(&raw.name, reason))?;
    let kind = match raw.kind.as_ref().map(|kind| kind.get_ref().as_str()) {
        None | Some("default") => NativeLibraryKind::TargetDefault,
        Some("dynamic") => NativeLibraryKind::Dynamic,
        Some("static") => NativeLibraryKind::StaticArchive,
        Some("framework") => NativeLibraryKind::Framework,
        Some(_) => {
            return Err(error(
                raw.kind.as_ref().expect("explicit kind"),
                "expected default, dynamic, static or framework",
            ));
        }
    };
    Ok(NativeLibrary {
        name,
        kind,
        predicate: crate::selection::parse_predicate(raw.when)?,
    })
}

fn error(value: &Spanned<String>, reason: impl std::fmt::Display) -> ManifestParseError {
    ManifestParseError::new(
        ManifestParseErrorKind::InvalidNative(format!(
            "invalid native library {:?}: {reason}",
            value.get_ref()
        )),
        Some(value.span()),
    )
}
