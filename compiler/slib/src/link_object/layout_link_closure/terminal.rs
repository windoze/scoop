//! Closure-wide terminal-provider and final Strong-owner validation.

use std::collections::BTreeMap;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    CrossConeLayoutAbiSectionV1, ExternalShapeLinkImportV1, OdrFreeLirFoundation,
    ShapeLinkProductionV1, ShapeLinkProviderPartsV1, ShapeLinkProviderV1, ShapeLinkSupportLookupV1,
};
use scoop_wire::{BudgetMeter, WirePath, encode_canonical_temporary_with_meter};

use crate::CanonicalDefinedLinkSymbolOwnerSetV1;

mod error;
pub use error::*;
mod foreign;
mod owner;
pub(crate) use foreign::reject_layout_foreign_strong_owners_v1;
mod replay;
pub use replay::verify_replayed_layout_strong_owners_v1;

/// Complete per-artifact inputs already validated by the LIR and object
/// readers. Construction binds the layout section to the same provider's
/// Strong V2 production before closure traversal begins.
pub struct CrossConeLayoutTerminalArtifactV1<'a> {
    section: &'a CrossConeLayoutAbiSectionV1<'a>,
    provider: ShapeLinkProviderV1<'a>,
    support: &'a dyn ShapeLinkSupportLookupV1<'a>,
    defined_symbols: &'a CanonicalDefinedLinkSymbolOwnerSetV1,
}

pub struct CrossConeLayoutTerminalArtifactPartsV1<'a> {
    pub foundation: &'a OdrFreeLirFoundation,
    pub production: ShapeLinkProductionV1<'a>,
    pub ordinary: &'a scoop_lir::CrossConeLirBridgeSectionV1,
    pub section: &'a CrossConeLayoutAbiSectionV1<'a>,
    pub support: &'a dyn ShapeLinkSupportLookupV1<'a>,
    pub defined_symbols: &'a CanonicalDefinedLinkSymbolOwnerSetV1,
}

impl<'a> CrossConeLayoutTerminalArtifactV1<'a> {
    pub fn try_new(
        parts: CrossConeLayoutTerminalArtifactPartsV1<'a>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, CrossConeLayoutTerminalValidationError> {
        let provider = ShapeLinkProviderV1::try_new(
            ShapeLinkProviderPartsV1 {
                foundation: parts.foundation,
                production: parts.production,
                ordinary: parts.ordinary,
                layouts: parts.section.layouts(),
                callables: parts.section.callables(),
                descriptors: parts.section.descriptors(),
                dispatch: parts.section.dispatch(),
            },
            meter,
        )
        .map_err(CrossConeLayoutTerminalValidationError::ProviderSection)?;
        let identity = provider.provider();
        if parts.section.provider() != identity || parts.defined_symbols.producer() != identity {
            return Err(
                CrossConeLayoutTerminalValidationError::ArtifactIdentityMismatch {
                    foundation: identity,
                    section: parts.section.provider(),
                    defined_symbols: parts.defined_symbols.producer(),
                },
            );
        }
        Ok(Self {
            section: parts.section,
            provider,
            support: parts.support,
            defined_symbols: parts.defined_symbols,
        })
    }

    pub fn identity(&self) -> ConeIdentity {
        self.provider.provider()
    }

    pub const fn section(&self) -> &'a CrossConeLayoutAbiSectionV1<'a> {
        self.section
    }

    pub const fn defined_symbols(&self) -> &'a CanonicalDefinedLinkSymbolOwnerSetV1 {
        self.defined_symbols
    }
}

/// A proof that every physical import in the explicit closure was replayed
/// against its unique terminal provider and resolved to one final Strong
/// definition. It retains the exact artifact slice used for validation.
pub struct ValidatedCrossConeLayoutTerminalClosureV1<'closure, 'artifact> {
    artifacts: &'closure [CrossConeLayoutTerminalArtifactV1<'artifact>],
    positions: BTreeMap<ConeIdentity, usize>,
    import_count: usize,
}

impl<'closure, 'artifact> ValidatedCrossConeLayoutTerminalClosureV1<'closure, 'artifact> {
    pub fn artifact_count(&self) -> usize {
        self.artifacts.len()
    }

    pub const fn import_count(&self) -> usize {
        self.import_count
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&CrossConeLayoutTerminalArtifactV1<'artifact>> {
        self.positions
            .get(&identity)
            .map(|position| &self.artifacts[*position])
    }
}

