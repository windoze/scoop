use std::fmt;

use scoop_hir as hir;
use scoop_identity::{CborIdentityRecord, DispatchSlotKey};

use crate::Lowerer;

#[derive(Clone)]
pub(crate) enum VirtualMethodRoot {
    Local(hir::FunctionId),
    Imported(hir::HirDispatchSlotIdentity),
}

pub(crate) fn build(
    lowerer: &Lowerer,
    functions: &hir::HirFunctionIdentities,
    accessors: &hir::HirPropertyAccessorIdentities,
) -> Result<hir::HirDispatchSlotIdentities, PersistentDispatchSlotIdentityError> {
    let mut roots = lowerer
        .virtual_method_roots
        .iter()
        .map(|(family, root)| (*family, root.clone()))
        .collect::<Vec<_>>();
    roots.sort_by_key(|(family, _)| family.into_raw());
    let virtual_slots = roots
        .into_iter()
        .map(|(family, root)| {
            let root = match root {
                VirtualMethodRoot::Local(root) => root,
                VirtualMethodRoot::Imported(record) => {
                    return Ok((
                        family,
                        hir::HirVirtualDispatchSlotIdentity::Imported(record),
                    ));
                }
            };
            let key = dispatch_key(functions, accessors, root, DispatchDeclarationKind::Virtual)
                .map_err(|detail| failure(lowerer, root, detail))?;
            let record = CborIdentityRecord::from_key(key).map_err(|error| {
                failure(
                    lowerer,
                    root,
                    PersistentDispatchSlotIdentityErrorDetail::Hash(error.to_string()),
                )
            })?;
            Ok((
                family,
                hir::HirVirtualDispatchSlotIdentity::Local { root, record },
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let interface_slots = lowerer
        .interface_method_entities
        .iter()
        .map(|(_, member)| {
            let key = dispatch_key(
                functions,
                accessors,
                member.function,
                DispatchDeclarationKind::Interface,
            )
            .map_err(|detail| failure(lowerer, member.function, detail))?;
            CborIdentityRecord::from_key(key).map_err(|error| {
                failure(
                    lowerer,
                    member.function,
                    PersistentDispatchSlotIdentityErrorDetail::Hash(error.to_string()),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    hir::HirDispatchSlotIdentities::checked(
        hir::HirDispatchSlotIdentityInputs {
            functions: &lowerer.functions,
            function_identities: functions,
            property_accessor_identities: accessors,
            interface_methods: &lowerer.interface_method_entities,
        },
        virtual_slots,
        interface_slots,
    )
    .map_err(|error| PersistentDispatchSlotIdentityError {
        file: 0,
        span: scoop_ast::Span { start: 0, end: 0 },
        detail: PersistentDispatchSlotIdentityErrorDetail::Relation(error),
    })
}

#[derive(Clone, Copy)]
enum DispatchDeclarationKind {
    Virtual,
    Interface,
}

fn dispatch_key(
    functions: &hir::HirFunctionIdentities,
    accessors: &hir::HirPropertyAccessorIdentities,
    function: hir::FunctionId,
    kind: DispatchDeclarationKind,
) -> Result<DispatchSlotKey, PersistentDispatchSlotIdentityErrorDetail> {
    match &functions[function] {
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
            Ok(match kind {
                DispatchDeclarationKind::Virtual => DispatchSlotKey::virtual_method(record.id()),
                DispatchDeclarationKind::Interface => {
                    DispatchSlotKey::interface_method(record.id())
                }
            })
        }
        hir::HirFunctionIdentity::PropertyAccessor(hir::HirPropertyAccessorFunction::Getter(
            getter,
        )) => accessors
            .get_getter(*getter)
            .map(|identity| DispatchSlotKey::property_getter(identity.id()))
            .ok_or(PersistentDispatchSlotIdentityErrorDetail::InvalidOwner),
        hir::HirFunctionIdentity::PropertyAccessor(hir::HirPropertyAccessorFunction::Setter(
            setter,
        )) => accessors
            .get_setter(*setter)
            .map(|identity| DispatchSlotKey::property_setter(identity.id()))
            .ok_or(PersistentDispatchSlotIdentityErrorDetail::InvalidOwner),
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(_))
        | hir::HirFunctionIdentity::LexicalGenerated(_)
        | hir::HirFunctionIdentity::Initialization { .. }
        | hir::HirFunctionIdentity::DerivedEquality(_)
        | hir::HirFunctionIdentity::TupleEncoding(_) => {
            Err(PersistentDispatchSlotIdentityErrorDetail::InvalidOwner)
        }
    }
}

fn failure(
    lowerer: &Lowerer,
    function: hir::FunctionId,
    detail: PersistentDispatchSlotIdentityErrorDetail,
) -> PersistentDispatchSlotIdentityError {
    PersistentDispatchSlotIdentityError {
        file: lowerer.function_files[&function],
        span: lowerer.functions[function].span,
        detail,
    }
}

#[derive(Debug)]
pub(crate) struct PersistentDispatchSlotIdentityError {
    file: usize,
    span: scoop_ast::Span,
    detail: PersistentDispatchSlotIdentityErrorDetail,
}

impl PersistentDispatchSlotIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> scoop_ast::Span {
        self.span
    }
}

#[derive(Debug)]
enum PersistentDispatchSlotIdentityErrorDetail {
    InvalidOwner,
    Hash(String),
    Relation(hir::HirDispatchSlotIdentityError),
}

impl fmt::Display for PersistentDispatchSlotIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("cannot derive persistent dispatch slot identity: ")?;
        match &self.detail {
            PersistentDispatchSlotIdentityErrorDetail::InvalidOwner => {
                formatter.write_str("dispatch owner is not a plain function or property accessor")
            }
            PersistentDispatchSlotIdentityErrorDetail::Hash(error) => error.fmt(formatter),
            PersistentDispatchSlotIdentityErrorDetail::Relation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PersistentDispatchSlotIdentityError {}

#[cfg(test)]
mod tests;
