use super::*;
use crate::{
    DeclarationAccessSourceV1, NominalSourceCallablePayloadV1, NominalSupportCallableInterfaceV1,
    NominalSupportConstructorInterfaceV1, ProtectedCallableInterfaceV1,
    ProtectedConstructorInterfaceV1, ProtectedDefaultOwnerSourceV1,
};

/// References originate only in the already checked declaration surfaces.
#[derive(Clone, Copy, Debug)]
pub(super) enum Source<'a> {
    ProtectedCallable(&'a ProtectedCallableInterfaceV1),
    ProtectedConstructor(&'a ProtectedConstructorInterfaceV1),
    SupportCallable(&'a NominalSupportCallableInterfaceV1),
    SupportConstructor(&'a NominalSupportConstructorInterfaceV1),
}
impl<'a> Source<'a> {
    pub fn owner(&self) -> CallableTemplateOrigin {
        match self {
            Self::ProtectedCallable(record) => record.declaration(),
            Self::ProtectedConstructor(record) => {
                CallableTemplateOrigin::Constructor(record.declaration())
            }
            Self::SupportCallable(record) => record.declaration(),
            Self::SupportConstructor(record) => {
                CallableTemplateOrigin::Constructor(record.declaration())
            }
        }
    }
    pub fn payload(&self) -> &'a NominalSourceCallablePayloadV1 {
        match self {
            Self::ProtectedCallable(record) => record.payload(),
            Self::ProtectedConstructor(record) => record.payload(),
            Self::SupportCallable(record) => record.payload(),
            Self::SupportConstructor(record) => record.payload(),
        }
    }
    pub const fn access(&self) -> &'a DeclarationAccessSourceV1 {
        match self {
            Self::ProtectedCallable(record) => record.declaration_access(),
            Self::ProtectedConstructor(record) => record.declaration_access(),
            Self::SupportCallable(record) => record.declaration_access(),
            Self::SupportConstructor(record) => record.declaration_access(),
        }
    }
    pub fn validate<'s, A: NominalSupportCallableSemanticAuthority<E>, E>(
        self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        authority: &'s mut A,
    ) -> Result<ProtectedDefaultOwnerSourceV1<'s>, ProtectedSourceClosureError<E>>
    where
        'a: 's,
    {
        use ProtectedSourceClosureError as Error;
        let owner = self.owner();
        match self {
            Self::ProtectedCallable(record) => record
                .validate_source(graph, authority)
                .map(ProtectedDefaultOwnerSourceV1::Protected)
                .map_err(|error| Error::Protected {
                    owner,
                    error: Box::new(error),
                }),
            Self::ProtectedConstructor(record) => record
                .validate_source(graph, authority)
                .map(ProtectedDefaultOwnerSourceV1::Protected)
                .map_err(|error| Error::Protected {
                    owner,
                    error: Box::new(error),
                }),
            Self::SupportCallable(record) => record
                .validate_source(graph, authority)
                .map(ProtectedDefaultOwnerSourceV1::NominalSupport)
                .map_err(|error| Error::Support {
                    owner,
                    error: Box::new(error),
                }),
            Self::SupportConstructor(record) => record
                .validate_source(graph, authority)
                .map(ProtectedDefaultOwnerSourceV1::NominalSupport)
                .map_err(|error| Error::Support {
                    owner,
                    error: Box::new(error),
                }),
        }
    }
}
