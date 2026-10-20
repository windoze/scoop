//! Final image records obtained from the retained artifact definitions.

use super::*;
use scoop_identity::ObjectDefinitionPlanId;
use scoop_slib::ProgramLinkArtifact;

pub(crate) struct SelectedImage {
    pub name: String,
    pub coordinate: [Vec<u8>; 3],
    pub cone: [u8; 32],
    pub dependencies: Vec<[u8; 32]>,
    pub tables: [Vec<String>; 6],
    pub traps: [(String, &'static [u8]); 2],
}

impl SelectedImage {
    pub(super) fn new(
        artifact: &ProgramLinkArtifact,
        selected: &BTreeSet<ObjectDefinitionPlanId>,
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<Self, LinkError> {
        let production = artifact.production();
        let coordinate = artifact.manifest().cone().coordinate();
        let tables = production.registrations.each_ref().map(|entries| {
            entries
                .iter()
                .filter(|(definition, _)| selected.contains(definition))
                .map(|(_, symbol)| object_symbol(*symbol, profile))
                .collect()
        });
        let traps = [
            (
                object_symbol(production.trap_symbols[0], profile),
                b"array index out of bounds\0".as_slice(),
            ),
            (
                object_symbol(production.trap_symbols[1], profile),
                b"array size overflow\0".as_slice(),
            ),
        ];
        Ok(Self {
            name: object_symbol(production.image_symbol, profile),
            coordinate: [coordinate.group(), coordinate.name(), coordinate.version()]
                .map(|part| part.as_bytes().to_vec()),
            cone: *artifact.identity().as_array(),
            dependencies: artifact
                .manifest()
                .direct_dependencies()
                .iter()
                .map(|dependency| *dependency.identity().as_array())
                .collect(),
            tables,
            traps,
        })
    }

    pub(super) fn defined_names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.name.as_str()).chain(self.traps.iter().map(|(name, _)| name.as_str()))
    }

    pub(crate) fn references(&self) -> impl Iterator<Item = &String> {
        self.tables.iter().flatten()
    }
}
