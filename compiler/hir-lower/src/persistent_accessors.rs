use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

pub(crate) fn build(
    lowerer: &Lowerer,
    properties: &hir::HirPropertyIdentities,
) -> Result<hir::HirPropertyAccessorIdentities, PersistentPropertyAccessorIdentityError> {
    PropertyAccessorIdentityBuilder {
        lowerer,
        properties,
    }
    .build()
}

#[derive(Debug)]
pub(crate) struct PersistentPropertyAccessorIdentityError {
    file: usize,
    span: Span,
    detail: PersistentPropertyAccessorIdentityErrorDetail,
}

impl PersistentPropertyAccessorIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentPropertyAccessorIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent property accessor identity: {}",
            self.detail
        )
    }
}

impl std::error::Error for PersistentPropertyAccessorIdentityError {}

#[derive(Debug)]
enum PersistentPropertyAccessorIdentityErrorDetail {
    UnknownAccessor {
        table: hir::AccessorTable,
        accessor: u32,
    },
    DuplicateAccessor {
        table: hir::AccessorTable,
        accessor: u32,
    },
    UnownedAccessor {
        table: hir::AccessorTable,
        accessor: u32,
    },
    InvalidIdentity(hir::HirPropertyAccessorIdentityError),
    MisalignedTable(hir::HirPropertyAccessorIdentityTableError),
}

impl fmt::Display for PersistentPropertyAccessorIdentityErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAccessor { table, accessor } => {
                write!(formatter, "property refers to unknown {table} {accessor}")
            }
            Self::DuplicateAccessor { table, accessor } => {
                write!(formatter, "property {table} {accessor} has multiple owners")
            }
            Self::UnownedAccessor { table, accessor } => {
                write!(
                    formatter,
                    "property {table} {accessor} has no logical property"
                )
            }
            Self::InvalidIdentity(error) => error.fmt(formatter),
            Self::MisalignedTable(error) => error.fmt(formatter),
        }
    }
}

struct PropertyAccessorIdentityBuilder<'a> {
    lowerer: &'a Lowerer,
    properties: &'a hir::HirPropertyIdentities,
}

impl PropertyAccessorIdentityBuilder<'_> {
    fn build(
        self,
    ) -> Result<hir::HirPropertyAccessorIdentities, PersistentPropertyAccessorIdentityError> {
        let mut getters = vec![None; self.lowerer.property_getters.len()];
        let mut setters = vec![None; self.lowerer.property_setters.len()];
        for (property_id, property) in self.lowerer.properties.iter() {
            self.bind(
                property_id,
                hir::AccessorTable::Getter,
                raw_index(property.capability.getter()),
                &mut getters,
            )?;
            if let Some(setter) = property.capability.setter() {
                self.bind(
                    property_id,
                    hir::AccessorTable::Setter,
                    raw_index(setter),
                    &mut setters,
                )?;
            }
        }

        let getter_identities = getters
            .into_iter()
            .enumerate()
            .map(|(index, property)| {
                let property = property.ok_or_else(|| {
                    self.accessor_failure(
                        hir::AccessorTable::Getter,
                        index,
                        PersistentPropertyAccessorIdentityErrorDetail::UnownedAccessor {
                            table: hir::AccessorTable::Getter,
                            accessor: index as u32,
                        },
                    )
                })?;
                hir::HirPropertyAccessorIdentity::getter(
                    property,
                    self.properties[property].property_owner(),
                )
                .map_err(|error| {
                    self.property_failure(
                        property,
                        PersistentPropertyAccessorIdentityErrorDetail::InvalidIdentity(error),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let setter_identities = setters
            .into_iter()
            .enumerate()
            .map(|(index, property)| {
                let property = property.ok_or_else(|| {
                    self.accessor_failure(
                        hir::AccessorTable::Setter,
                        index,
                        PersistentPropertyAccessorIdentityErrorDetail::UnownedAccessor {
                            table: hir::AccessorTable::Setter,
                            accessor: index as u32,
                        },
                    )
                })?;
                hir::HirPropertyAccessorIdentity::setter(
                    property,
                    self.properties[property].property_owner(),
                )
                .map_err(|error| {
                    self.property_failure(
                        property,
                        PersistentPropertyAccessorIdentityErrorDetail::InvalidIdentity(error),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        hir::HirPropertyAccessorIdentities::checked(
            &self.lowerer.properties,
            self.properties,
            &self.lowerer.property_getters,
            getter_identities,
            &self.lowerer.property_setters,
            setter_identities,
        )
        .map_err(|error| PersistentPropertyAccessorIdentityError {
            file: 0,
            span: Span { start: 0, end: 0 },
            detail: PersistentPropertyAccessorIdentityErrorDetail::MisalignedTable(error),
        })
    }

    fn bind(
        &self,
        property: hir::PropertyId,
        table: hir::AccessorTable,
        accessor: u32,
        owners: &mut [Option<hir::PropertyId>],
    ) -> Result<(), PersistentPropertyAccessorIdentityError> {
        let Some(owner) = owners.get_mut(accessor as usize) else {
            return Err(self.property_failure(
                property,
                PersistentPropertyAccessorIdentityErrorDetail::UnknownAccessor { table, accessor },
            ));
        };
        if owner.replace(property).is_some() {
            return Err(self.property_failure(
                property,
                PersistentPropertyAccessorIdentityErrorDetail::DuplicateAccessor {
                    table,
                    accessor,
                },
            ));
        }
        Ok(())
    }

    fn property_failure(
        &self,
        property: hir::PropertyId,
        detail: PersistentPropertyAccessorIdentityErrorDetail,
    ) -> PersistentPropertyAccessorIdentityError {
        let declaration = &self.lowerer.properties[property];
        PersistentPropertyAccessorIdentityError {
            file: self.lowerer.property_source_file(property).unwrap_or(0),
            span: declaration.span,
            detail,
        }
    }

    fn accessor_failure(
        &self,
        table: hir::AccessorTable,
        accessor: usize,
        detail: PersistentPropertyAccessorIdentityErrorDetail,
    ) -> PersistentPropertyAccessorIdentityError {
        let span = match table {
            hir::AccessorTable::Getter => {
                self.lowerer.property_getters
                    [hir::PropertyGetterId::from_raw((accessor as u32).into())]
                .span
            }
            hir::AccessorTable::Setter => {
                self.lowerer.property_setters
                    [hir::PropertySetterId::from_raw((accessor as u32).into())]
                .span
            }
        };
        PersistentPropertyAccessorIdentityError {
            file: 0,
            span,
            detail,
        }
    }
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
