//! Follow saved physical references without making image tables liveness roots.

use super::*;
use scoop_slib::ProgramLinkMember;

type Member = (ConeIdentity, SlibMemberId);

impl SelectedMembers {
    pub(super) fn retain_reachable<'a>(
        &mut self,
        closure: &ProgramLinkClosure,
        native_references: impl Iterator<Item = &'a str>,
    ) -> Result<(), LinkError> {
        let mut owners = BTreeMap::<&[u8], Member>::new();
        let mut references = BTreeMap::<Member, Vec<&[u8]>>::new();
        let mut pending = Vec::new();
        for (artifact, symbols) in closure.artifacts() {
            let cone = artifact.identity();
            let plans = &artifact.production().definitions;
            for member in symbols.members() {
                if !self.contains(cone, member.member()) {
                    continue;
                }
                let callable = match member {
                    ProgramLinkMember::Scoop { definitions, .. } => definitions
                        .iter()
                        .any(|id| plans[id].role == StrongDefinitionRole::CallableBody),
                    ProgramLinkMember::Generated { .. } => false,
                };
                if !callable {
                    // Keep storage, dispatch, initialization and native bridges.
                    // Their actual references retain the code they can invoke.
                    pending.push((cone, member.member()));
                }
            }
            for definition in symbols.defined_symbols().owners() {
                let member = (cone, definition.member());
                if self.members.contains(&member)
                    && let Some(previous) = owners.insert(definition.symbol(), member)
                {
                    return Err(error(format!(
                        "multiple selected definitions for {}: {previous:?}, {member:?}",
                        String::from_utf8_lossy(definition.symbol())
                    )));
                }
            }
            for requirement in symbols.requirements() {
                let member = (cone, requirement.member());
                if self.members.contains(&member) {
                    references
                        .entry(member)
                        .or_default()
                        .push(requirement.symbol());
                }
            }
        }
        pending
            .extend(native_references.filter_map(|symbol| owners.get(symbol.as_bytes()).copied()));
        let mut reached = BTreeSet::new();
        while let Some(member) = pending.pop() {
            if !reached.insert(member) {
                continue;
            }
            if let Some(symbols) = references.get(&member) {
                pending.extend(
                    symbols
                        .iter()
                        .filter_map(|symbol| owners.get(symbol).copied()),
                );
            }
        }
        self.members.retain(|member| reached.contains(member));
        for (artifact, symbols) in closure.artifacts() {
            let cone = artifact.identity();
            let definitions = self.definitions.get_mut(&cone).expect("selected Cone");
            definitions.clear();
            for member in symbols.members() {
                if self.members.contains(&(cone, member.member()))
                    && let ProgramLinkMember::Scoop {
                        definitions: retained,
                        ..
                    } = member
                {
                    // Registrations are in the same physical member as the body.
                    definitions.extend(retained);
                }
            }
        }
        Ok(())
    }
}
