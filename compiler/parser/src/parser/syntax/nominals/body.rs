use super::{NominalMember, ast};

pub(super) enum RecoveredBody {
    Nominal(ast::RecoveredNominal),
    Companion(Box<ast::RecoveredCompanionObject>),
}

impl RecoveredBody {
    pub(super) fn span(&self) -> ast::Span {
        match self {
            Self::Nominal(value) => value.span(),
            Self::Companion(value) => value.span,
        }
    }

    pub(super) fn recovery_mut(&mut self) -> &mut ast::RecoveredNominalMembers {
        match self {
            Self::Nominal(value) => value.recovery_mut(),
            Self::Companion(value) => &mut value.recovery,
        }
    }

    pub(super) fn push_recovered(&mut self, member: Self) -> Option<()> {
        match member {
            Self::Nominal(value) => self.recovery_mut().nested.push(value),
            Self::Companion(value) => {
                if self.has_companion() {
                    return None;
                }
                self.recovery_mut().companion = Some(value);
            }
        }
        Some(())
    }

    pub(super) fn push_member(&mut self, member: NominalMember) -> Option<()> {
        use ast::RecoveredNominal as Nominal;
        let companion = matches!(&member,
            NominalMember::Class(value) if matches!(**value, ast::ClassMember::Companion(_)))
            || matches!(&member, NominalMember::Struct(value) if matches!(**value, ast::StructMember::Companion(_)))
            || matches!(&member, NominalMember::Companion(_));
        if companion && self.has_companion() {
            return None;
        }
        match (self, member) {
            (Self::Nominal(Nominal::Class(owner)), NominalMember::Class(value)) => {
                owner.members.push(*value)
            }
            (Self::Nominal(Nominal::Object(owner)), NominalMember::Class(value)) => {
                if matches!(*value, ast::ClassMember::SecondaryConstructor(_)) {
                    return None;
                }
                owner.members.push(*value);
            }
            (Self::Companion(owner), NominalMember::Class(value)) => {
                if matches!(*value, ast::ClassMember::SecondaryConstructor(_)) {
                    return None;
                }
                owner.members.push(*value);
            }
            (Self::Nominal(Nominal::Struct(owner)), NominalMember::Struct(value)) => {
                owner.members.push(*value)
            }
            (Self::Nominal(Nominal::Interface(owner)), NominalMember::Function(value)) => {
                owner.methods.push(*value)
            }
            (Self::Nominal(Nominal::Interface(owner)), NominalMember::Property(value)) => {
                owner.properties.push(*value)
            }
            (Self::Nominal(Nominal::Interface(owner)), NominalMember::Nested(value)) => {
                owner.nested.push(*value)
            }
            (Self::Nominal(Nominal::Interface(owner)), NominalMember::Companion(value)) => {
                owner.companion = Some(*value)
            }
            (Self::Nominal(Nominal::Enum(owner)), NominalMember::Variant(value)) => {
                owner.variants.push(*value)
            }
            (Self::Nominal(Nominal::Enum(owner)), NominalMember::Function(value)) => {
                owner.methods.push(*value)
            }
            (Self::Nominal(Nominal::Enum(owner)), NominalMember::Property(value)) => {
                owner.properties.push(*value)
            }
            (Self::Nominal(Nominal::Enum(owner)), NominalMember::Nested(value)) => {
                owner.nested.push(*value)
            }
            (Self::Nominal(Nominal::Enum(owner)), NominalMember::Companion(value)) => {
                owner.companion = Some(*value)
            }
            _ => unreachable!("each nominal body records its own member syntax"),
        }
        Some(())
    }

    fn has_companion(&self) -> bool {
        use ast::RecoveredNominal as Nominal;
        let class_companion = |members: &[ast::ClassMember]| {
            members
                .iter()
                .any(|member| matches!(member, ast::ClassMember::Companion(_)))
        };
        match self {
            Self::Nominal(Nominal::Class(owner)) => {
                owner.recovery.companion.is_some() || class_companion(&owner.members)
            }
            Self::Nominal(Nominal::Object(owner)) => {
                owner.recovery.companion.is_some() || class_companion(&owner.members)
            }
            Self::Companion(owner) => {
                owner.recovery.companion.is_some() || class_companion(&owner.members)
            }
            Self::Nominal(Nominal::Struct(owner)) => {
                owner.recovery.companion.is_some()
                    || owner
                        .members
                        .iter()
                        .any(|member| matches!(member, ast::StructMember::Companion(_)))
            }
            Self::Nominal(Nominal::Interface(owner)) => {
                owner.recovery.companion.is_some() || owner.companion.is_some()
            }
            Self::Nominal(Nominal::Enum(owner)) => {
                owner.recovery.companion.is_some() || owner.companion.is_some()
            }
        }
    }
}
