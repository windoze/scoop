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
#[derive(Debug, Clone, Default)]
pub struct HirFacts {
    pub declarations: DeclarationFacts,
    pub source_sites: SourceSiteFacts,
    pub globals: GlobalRootFacts,
    pub native: NativeExternFacts,
    pub type_context: TypeContextFacts,
}

impl HirFacts {
    /// Create an empty fact product for incremental migration and tests.
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
    use scoopc_ids::CanonicalTextKey;
    use scoopc_project_model::StableConeKey;
    use scoopc_types::{TypeKind, TypeStore, ValueTypeKind};

    use super::*;
    use crate::common::FactIdentity;
    use crate::globals::{GlobalRootFact, GlobalRootKind};
    use crate::verify::VerifyError;

    #[test]
    fn empty_hir_facts_verify_and_dump_group_boundaries() {
        let facts = HirFacts::new();

        assert!(facts.is_empty());
        assert!(facts.verify().is_ok());

        let dump = facts.dump();
        assert!(dump.contains("declarations: nominals=0"));
        assert!(dump.contains("source_sites: calls=0"));
        assert!(dump.contains("type_context: universe=<none>"));
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

    fn global_root(key: &str, ty: scoopc_types::TypeId) -> GlobalRootFact {
        GlobalRootFact {
            identity: FactIdentity::new(
                CanonicalTextKey::new(key),
                key,
                StableConeKey::new("fixture", "0.0.0"),
                None,
            ),
            kind: GlobalRootKind::TopLevelVal,
            ty,
            storage: None,
            initializer: None,
            monomorphic: true,
        }
    }
}
