//! Stable lexical paths for identity-bearing HIR definitions.

use std::collections::HashMap;

use scoop_identity::{
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

use crate::Lowerer;

/// Role-local counters for one definition owner. `prefix` is the complete
/// path of that owner within its own source declaration root.
#[derive(Clone, Debug, Default)]
pub(crate) struct DefinitionPathContext {
    prefix: Vec<StructuralPathSegment>,
    next_ordinals: HashMap<StructuralDefinitionSiteRole, usize>,
}

impl DefinitionPathContext {
    pub(crate) fn nested(path: &StructuralDefinitionPath) -> Self {
        Self {
            prefix: path.segments().to_vec(),
            next_ordinals: HashMap::new(),
        }
    }

    pub(crate) fn next(&mut self, role: StructuralDefinitionSiteRole) -> StructuralDefinitionPath {
        let next = self.next_ordinals.entry(role).or_default();
        let ordinal = u32::try_from(*next)
            .expect("one definition owner cannot contain more than u32::MAX sites of one role");
        *next = next
            .checked_add(1)
            .expect("definition-site counter overflowed usize");
        self.at(role, ordinal)
    }

    pub(crate) fn at(
        &self,
        role: StructuralDefinitionSiteRole,
        ordinal: u32,
    ) -> StructuralDefinitionPath {
        let mut segments = self.prefix.clone();
        segments.push(StructuralPathSegment::new(role, ordinal));
        StructuralDefinitionPath::new(segments)
            .expect("appending a definition-site segment always makes a non-empty path")
    }
}

impl Lowerer {
    pub(crate) fn current_definition_root(&self) -> scoop_hir::LexicalDefinitionRoot {
        self.definition_root
            .expect("identity-bearing lexical sites are lowered inside a typed definition root")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(path: &StructuralDefinitionPath) -> Vec<(StructuralDefinitionSiteRole, u32)> {
        path.segments()
            .iter()
            .map(|segment| (segment.site_role(), segment.ordinal()))
            .collect()
    }

    #[test]
    fn roles_count_independently_and_nested_owners_extend_the_parent_path() {
        let mut root = DefinitionPathContext::default();
        let local = root.next(StructuralDefinitionSiteRole::LocalDeclaration);
        let first_lambda = root.next(StructuralDefinitionSiteRole::Lambda);
        let second_lambda = root.next(StructuralDefinitionSiteRole::Lambda);

        assert_eq!(
            segments(&local),
            vec![(StructuralDefinitionSiteRole::LocalDeclaration, 0)]
        );
        assert_eq!(
            segments(&first_lambda),
            vec![(StructuralDefinitionSiteRole::Lambda, 0)]
        );
        assert_eq!(
            segments(&second_lambda),
            vec![(StructuralDefinitionSiteRole::Lambda, 1)]
        );

        let mut nested = DefinitionPathContext::nested(&first_lambda);
        assert_eq!(
            segments(&nested.next(StructuralDefinitionSiteRole::Lambda)),
            vec![
                (StructuralDefinitionSiteRole::Lambda, 0),
                (StructuralDefinitionSiteRole::Lambda, 0),
            ]
        );
    }
}
