use super::*;
use scoop_identity::ConeIdentity;

impl ExactTypeIdentityInputs<'_> {
    /// Uses the same nominal roles and source keys as exact identity building.
    /// Structural and generated types do not acquire a source nominal provider.
    pub(crate) fn source_nominal_provider(&self, ty: TypeId) -> Option<ConeIdentity> {
        let source = match self.types[ty].kind {
            TypeKind::Unit => return Some(CoreBuiltinNominal::Unit.declaration_key().origin()),
            TypeKind::Any => match self.core_types {
                ConcreteCoreTypeIdentityAuthority::Defined(core) => &self.classes[core.any].origin,
                ConcreteCoreTypeIdentityAuthority::Imported(core) => {
                    return Some(core.any().provider());
                }
            },
            TypeKind::Integer(kind) => match self.core_types {
                ConcreteCoreTypeIdentityAuthority::Defined(core) => {
                    &self.structs[core.integers.owner(kind)].origin
                }
                ConcreteCoreTypeIdentityAuthority::Imported(core) => {
                    return Some(core.integer(kind).provider());
                }
            },
            TypeKind::Boolean => match self.core_types {
                ConcreteCoreTypeIdentityAuthority::Defined(core) => {
                    &self.structs[core.boolean].origin
                }
                ConcreteCoreTypeIdentityAuthority::Imported(core) => {
                    return Some(core.boolean().provider());
                }
            },
            TypeKind::String => match self.core_types {
                ConcreteCoreTypeIdentityAuthority::Defined(core) => {
                    &self.classes[core.string].origin
                }
                ConcreteCoreTypeIdentityAuthority::Imported(core) => {
                    return Some(core.string().provider());
                }
            },
            TypeKind::Struct(id) => &self.structs[id].origin,
            TypeKind::Enum(id) => &self.enums[id].origin,
            TypeKind::Class(id) => &self.classes[id].origin,
            TypeKind::Interface(id) => &self.interfaces[id].origin,
            TypeKind::Tuple(_) | TypeKind::Function(_) | TypeKind::Ptr(_) | TypeKind::FunPtr(_) => {
                return None;
            }
        };
        source.source().map(|source| source.declaration().origin())
    }
}

#[cfg(test)]
mod tests;
