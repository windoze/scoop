//! Source providers survive physical type transposition, including imports.

use scoop_hir::concrete as hir;
use scoop_identity::{ConeIdentity, CoreBuiltinNominal, ExactTypeKey};
use scoop_mir::{self as mir, SourceExactTypeOrigin};

pub(crate) fn owned_builtin_types(module: &hir::Module) -> impl Iterator<Item = hir::TypeId> {
    module.types.iter().filter_map(|(id, ty)| {
        (matches!(ty.kind, hir::TypeKind::Unit | hir::TypeKind::Any)
            && matches!(
                lower(module, id),
                mir::SourceExactTypeOrigin::Nominal(provider) if provider == module.cone
            ))
        .then_some(id)
    })
}

/// Exact source types that have actually crossed the HIR -> MIR boundary.
/// Registration happens inside `Types::lower`, so recursive child types are
/// covered without forcing unused LocalConcrete declarations into MIR.
#[derive(Default)]
pub(crate) struct SourceExactTypeRegistry {
    entries: Vec<(hir::TypeId, mir::SourceExactTypeIdentity)>,
}

impl SourceExactTypeRegistry {
    pub(crate) fn record(&mut self, module: &hir::Module, source: hir::TypeId, lowered: mir::Type) {
        if let Some((_, existing)) = self.entries.iter().find(|(found, _)| *found == source) {
            assert_eq!(
                existing.ty(),
                &lowered,
                "one LocalConcrete HIR type must always lower to the same MIR type"
            );
            return;
        }
        assert!(
            self.entries.iter().all(|(_, existing)| {
                existing.ty() != &lowered
                    && existing.identity_record().id() != module.exact_type_identities[source].id()
            }),
            "source exact types transpose one-to-one into MIR"
        );
        let identity = mir::SourceExactTypeIdentity::checked(
            lowered,
            module.exact_type_identities[source].clone(),
            lower(module, source),
        )
        .expect("validated HIR exact types retain their complete provenance");
        self.entries.push((source, identity));
    }

    pub(crate) fn get(&self, ty: &mir::Type) -> Option<&mir::SourceExactTypeIdentity> {
        self.entries
            .iter()
            .find_map(|(_, entry)| (entry.ty() == ty).then_some(entry))
    }

    pub(crate) fn finish(self) -> mir::SourceExactTypeIdentities {
        mir::SourceExactTypeIdentities::checked(
            self.entries
                .into_iter()
                .map(|(_, identity)| identity)
                .collect(),
        )
        .expect("registered source exact types are one-to-one")
    }
}

fn lower(module: &hir::Module, ty: hir::TypeId) -> SourceExactTypeOrigin {
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
