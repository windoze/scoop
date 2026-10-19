use super::*;

/// Declaration identity is distinct from callable, slot and application ids.
/// The original parameter identity survives alpha-renaming of contract types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemberRequirementId {
    pub declaration_parameter: TypeParamId,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRequirement {
    pub id: MemberRequirementId,
    pub contract: MemberRequirementContract,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRequirementContract {
    /// The instance/companion receiver, or the type to construct.
    pub subject: TypeId,
    pub call: RequiredCallKind,
    pub arguments: Vec<RequiredCallArgument>,
    pub result: TypeId,
    pub effects: RequiredCallEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequiredCallKind {
    Method {
        name: String,
        type_arguments: Vec<RequiredTypeArgument>,
    },
    Constructor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredTypeArgument {
    Explicit(TypeId),
    Infer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredCallArgument {
    pub name: Option<String>,
    pub spread: bool,
    pub ty: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequiredCallEffects {
    pub is_suspend: bool,
    pub safety: Safety,
    pub gc_effect: GcEffect,
}

impl MemberRequirementContract {
    pub fn type_arguments(&self) -> &[RequiredTypeArgument] {
        match &self.call {
            RequiredCallKind::Method { type_arguments, .. } => type_arguments,
            RequiredCallKind::Constructor => &[],
        }
    }

    pub fn types(&self) -> impl Iterator<Item = TypeId> + '_ {
        [self.subject, self.result]
            .into_iter()
            .chain(self.arguments.iter().map(|argument| argument.ty))
            .chain(
                self.type_arguments()
                    .iter()
                    .filter_map(|argument| match argument {
                        RequiredTypeArgument::Explicit(ty) => Some(*ty),
                        RequiredTypeArgument::Infer => None,
                    }),
            )
    }
}

impl MemberRequirementContract {
    pub fn key<T, E>(
        &self,
        mut map: impl FnMut(TypeId) -> Result<T, E>,
    ) -> Result<scoop_identity::MemberRequirementKey<T>, E> {
        use scoop_identity::{
            MemberRequirementKey, RequiredArgumentKey, RequiredEffectsKey, RequiredMemberKey,
        };
        Ok(MemberRequirementKey {
            subject: map(self.subject)?,
            call: match &self.call {
                RequiredCallKind::Constructor => RequiredMemberKey::Constructor,
                RequiredCallKind::Method {
                    name,
                    type_arguments,
                } => RequiredMemberKey::Method {
                    name: name.clone(),
                    type_arguments: type_arguments
                        .iter()
                        .map(|argument| match argument {
                            RequiredTypeArgument::Explicit(ty) => map(*ty).map(Some),
                            RequiredTypeArgument::Infer => Ok(None),
                        })
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
                        ty: map(argument.ty)?,
                    })
                })
                .collect::<Result<_, E>>()?,
            result: map(self.result)?,
            effects: RequiredEffectsKey {
                is_suspend: self.effects.is_suspend,
                is_unsafe: self.effects.safety == Safety::Unsafe,
                is_no_gc: self.effects.gc_effect == GcEffect::NoGc,
            },
        })
    }

    pub fn from_key(key: scoop_identity::MemberRequirementKey<TypeId>) -> Self {
        use scoop_identity::RequiredMemberKey;
        Self {
            subject: key.subject,
            call: match key.call {
                RequiredMemberKey::Constructor => RequiredCallKind::Constructor,
                RequiredMemberKey::Method {
                    name,
                    type_arguments,
                } => RequiredCallKind::Method {
                    name,
                    type_arguments: type_arguments
                        .into_iter()
                        .map(|ty| {
                            ty.map_or(RequiredTypeArgument::Infer, RequiredTypeArgument::Explicit)
                        })
                        .collect(),
                },
            },
            arguments: key
                .arguments
                .into_iter()
                .map(|argument| RequiredCallArgument {
                    name: argument.name,
                    spread: argument.spread,
                    ty: argument.ty,
                })
                .collect(),
            result: key.result,
            effects: RequiredCallEffects {
                is_suspend: key.effects.is_suspend,
                safety: if key.effects.is_unsafe {
                    Safety::Unsafe
                } else {
                    Safety::Safe
                },
                gc_effect: if key.effects.is_no_gc {
                    GcEffect::NoGc
                } else {
                    GcEffect::Managed
                },
            },
        }
    }

    pub fn map_types(&mut self, mut map: impl FnMut(TypeId) -> TypeId) {
        let key = self
            .key(|ty| Ok::<_, std::convert::Infallible>(map(ty)))
            .unwrap();
        *self = Self::from_key(key);
    }
}

/// A checked call template uses independent local inputs, never a synthetic function identity.
#[derive(Debug, Clone)]
pub struct MemberRequirementImplementation {
    pub inputs: Vec<LocalId>,
    pub body: Body,
    pub result: Expr,
}

/// An invocation of a definition-site structural capability. Its inputs have
/// already been evaluated in source order; no runtime dictionary is involved.
#[derive(Debug, Clone)]
pub struct MemberRequirementCall {
    pub contract: MemberRequirementContract,
    pub binding: MemberRequirementBindingValue,
    pub arguments: Vec<Expr>,
    pub result: LocalId,
    pub origin: ExpressionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MemberRequirementBindingValue {
    Implementation {
        implementation: MemberRequirementImplementationId,
        type_arguments: Vec<TypeId>,
    },
    Forwarded(MemberRequirementId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberRequirementBinding {
    pub requirement: MemberRequirementId,
    pub value: MemberRequirementBindingValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRequirementOwner {
    Source(FunctionId),
    Imported(ImportedGenericCallableTemplateId),
}
