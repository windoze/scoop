use std::collections::HashMap;

use scoop_identity::{StableIdentityOrderError, stable_topological_identity_order};

use super::*;

pub type CallableApplicationRecord =
    CborIdentityRecord<PersistentCallableApplicationId, CallableApplicationKey>;

/// Complete canonical application table referenced by LocalConcrete HIR.
/// Records are dependency-first and every enclosing application is present.
#[derive(Clone, Debug)]
pub struct CallableApplicationIdentities {
    records: Vec<CallableApplicationRecord>,
    positions: HashMap<PersistentCallableApplicationId, usize>,
}

impl CallableApplicationIdentities {
    pub fn checked(
        records: Vec<CallableApplicationRecord>,
    ) -> Result<Self, StableIdentityOrderError<PersistentCallableApplicationId>> {
        let records =
            stable_topological_identity_order(records, CborIdentityRecord::id, |record| {
                record.key().callable_application_dependencies()
            })?;
        let positions = records
            .iter()
            .enumerate()
            .map(|(position, record)| (record.id(), position))
            .collect();
        Ok(Self { records, positions })
    }

    pub fn get(&self, id: PersistentCallableApplicationId) -> Option<&CallableApplicationRecord> {
        self.positions
            .get(&id)
            .map(|position| &self.records[*position])
    }

    pub fn records(&self) -> &[CallableApplicationRecord] {
        &self.records
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableInstantiationOwner, CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactTypeKey, PackagePath, PersistentExactTypeId,
        PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;

    fn function(name: &str) -> PersistentFunctionId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap()
        .id()
    }

    fn unit() -> PersistentExactTypeId {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
        .id()
    }

    #[test]
    fn applications_are_dependency_ordered_and_require_enclosing_records() {
        let outer_key = CallableApplicationKey::for_function(
            function("outer"),
            CallableInstantiationOwner::ExactNominalOwner(unit()),
        );
        let outer = CborIdentityRecord::from_key(outer_key).unwrap();
        let inner = CborIdentityRecord::from_key(CallableApplicationKey::for_function(
            function("inner"),
            CallableInstantiationOwner::EnclosingCallableApplication(outer.id()),
        ))
        .unwrap();

        let table = CallableApplicationIdentities::checked(vec![inner.clone(), outer.clone()])
            .expect("the complete application graph is valid");
        assert_eq!(table.records(), &[outer.clone(), inner.clone()]);
        assert_eq!(table.get(inner.id()), Some(&inner));

        assert!(matches!(
            CallableApplicationIdentities::checked(vec![inner]),
            Err(StableIdentityOrderError::MissingDependency { .. })
        ));
    }
}
