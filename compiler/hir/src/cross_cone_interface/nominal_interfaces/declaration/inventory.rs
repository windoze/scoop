use super::*;
use crate::CanonicalHirFoundation;
use scoop_identity::{DefinitionOwnerAtom, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

impl CanonicalNominalInterfacesV1 {
    /// Every source key owned by an included nominal must appear in its
    /// declaration relationships, including restricted and source-only members.
    pub fn validate_declared_relation_inventory(
        &self,
        foundation: &CanonicalHirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<(), NominalDeclarationInventoryError> {
        use Relation::*;
        let relations = foundation
            .type_source_constructor_records()
            .iter()
            .map(|record| (record.key(), Constructor(record.id())))
            .chain(
                foundation
                    .type_source_function_records()
                    .iter()
                    .map(|record| {
                        (
                            record.key(),
                            Member(NestedSourceMemberRefV1::Function(record.id())),
                        )
                    }),
            )
            .chain(
                foundation
                    .type_source_generic_function_records()
                    .iter()
                    .map(|record| {
                        (
                            record.key(),
                            Member(NestedSourceMemberRefV1::GenericFunction(record.id())),
                        )
                    }),
            )
            .chain(
                foundation
                    .type_source_property_records()
                    .iter()
                    .map(|record| {
                        (
                            record.key(),
                            Member(NestedSourceMemberRefV1::Property(record.id())),
                        )
                    }),
            )
            .chain(
                foundation
                    .type_source_nominal_records()
                    .iter()
                    .map(|record| (record.key(), Child(SourceNominalId::Concrete(record.id())))),
            )
            .chain(
                foundation
                    .type_source_generic_records()
                    .iter()
                    .map(|record| {
                        (
                            record.key(),
                            Child(SourceNominalId::GenericTemplate(record.id())),
                        )
                    }),
            );
        let path = WirePath::root().field(2);
        for (key, relation) in relations {
            meter.charge_work(self.declaration_count().max(1).ilog2() as u64 + 1, &path)?;
            let Some(owner) = parent(key) else { continue };
            let Some(record) = self.declaration(owner) else {
                continue;
            };
            let details = record.declaration_details();
            let count = details.constructors().values().len()
                + details.members().values().len()
                + details.children().values().len();
            meter.check_table_entries(count as u64, &path)?;
            meter.charge_work(count as u64 + 1, &path)?;
            let error = match relation {
                // Object initialization has a constructor identity but is not a
                // callable source constructor declaration (including privately).
                Constructor(_) if record.kind() == PublicNominalKindV1::Object => continue,
                Constructor(constructor)
                    if !details.constructors().values().contains(&constructor) =>
                {
                    NominalDeclarationInventoryError::MissingConstructor { owner, constructor }
                }
                Member(member) if !details.members().values().contains(&member) => {
                    NominalDeclarationInventoryError::MissingMember { owner, member }
                }
                Child(child) if !details.children().values().contains(&child) => {
                    NominalDeclarationInventoryError::MissingChild { owner, child }
                }
                _ => continue,
            };
            return Err(error);
        }
        Ok(())
    }
}

enum Relation {
    Constructor(PersistentConstructorId),
    Member(NestedSourceMemberRefV1),
    Child(SourceNominalId),
}

fn parent(key: &SourceDeclarationKey) -> Option<SourceNominalId> {
    match key.owners().owners().last()? {
        DefinitionOwnerAtom::Type(id) => Some(SourceNominalId::Concrete(*id)),
        DefinitionOwnerAtom::GenericType(id) => Some(SourceNominalId::GenericTemplate(*id)),
        _ => None,
    }
}

#[derive(Debug)]
pub enum NominalDeclarationInventoryError {
    Resource(WireError),
    MissingConstructor {
        owner: SourceNominalId,
        constructor: PersistentConstructorId,
    },
    MissingMember {
        owner: SourceNominalId,
        member: NestedSourceMemberRefV1,
    },
    MissingChild {
        owner: SourceNominalId,
        child: SourceNominalId,
    },
}

impl From<WireError> for NominalDeclarationInventoryError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for NominalDeclarationInventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::MissingConstructor { owner, constructor } => write!(
                f,
                "nominal {owner:?} omits declared constructor {constructor}"
            ),
            Self::MissingMember { owner, member } => {
                write!(f, "nominal {owner:?} omits declared member {member:?}")
            }
            Self::MissingChild { owner, child } => {
                write!(f, "nominal {owner:?} omits declared child {child:?}")
            }
        }
    }
}
impl std::error::Error for NominalDeclarationInventoryError {}
