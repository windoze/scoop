use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;
use crate::persistent_types::identity_inputs;

pub(crate) fn build(
    lowerer: &Lowerer,
    nominals: &hir::HirNominalIdentities,
    core_types: hir::HirCoreTypeIdentityAuthority<'_>,
) -> Result<hir::HirConstructorIdentities, PersistentConstructorIdentityError> {
    ConstructorIdentityBuilder {
        lowerer,
        nominals,
        type_inputs: identity_inputs(lowerer, nominals, core_types),
        class_identities: vec![None; lowerer.class_constructors.len()],
        visiting: vec![false; lowerer.class_constructors.len()],
    }
    .build()
}

#[derive(Debug)]
pub(crate) struct PersistentConstructorIdentityError {
    file: usize,
    span: Span,
    detail: PersistentConstructorIdentityErrorDetail,
}

impl PersistentConstructorIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentConstructorIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent constructor identity: {}",
            self.detail
        )
    }
}

impl std::error::Error for PersistentConstructorIdentityError {}

#[derive(Debug)]
enum PersistentConstructorIdentityErrorDetail {
    GeneratedOwner,
    InvalidSourceIdentity(hir::HirSourceConstructorIdentityError),
    InvalidClassIdentity(hir::HirConstructorIdentityError),
    AdapterCycle,
    AdapterSourceIdentity,
    InvalidRelation(hir::HirConstructorIdentityError),
}

impl fmt::Display for PersistentConstructorIdentityErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneratedOwner => {
                formatter.write_str("a source constructor has no source nominal owner")
            }
            Self::InvalidSourceIdentity(error) => error.fmt(formatter),
            Self::InvalidClassIdentity(error) | Self::InvalidRelation(error) => {
                error.fmt(formatter)
            }
            Self::AdapterCycle => formatter.write_str("constructor adapter relation is cyclic"),
            Self::AdapterSourceIdentity => {
                formatter.write_str("constructor adapter does not target a source constructor")
            }
        }
    }
}

struct ConstructorIdentityBuilder<'a> {
    lowerer: &'a Lowerer,
    nominals: &'a hir::HirNominalIdentities,
    type_inputs: hir::HirTypeIdentityInputs<'a>,
    class_identities: Vec<Option<hir::HirClassConstructorIdentity>>,
    visiting: Vec<bool>,
}

