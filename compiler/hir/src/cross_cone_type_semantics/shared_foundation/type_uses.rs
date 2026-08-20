//! Rebuilds materialized type requirements from the original shared metadata.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{CoreBuiltinNominal, ExactTypeKey, SignatureTypeKey, SourceDeclarationKey};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CanonicalSelectedExternalTypeUsesV1, ExternalHirTargetV1, HirDependencyTypeSiteV1,
    HirExpressionTypeRoleV1, NominalMaterializationRequirementV1, SelectedDirectInheritanceEdgeV1,
    SelectedExternalTypeUseV1, SelectedTypeUseV1, SourceNominalId,
};

mod calls;
mod declarations;
mod graph;
mod nominals;
mod roots;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Kind {
    Signature,
    Representation,
    TypeTest,
    ShapeSupport,
    Singleton(scoop_identity::PersistentObjectValueId),
    Construct(scoop_identity::PersistentConstructorId),
    VariantConstruct(scoop_identity::PersistentEnumVariantId),
    MemberCall {
        receiver: PersistentExactTypeId,
        declaration: crate::InheritanceCallableDeclarationV1,
    },
    ClassBase(PersistentExactTypeId),
    Interface(PersistentExactTypeId),
}

impl Kind {
    fn usage(self, exact: PersistentExactTypeId) -> SelectedTypeUseV1 {
        match self {
            Self::Signature => SelectedTypeUseV1::Signature { exact },
            Self::Representation => SelectedTypeUseV1::Representation { exact },
            Self::TypeTest => SelectedTypeUseV1::TypeTest { exact },
            Self::ShapeSupport => SelectedTypeUseV1::ShapeSupport { exact },
            Self::Singleton(value) => SelectedTypeUseV1::SingletonValue { exact, value },
            Self::Construct(declaration) => SelectedTypeUseV1::Construct {
                exact,
                declaration: crate::SelectedTypeConstructionV1::Constructor(declaration),
            },
            Self::VariantConstruct(declaration) => SelectedTypeUseV1::Construct {
                exact,
                declaration: crate::SelectedTypeConstructionV1::EnumVariant(declaration),
            },
            Self::MemberCall {
                receiver,
                declaration,
            } => SelectedTypeUseV1::MemberCall {
                receiver,
                declaration,
            },
            Self::ClassBase(derived) => SelectedTypeUseV1::Inheritance {
                derived,
                edge: SelectedDirectInheritanceEdgeV1::ClassBase { exact },
            },
            Self::Interface(derived) => SelectedTypeUseV1::Inheritance {
                derived,
                edge: SelectedDirectInheritanceEdgeV1::Interface { exact },
            },
        }
    }

    fn includes(usage: SelectedTypeUseV1) -> bool {
        matches!(
            usage,
            SelectedTypeUseV1::Signature { .. }
                | SelectedTypeUseV1::Representation { .. }
                | SelectedTypeUseV1::TypeTest { .. }
                | SelectedTypeUseV1::ShapeSupport { .. }
                | SelectedTypeUseV1::Inheritance { .. }
                | SelectedTypeUseV1::Construct { .. }
                | SelectedTypeUseV1::MemberCall { .. }
                | SelectedTypeUseV1::SingletonValue { .. }
        )
    }
}

struct Provider<'a> {
    metadata: SharedTypeMetadataV1<'a>,
    materialization: NominalMaterializationClosure,
}

struct Graph<'a> {
    current: SharedTypeMetadataV1<'a>,
    providers: BTreeMap<ConeIdentity, Provider<'a>>,
    requirements: BTreeMap<PersistentTypeId, Vec<NominalMaterializationRequirementV1<'a>>>,
    expanded: BTreeSet<PersistentTypeId>,
    pending: Vec<PersistentTypeId>,
    selected: BTreeSet<(ConeIdentity, Kind, PersistentExactTypeId)>,
}

impl<'a> SharedTypeMetadataV1<'a> {
    /// Uses actual shared occurrences and declaration dependencies, never the
    /// candidate selected table. This is the type-requirement partition only;
    /// actual call signatures, member calls, explicit and runtime construction,
    /// inheritance, and shape operations are included. Other source operations
    /// and access relations remain independent checks.
    pub fn materialized_type_uses(
        self,
        dependencies: &[SharedTypeMetadataV1<'a>],
    ) -> Result<CanonicalSelectedExternalTypeUsesV1, Error> {
        let mut graph = Graph::new(self, dependencies)?;
        graph.roots()?;
        graph.close()?;
        graph.finish()
    }

    pub fn validate_materialized_type_uses(
        self,
        selected: &CanonicalSelectedExternalTypeUsesV1,
        dependencies: &[SharedTypeMetadataV1<'a>],
    ) -> Result<(), Error> {
        let expected = self.materialized_type_uses(dependencies)?;

        if !expected.records().iter().eq(selected
            .records()
            .iter()
            .filter(|record| Kind::includes(record.usage())))
        {
            return Err(Error::TypeUseInventory);
        }
        Ok(())
    }
}

impl CheckedSharedTypeFoundationV1<'_> {
    /// Precisely compares type, inheritance, shape, construction and member calls.
    /// This does not construct a complete selected-use or artifact permit.
    pub fn validate_materialized_type_uses(self, dependencies: &[Self]) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        let mut metadata = Vec::new();
        scoop_wire::allocation::try_reserve(&mut metadata, dependencies.len(), &path)?;
        metadata.extend(dependencies.iter().map(|dependency| dependency.metadata()));
        self.metadata
            .validate_materialized_type_uses(self.section.selected(), &metadata)
    }
}

#[cfg(test)]
mod tests;
