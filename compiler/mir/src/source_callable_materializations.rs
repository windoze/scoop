use scoop_identity::CallableMaterialization;

use crate::FunctionId;

/// One callable materialization from LocalConcrete HIR at its MIR function
/// location.
///
/// The identity remains HIR-owned. MIR retains this typed relation so later
/// transforms can identify source callables without recovering them from
/// display names, symbols, or arena ordinals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCallableMaterialization {
    function: FunctionId,
    materialization: CallableMaterialization,
}

impl SourceCallableMaterialization {
    pub const fn new(function: FunctionId, materialization: CallableMaterialization) -> Self {
        Self {
            function,
            materialization,
        }
    }

    pub const fn function(&self) -> FunctionId {
        self.function
    }

    pub const fn materialization(&self) -> CallableMaterialization {
        self.materialization
    }

    fn sort_key(&self) -> u32 {
        self.function.into_raw().into_u32()
    }
}

/// Complete one-to-one relation for LocalConcrete HIR callables transposed
/// into this MIR module.
///
/// MIR-generated functions do not enter this relation. They retain their own
/// generated callable identities in the metadata owned by their transform.
#[derive(Clone, Debug, Default)]
pub struct SourceCallableMaterializations {
    entries: Vec<SourceCallableMaterialization>,
}

impl SourceCallableMaterializations {
    pub fn checked(
        mut entries: Vec<SourceCallableMaterialization>,
    ) -> Result<Self, SourceCallableMaterializationRelationError> {
        entries.sort_by_key(SourceCallableMaterialization::sort_key);
        if let Some((first, _)) = entries
            .windows(2)
            .enumerate()
            .find(|(_, pair)| pair[0].function == pair[1].function)
        {
            return Err(
                SourceCallableMaterializationRelationError::DuplicateFunction {
                    first,
                    index: first + 1,
                },
            );
        }
        for (index, entry) in entries.iter().enumerate() {
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.materialization == entry.materialization)
            {
                return Err(
                    SourceCallableMaterializationRelationError::DuplicateMaterialization {
                        first,
                        index,
                    },
                );
            }
        }
        Ok(Self { entries })
    }

    pub fn get(&self, function: FunctionId) -> Option<&SourceCallableMaterialization> {
        let key = function.into_raw().into_u32();
        self.entries
            .binary_search_by_key(&key, SourceCallableMaterialization::sort_key)
            .ok()
            .map(|index| &self.entries[index])
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SourceCallableMaterialization> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceCallableMaterializationRelationError {
    DuplicateFunction { first: usize, index: usize },
    DuplicateMaterialization { first: usize, index: usize },
}

impl std::fmt::Display for SourceCallableMaterializationRelationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateFunction { first, index } => write!(
                formatter,
                "source callable entries {first} and {index} have the same MIR function"
            ),
            Self::DuplicateMaterialization { first, index } => write!(
                formatter,
                "source callable entries {first} and {index} have the same materialization"
            ),
        }
    }
}

impl std::error::Error for SourceCallableMaterializationRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableMaterializationContext, CallableTemplateOwner, CanonicalIdentifier, ConeIdentity,
        DeclarationScope, DefinitionOwnerChain, PackagePath, PersistentFunctionId,
        SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;

    fn materialization(name: &str) -> CallableMaterialization {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        CallableMaterialization::new(
            CallableTemplateOwner::Function(
                PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
            ),
            CallableMaterializationContext::NoSubstitution,
        )
    }

    #[test]
    fn relation_sorts_and_queries_typed_function_locations() {
        let first = FunctionId::from_raw(1_u32.into());
        let second = FunctionId::from_raw(4_u32.into());
        let first_materialization = materialization("first");
        let relation = SourceCallableMaterializations::checked(vec![
            SourceCallableMaterialization::new(second, materialization("second")),
            SourceCallableMaterialization::new(first, first_materialization),
        ])
        .unwrap();

        assert_eq!(relation.len(), 2);
        assert_eq!(relation.iter().next().unwrap().function(), first);
        assert_eq!(
            relation.get(first).unwrap().materialization(),
            first_materialization
        );
    }

    #[test]
    fn relation_rejects_duplicate_functions_and_materializations() {
        let first = FunctionId::from_raw(1_u32.into());
        let second = FunctionId::from_raw(4_u32.into());
        let shared = materialization("shared");
        assert_eq!(
            SourceCallableMaterializations::checked(vec![
                SourceCallableMaterialization::new(first, materialization("first")),
                SourceCallableMaterialization::new(first, materialization("second")),
            ])
            .unwrap_err(),
            SourceCallableMaterializationRelationError::DuplicateFunction { first: 0, index: 1 }
        );
        assert_eq!(
            SourceCallableMaterializations::checked(vec![
                SourceCallableMaterialization::new(first, shared),
                SourceCallableMaterialization::new(second, shared),
            ])
            .unwrap_err(),
            SourceCallableMaterializationRelationError::DuplicateMaterialization {
                first: 0,
                index: 1,
            }
        );
    }
}