pub fn validate_cross_cone_layout_terminal_closure_v1<'closure, 'artifact>(
    artifacts: &'closure [CrossConeLayoutTerminalArtifactV1<'artifact>],
    meter: &mut BudgetMeter,
) -> Result<
    ValidatedCrossConeLayoutTerminalClosureV1<'closure, 'artifact>,
    CrossConeLayoutTerminalValidationError,
> {
    let path = WirePath::root();
    if artifacts.is_empty() {
        return Err(CrossConeLayoutTerminalValidationError::NoArtifacts);
    }
    meter.check_table_entries(artifacts.len() as u64, &path)?;
    meter.charge_nodes(artifacts.len() as u64, &path)?;
    meter.charge_collection_slots((artifacts.len() as u64).saturating_mul(2), &path)?;
    let mut positions = BTreeMap::new();
    for (position, artifact) in artifacts.iter().enumerate() {
        if let Some(previous) = positions.insert(artifact.identity(), position) {
            return Err(CrossConeLayoutTerminalValidationError::DuplicateProvider {
                provider: artifact.identity(),
                first: previous,
                second: position,
            });
        }
    }
    let mut import_count = 0usize;
    for consumer in artifacts {
        let imports = consumer.section.selected().physical_imports().records();
        meter.check_table_entries(imports.len() as u64, &path)?;
        import_count = import_count
            .checked_add(imports.len())
            .ok_or(CrossConeLayoutTerminalValidationError::ImportCountOverflow)?;
        meter.check_table_entries(import_count as u64, &path)?;
        meter.charge_edges(imports.len() as u64, &path)?;
        for import in imports {
            let position = positions.get(&import.provider()).copied().ok_or(
                CrossConeLayoutTerminalValidationError::MissingProvider {
                    consumer: consumer.identity(),
                    provider: import.provider(),
                },
            )?;
            validate_import(consumer, import, &artifacts[position], artifacts, meter)?;
        }
    }
    Ok(ValidatedCrossConeLayoutTerminalClosureV1 {
        artifacts,
        positions,
        import_count,
    })
}

fn validate_import(
    consumer: &CrossConeLayoutTerminalArtifactV1<'_>,
    import: &ExternalShapeLinkImportV1<'_>,
    terminal: &CrossConeLayoutTerminalArtifactV1<'_>,
    artifacts: &[CrossConeLayoutTerminalArtifactV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    if consumer.section.target_profile() != terminal.section.target_profile() {
        return Err(CrossConeLayoutTerminalValidationError::TargetMismatch {
            consumer: consumer.identity(),
            provider: terminal.identity(),
        });
    }
    let expected = ExternalShapeLinkImportV1::replay(
        &terminal.provider,
        import.subject(),
        consumer.identity(),
        consumer.provider.canonical_definitions(),
        terminal.support,
        meter,
    )
    .map_err(
        |source| CrossConeLayoutTerminalValidationError::ImportReplay {
            context: import_context(consumer.identity(), import),
            source: Box::new(source),
        },
    )?;
    validate_header(consumer.identity(), import, &expected)?;
    let path = WirePath::root();
    if encode_canonical_temporary_with_meter(import.contract(), meter, &path)?
        != encode_canonical_temporary_with_meter(expected.contract(), meter, &path)?
    {
        return Err(CrossConeLayoutTerminalValidationError::ContractMismatch(
            import_context(consumer.identity(), import),
        ));
    }
    owner::validate(consumer, import, terminal, artifacts, meter)
}

fn validate_header(
    consumer: ConeIdentity,
    actual: &ExternalShapeLinkImportV1<'_>,
    expected: &ExternalShapeLinkImportV1<'_>,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    if actual.expected_symbol() != expected.expected_symbol() {
        return Err(CrossConeLayoutTerminalValidationError::SymbolMismatch {
            context: import_context(consumer, actual),
            expected: expected.expected_symbol(),
            actual: actual.expected_symbol(),
        });
    }
    if actual.required_definition() != expected.required_definition() {
        return Err(CrossConeLayoutTerminalValidationError::DefinitionMismatch {
            context: import_context(consumer, actual),
            expected: expected.required_definition(),
            actual: actual.required_definition(),
        });
    }
    Ok(())
}

fn import_context(
    consumer: ConeIdentity,
    import: &ExternalShapeLinkImportV1<'_>,
) -> Box<CrossConeLayoutTerminalImportContextV1> {
    Box::new(CrossConeLayoutTerminalImportContextV1::new(
        consumer,
        import.provider(),
        import.subject(),
    ))
}