impl ConstructorIdentityBuilder<'_> {
    fn build(
        mut self,
    ) -> Result<hir::HirConstructorIdentities, PersistentConstructorIdentityError> {
        let mut structs = Vec::with_capacity(self.lowerer.struct_constructors.len());
        for (id, constructor) in self.lowerer.struct_constructors.iter() {
            let owner = self.nominals[constructor.owner].source().ok_or_else(|| {
                self.struct_failure(id, PersistentConstructorIdentityErrorDetail::GeneratedOwner)
            })?;
            structs.push(self.source_record(
                owner,
                &self.lowerer.structs[constructor.owner].type_params,
                &constructor.parameters,
                |detail| self.struct_failure(id, detail),
            )?);
        }

        let ids = self
            .lowerer
            .class_constructors
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in ids {
            self.resolve_class(id)?;
        }
        let classes = self
            .class_identities
            .into_iter()
            .enumerate()
            .map(|(index, identity)| {
                identity.ok_or(PersistentConstructorIdentityError {
                    file: 0,
                    span: Span { start: 0, end: 0 },
                    detail: PersistentConstructorIdentityErrorDetail::InvalidRelation(
                        hir::HirConstructorIdentityError::IdentityKind {
                            constructor: index as u32,
                        },
                    ),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        hir::HirConstructorIdentities::checked(
            hir::HirConstructorIdentityInputs {
                type_inputs: self.type_inputs,
                struct_constructors: &self.lowerer.struct_constructors,
                class_constructors: &self.lowerer.class_constructors,
                class_constructor_applications: &self.lowerer.class_constructor_applications,
            },
            structs,
            classes,
        )
        .map_err(|error| PersistentConstructorIdentityError {
            file: 0,
            span: Span { start: 0, end: 0 },
            detail: PersistentConstructorIdentityErrorDetail::InvalidRelation(error),
        })
    }

    fn resolve_class(
        &mut self,
        id: hir::ClassConstructorId,
    ) -> Result<hir::HirClassConstructorIdentity, PersistentConstructorIdentityError> {
        let index = local_index(id);
        if let Some(identity) = &self.class_identities[index] {
            return Ok(identity.clone());
        }
        if std::mem::replace(&mut self.visiting[index], true) {
            return Err(
                self.class_failure(id, PersistentConstructorIdentityErrorDetail::AdapterCycle)
            );
        }
        let constructor = &self.lowerer.class_constructors[id];
        let identity = match constructor.identity_kind {
            hir::ClassConstructorIdentityKind::Source => {
                let (owner, parameters) = if let Some(object) =
                    self.lowerer.object_by_backing_class.get(&constructor.owner)
                {
                    (&self.nominals[*object], &[][..])
                } else {
                    (
                        &self.nominals[constructor.owner],
                        self.lowerer.classes[constructor.owner]
                            .type_params
                            .as_slice(),
                    )
                };
                let owner = owner.source().ok_or_else(|| {
                    self.class_failure(id, PersistentConstructorIdentityErrorDetail::GeneratedOwner)
                })?;
                let record =
                    self.source_record(owner, parameters, &constructor.parameters, |detail| {
                        self.class_failure(id, detail)
                    })?;
                hir::HirClassConstructorIdentity::Source(record)
            }
            hir::ClassConstructorIdentityKind::ZeroArgumentAdapter { source } => {
                if local_index(source) >= self.lowerer.class_constructors.len() {
                    return Err(self.class_failure(
                        id,
                        PersistentConstructorIdentityErrorDetail::AdapterSourceIdentity,
                    ));
                }
                let source_identity = self.resolve_class(source)?;
                let Some(source_record) = source_identity.source_record() else {
                    return Err(self.class_failure(
                        id,
                        PersistentConstructorIdentityErrorDetail::AdapterSourceIdentity,
                    ));
                };
                hir::HirClassConstructorIdentity::zero_argument_adapter(source, source_record.id())
                    .map_err(|error| {
                        self.class_failure(
                            id,
                            PersistentConstructorIdentityErrorDetail::InvalidClassIdentity(error),
                        )
                    })?
            }
        };
        self.visiting[index] = false;
        self.class_identities[index] = Some(identity.clone());
        Ok(identity)
    }

    fn source_record(
        &self,
        owner: &hir::HirSourceNominalIdentity,
        type_parameters: &[hir::TypeParamDecl],
        parameters: &[hir::ConstructorParameter],
        failure: impl Fn(PersistentConstructorIdentityErrorDetail) -> PersistentConstructorIdentityError,
    ) -> Result<hir::HirSourceConstructorIdentity, PersistentConstructorIdentityError> {
        hir::derive_source_constructor_identity(
            self.type_inputs,
            owner,
            type_parameters,
            parameters,
        )
        .map_err(|error| {
            failure(PersistentConstructorIdentityErrorDetail::InvalidSourceIdentity(error))
        })
    }

    fn struct_failure(
        &self,
        id: hir::StructConstructorId,
        detail: PersistentConstructorIdentityErrorDetail,
    ) -> PersistentConstructorIdentityError {
        let constructor = &self.lowerer.struct_constructors[id];
        PersistentConstructorIdentityError {
            file: constructor.origin.file as usize,
            span: constructor.span,
            detail,
        }
    }

    fn class_failure(
        &self,
        id: hir::ClassConstructorId,
        detail: PersistentConstructorIdentityErrorDetail,
    ) -> PersistentConstructorIdentityError {
        let constructor = &self.lowerer.class_constructors[id];
        PersistentConstructorIdentityError {
            file: constructor.origin.file as usize,
            span: constructor.span,
            detail,
        }
    }
}

fn local_index<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

#[cfg(test)]
mod tests;
