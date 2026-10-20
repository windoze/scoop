use super::*;

pub(in crate::documentation) struct DocumentationIndex {
    entries: Vec<(Target, Option<Target>, Kind, String, bool)>,
}

impl DocumentationIndex {
    pub(super) fn new(declarations: &[CapturedDeclaration]) -> Self {
        Self {
            entries: declarations
                .iter()
                .map(|entry| {
                    (
                        entry.declaration.target,
                        entry.declaration.parent,
                        entry.declaration.kind,
                        entry.declaration.name.clone(),
                        entry.comment.is_some(),
                    )
                })
                .collect(),
        }
    }

    pub(in crate::documentation) fn children(
        &self,
        parent: Target,
    ) -> impl Iterator<Item = (Target, Kind, &str)> {
        self.entries
            .iter()
            .filter_map(move |(target, owner, kind, name, _)| {
                (*owner == Some(parent)).then_some((*target, *kind, name.as_str()))
            })
    }

    pub(super) fn has_comment(&self, target: Target) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.0 == target && entry.4)
    }

    pub(in crate::documentation) fn lexical_parameter(
        &self,
        mut target: Target,
        name: &str,
    ) -> Option<Target> {
        loop {
            if let Some((target, _, _)) = self.children(target).find(|(target, kind, candidate)| {
                matches!(
                    kind,
                    Kind::Parameter | Kind::TypeParameter | Kind::ContextParameter
                ) && !matches!(target, Target::Declaration(Source::Receiver(_)))
                    && *candidate == name
                    && name != "_"
            }) {
                return Some(target);
            }
            target = self.entries.iter().find(|entry| entry.0 == target)?.1?;
        }
    }
}
