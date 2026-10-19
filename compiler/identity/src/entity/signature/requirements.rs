//! Source-independent call shape retained in a generic declaration's key.

use super::SignatureTypeKey;

mod wire;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemberRequirementKey<T = SignatureTypeKey> {
    pub subject: T,
    pub call: RequiredMemberKey<T>,
    pub arguments: Vec<RequiredArgumentKey<T>>,
    pub result: T,
    pub effects: RequiredEffectsKey,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RequiredMemberKey<T> {
    Method {
        name: String,
        type_arguments: Vec<Option<T>>,
    },
    Constructor,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RequiredArgumentKey<T> {
    pub name: Option<String>,
    pub spread: bool,
    pub ty: T,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RequiredEffectsKey {
    pub is_suspend: bool,
    pub is_unsafe: bool,
    pub is_no_gc: bool,
}

impl<T> MemberRequirementKey<T> {
    pub fn types(&self) -> impl Iterator<Item = &T> {
        let type_arguments = match &self.call {
            RequiredMemberKey::Method { type_arguments, .. } => type_arguments.as_slice(),
            RequiredMemberKey::Constructor => &[],
        };
        [&self.subject, &self.result]
            .into_iter()
            .chain(self.arguments.iter().map(|argument| &argument.ty))
            .chain(type_arguments.iter().flatten())
    }

    pub fn try_map<U, E>(
        &self,
        mut map: impl FnMut(&T) -> Result<U, E>,
    ) -> Result<MemberRequirementKey<U>, E> {
        Ok(MemberRequirementKey {
            subject: map(&self.subject)?,
            call: match &self.call {
                RequiredMemberKey::Constructor => RequiredMemberKey::Constructor,
                RequiredMemberKey::Method {
                    name,
                    type_arguments,
                } => RequiredMemberKey::Method {
                    name: name.clone(),
                    type_arguments: type_arguments
                        .iter()
                        .map(|ty| ty.as_ref().map(&mut map).transpose())
                        .collect::<Result<_, _>>()?,
                },
            },
            arguments: self
                .arguments
                .iter()
                .map(|argument| {
                    Ok(RequiredArgumentKey {
                        name: argument.name.clone(),
                        spread: argument.spread,
                        ty: map(&argument.ty)?,
                    })
                })
                .collect::<Result<_, E>>()?,
            result: map(&self.result)?,
            effects: self.effects,
        })
    }
}
