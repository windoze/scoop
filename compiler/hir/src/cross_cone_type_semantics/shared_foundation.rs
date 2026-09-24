//! Type facts and representations replayed from shared declaration metadata.

use scoop_identity::{
    ConeIdentity, PersistentExactTypeId, PersistentTypeId, ValidatedIdentityGraph,
};
use scoop_wire::BudgetMeter;

use crate::{
    CheckedExactTypeFactsV1, CheckedNominalRepresentationSupportV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1, NominalMaterializationClosure, OdrFreeHirFoundation,
};

mod errors;
mod facts;
mod inheritance;
mod keys;
mod requirements;
mod sources;
use SharedTypeMetadataError as Error;
pub use errors::SharedTypeMetadataError;
pub(crate) use sources::declaration_access;

/// The same decoded declaration data used by ordinary HIR readers. No source
/// transcript or compilation-process evidence is part of this input.
#[derive(Clone, Copy)]
pub struct SharedTypeMetadataV1<'a> {
    pub provider: ConeIdentity,
    pub identities: &'a ValidatedIdentityGraph,
    pub foundation: &'a OdrFreeHirFoundation,
    pub public: &'a CrossConeHirInterfaceSectionV1,
}

impl SharedTypeMetadataV1<'_> {
    /// Resolves a parameter-free source signature through this artifact's
    /// canonical identity graph, including each structural component.
    pub fn signature_exact_type(
        self,
        signature: &scoop_identity::SignatureTypeKey,
        meter: &mut BudgetMeter,
    ) -> Result<PersistentExactTypeId, SharedTypeMetadataError> {
        MetadataTypes {
            current: self,
            dependencies: &[],
        }
        .exact(signature, 1, meter)
    }
}

/// Complete fact and representation inventories agree with the shared
/// declarations. Inheritance, protected/default and selected-use checks remain
/// separate obligations before the complete type section is publishable.
#[derive(Clone, Copy)]
pub struct CheckedSharedTypeFoundationV1<'a> {
    metadata: SharedTypeMetadataV1<'a>,
    section: &'a CrossConeTypeSemanticsSectionV1,
    facts: CheckedExactTypeFactsV1<'a>,
    representations: CheckedNominalRepresentationSupportV1<'a>,
}

impl<'a> CheckedSharedTypeFoundationV1<'a> {
    pub const fn metadata(self) -> SharedTypeMetadataV1<'a> {
        self.metadata
    }

    pub const fn provider(self) -> ConeIdentity {
        self.metadata.provider
    }

    pub const fn section(self) -> &'a CrossConeTypeSemanticsSectionV1 {
        self.section
    }

    pub const fn facts(self) -> CheckedExactTypeFactsV1<'a> {
        self.facts
    }

    pub const fn representations(self) -> CheckedNominalRepresentationSupportV1<'a> {
        self.representations
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MetadataTypes<'a, 'd> {
    pub(crate) current: SharedTypeMetadataV1<'a>,
    pub(crate) dependencies: &'d [CheckedSharedTypeFoundationV1<'a>],
}

impl CrossConeTypeSemanticsSectionV1 {
    pub fn validate_shared_foundation<'a>(
        &'a self,
        metadata: SharedTypeMetadataV1<'a>,
        dependencies: &[CheckedSharedTypeFoundationV1<'a>],
        meter: &mut BudgetMeter,
    ) -> Result<CheckedSharedTypeFoundationV1<'a>, Error> {
        let types = MetadataTypes {
            current: metadata,
            dependencies,
        };
        types.validate_dependencies(meter)?;
        let materialization = NominalMaterializationClosure::from_declarations(
            metadata.public.nominal_interfaces(),
            metadata.public.callable_interfaces(),
            meter,
        )
        .map_err(Error::Materialization)?;
        let facts = facts::validate(self.exact_facts(), types, &materialization, meter)?;
        let representations = self.representation_support().validate_shared_metadata(
            types,
            &materialization,
            facts,
            meter,
        )?;
        Ok(CheckedSharedTypeFoundationV1 {
            metadata,
            section: self,
            facts,
            representations,
        })
    }
}
