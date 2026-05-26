//! HIR semantic fact product shared by later compiler stages.
//!
//! This crate is intentionally data-only: it depends on the stage-independent
//! base crates for identity, source, span, type, and cone context primitives, and
//! it does not depend on the `scoopc` facade, HIR nodes, MIR nodes, effect stages,
//! LIR, or backend ABI types.

#![forbid(unsafe_code)]

pub mod common;
pub mod declarations;
pub mod dump;
pub mod globals;
pub mod native;
pub mod source_sites;
pub mod type_context;
pub mod verify;

use declarations::DeclarationFacts;
use globals::GlobalRootFacts;
use native::NativeExternFacts;
use source_sites::SourceSiteFacts;
use type_context::TypeContextFacts;

/// Complete set of source-semantic facts published by the HIR barrier.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HirFacts {
    pub schema_version: scoopc_types::WireSchemaVersion,
    pub declarations: DeclarationFacts,
    pub source_sites: SourceSiteFacts,
    pub globals: GlobalRootFacts,
    pub native: NativeExternFacts,
    pub type_context: TypeContextFacts,
}

impl Default for HirFacts {
    fn default() -> Self {
        Self {
            schema_version: scoopc_types::WIRE_SCHEMA_VERSION,
            declarations: DeclarationFacts::default(),
            source_sites: SourceSiteFacts::default(),
            globals: GlobalRootFacts::default(),
            native: NativeExternFacts::default(),
            type_context: TypeContextFacts::default(),
        }
    }
}

impl HirFacts {
    /// Create an empty fact product for tests and staged construction.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return whether all fact groups are currently empty.
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
            && self.source_sites.is_empty()
            && self.globals.is_empty()
            && self.native.is_empty()
            && self.type_context.is_empty()
    }

    /// Verify structural invariants before handing facts to later stages.
    pub fn verify(&self) -> verify::Result<()> {
        verify::verify_hir_facts(self)
    }

    /// Render a stable textual summary of the fact groups.
    pub fn dump(&self) -> String {
        dump::dump_hir_facts(self)
    }
}

#[cfg(test)]
mod tests {
    use scoop_project_model::StableConeKey;
    use scoopc_ids::CanonicalTextKey;
    use scoopc_types::{TypeKind, TypeStore, ValueTypeKind};

    use super::*;
    use crate::common::FactIdentity;
    use crate::globals::{GlobalRootFact, GlobalRootKind, GlobalStoragePolicy};
    use crate::verify::VerifyError;

    #[test]
    fn empty_hir_facts_verify_and_dump_group_boundaries() {
        let facts = HirFacts::new();

        assert!(facts.is_empty());
        assert!(facts.verify().is_ok());

        let dump = facts.dump();
        assert!(dump.contains("declarations: nominals=0"));
        assert!(dump.contains("source_sites: function_effects=0"));
        assert!(dump.contains("type_context: universe=<none>"));
    }

    #[test]
    fn hir_facts_bincode_round_trip_preserves_schema_and_content() {
        let facts = HirFacts::new();
        let bytes = bincode::serialize(&facts).expect("serialize HIR facts");
        let decoded: HirFacts = bincode::deserialize(&bytes).expect("deserialize HIR facts");

        assert_eq!(decoded.schema_version, scoopc_types::WIRE_SCHEMA_VERSION);
        assert_eq!(decoded, facts);
    }

    #[test]
    fn verifier_rejects_duplicate_fact_identities() {
        let mut types = TypeStore::new();
        let unit = types.intern(TypeKind::Value(ValueTypeKind::Unit));
        let duplicate = global_root("app.main", unit);
        let mut facts = HirFacts::new();
        facts.globals.roots.push(duplicate.clone());
        facts.globals.roots.push(duplicate);

        let err = facts.verify().unwrap_err();
        assert_eq!(
            err,
            VerifyError::DuplicateFactIdentity("app.main".to_string())
        );
    }

    #[test]
    fn verifier_rejects_illegal_global_root_facts() {
        let mut types = TypeStore::new();
        let unit = types.intern(TypeKind::Value(ValueTypeKind::Unit));

        let mut generic_root = global_root("app.Root", unit);
        generic_root.monomorphic = false;
        let mut facts = HirFacts::new();
        facts.globals.roots.push(generic_root);
        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::GenericGlobalRoot("app.Root".to_string())
        );

        let mut var_without_storage = global_root("app.Counter", unit);
        var_without_storage.kind = GlobalRootKind::TopLevelVar;
        let mut facts = HirFacts::new();
        facts.globals.roots.push(var_without_storage);
        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::TopLevelVarMissingStoragePolicy("app.Counter".to_string())
        );

        let mut val_with_storage = global_root("app.Immutable", unit);
        val_with_storage.storage = Some(GlobalStoragePolicy::Global);
        let mut facts = HirFacts::new();
        facts.globals.roots.push(val_with_storage);
        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::NonVarGlobalRootHasStoragePolicy("app.Immutable".to_string())
        );
    }

    fn global_root(key: &str, ty: scoopc_types::TypeId) -> GlobalRootFact {
        GlobalRootFact {
            identity: FactIdentity::new(
                CanonicalTextKey::new(key),
                key,
                StableConeKey::new("fixture", "0.0.0"),
                None,
            ),
            kind: GlobalRootKind::TopLevelVal,
            ty: Some(ty),
            storage: None,
            initializer: None,
            monomorphic: true,
        }
    }
}
