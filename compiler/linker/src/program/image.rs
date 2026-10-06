//! Final image records obtained from the retained artifact definitions.

use super::*;
use scoop_identity::{ObjectDefinitionPlanId, PersistentId};
use scoop_lir::RegistrationIdentityV1;
use scoop_slib::{ProgramLinkArtifact, ReplayedLayoutLinkSymbolUsesV1};

pub(crate) struct SelectedImage {
    pub name: String,
    pub coordinate: [Vec<u8>; 3],
    pub cone: [u8; 32],
    pub fingerprint: [u8; 32],
    pub dependencies: Vec<[u8; 32]>,
    pub tables: [Vec<String>; 6],
    pub traps: [(String, &'static [u8]); 2],
}

impl SelectedImage {
    pub(super) fn new(
        artifact: &ProgramLinkArtifact,
        symbols: &ReplayedLayoutLinkSymbolUsesV1,
        selected: &BTreeSet<ObjectDefinitionPlanId>,
        profile: &ValidatedFinalLinkProfile,
    ) -> Result<Self, LinkError> {
        let production = artifact.production();
        let plan = production.image_plan();
        let coordinate = plan.cone().coordinate();
        let registrations = production.registration_production().identities();
        let tables = [
            table(
                registrations.static_storages(),
                registrations.static_storage_symbol_requests(),
                selected,
                profile,
            ),
            table(
                registrations.immortal_objects(),
                registrations.immortal_object_symbol_requests(),
                selected,
                profile,
            ),
            table(
                registrations.initialization_units(),
                registrations.initialization_unit_symbol_requests(),
                selected,
                profile,
            ),
            table(
                registrations.type_registrations(),
                registrations.type_registration_symbol_requests(),
                selected,
                profile,
            ),
            table(
                registrations.safepoints(),
                registrations.safepoint_symbol_requests(),
                selected,
                profile,
            ),
            table(
                registrations.callables(),
                registrations.callable_symbol_requests(),
                selected,
                profile,
            ),
        ];
        let image_definition = production
            .canonical_definitions()
            .plan(plan.definition_plan())
            .expect("verified image definition");
        let atoms = plan.support_atoms();
        let traps = [
            (
                atoms.array_bounds_message(),
                b"array index out of bounds\0".as_slice(),
            ),
            (
                atoms.array_size_overflow_message(),
                b"array size overflow\0".as_slice(),
            ),
        ]
        .map(|(atom, bytes)| {
            let boundary = image_definition
                .atom_boundaries()
                .iter()
                .find(|boundary| boundary.atom() == atom)
                .expect("verified image support atom");
            (object_symbol(boundary.start(), profile), bytes)
        });
        let fingerprint = symbols
            .final_objects()
            .runtime_images()
            .fingerprint()
            .fingerprint_for_definitions(selected)
            .map_err(error)?;
        Ok(Self {
            name: object_symbol(plan.symbol(), profile),
            coordinate: [coordinate.group(), coordinate.name(), coordinate.version()]
                .map(|part| part.as_bytes().to_vec()),
            cone: *artifact.identity().as_array(),
            fingerprint: *fingerprint.as_array(),
            dependencies: plan
                .dependencies()
                .iter()
                .map(|id| *id.as_array())
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

fn table<I: PersistentId>(
    identities: &[RegistrationIdentityV1<I>],
    symbols: impl Iterator<Item = PersistentSymbolRequest>,
    selected: &BTreeSet<ObjectDefinitionPlanId>,
    profile: &ValidatedFinalLinkProfile,
) -> Vec<String> {
    identities
        .iter()
        .zip(symbols)
        .filter(|(identity, _)| selected.contains(&identity.definition_plan()))
        .map(|(_, symbol)| object_symbol(symbol, profile))
        .collect()
}
