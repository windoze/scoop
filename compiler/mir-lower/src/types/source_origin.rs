//! Source providers survive physical type transposition, including imports.

use scoop_hir::concrete as hir;
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, ExactTypeKey};
use scoop_mir::SourceExactTypeOrigin;

pub(super) fn lower(module: &hir::Module, ty: hir::TypeId) -> SourceExactTypeOrigin {
    match module.exact_type_identities[ty].key() {
        ExactTypeKey::Nominal(_) => SourceExactTypeOrigin::Nominal(provider(module, ty)),
        ExactTypeKey::NominalApplication { .. } => SourceExactTypeOrigin::NominalApplication(
            module
                .exact_type_identities
                .nominal_specialization(ty)
                .expect("a validated HIR nominal application has its specialization")
                .clone(),
        ),
        ExactTypeKey::Tuple(_)
        | ExactTypeKey::Function { .. }
        | ExactTypeKey::RawPointer(_)
        | ExactTypeKey::NativeFunctionPointer { .. } => SourceExactTypeOrigin::Structural,
    }
}

fn provider(module: &hir::Module, ty: hir::TypeId) -> ConeIdentity {
    let source = match &module.types[ty].kind {
        hir::TypeKind::Unit => return CoreBuiltinNominal::Unit.declaration_key().origin(),
        hir::TypeKind::Any => return CoreBuiltinNominal::Any.declaration_key().origin(),
        hir::TypeKind::Integer(kind) => match &module.core_protocols {
            hir::ConcreteCoreProtocols::Defined(protocols) => {
                &module.structs[protocols.fundamental_types.integers.owner(*kind)].origin
            }
            hir::ConcreteCoreProtocols::Imported(protocols) => {
                return protocols.fundamental_types().integer(*kind).provider();
            }
        },
        hir::TypeKind::Boolean => match &module.core_protocols {
            hir::ConcreteCoreProtocols::Defined(protocols) => {
                &module.structs[protocols.fundamental_types.boolean].origin
            }
            hir::ConcreteCoreProtocols::Imported(protocols) => {
                return protocols.fundamental_types().boolean().provider();
            }
        },
        hir::TypeKind::String => match &module.core_protocols {
            hir::ConcreteCoreProtocols::Defined(protocols) => {
                &module.classes[protocols.fundamental_types.string].origin
            }
            hir::ConcreteCoreProtocols::Imported(protocols) => {
                return protocols.fundamental_types().string().provider();
            }
        },
        hir::TypeKind::Struct(id) => &module.structs[*id].origin,
        hir::TypeKind::Enum(id) => &module.enums[*id].origin,
        hir::TypeKind::Interface(id) => &module.interfaces[*id].origin,
        hir::TypeKind::Class(id) => module
            .objects
            .iter()
            .find(|(_, object)| object.backing_class == *id)
            .map_or(&module.classes[*id].origin, |(_, object)| &object.origin),
        hir::TypeKind::Tuple(_)
        | hir::TypeKind::Function(_)
        | hir::TypeKind::Ptr(_)
        | hir::TypeKind::FunPtr(_) => {
            unreachable!("structural exact types have no nominal provider")
        }
    };
    source
        .source()
        .expect("source nominals and object backing relations retain their declarations")
        .declaration()
        .origin()
}
