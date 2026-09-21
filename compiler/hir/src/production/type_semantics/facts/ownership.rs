use super::*;

impl FactProjector<'_> {
    pub(super) fn is_core_leaf(&self, ty: concrete::TypeId) -> bool {
        matches!(
            self.local.types[ty].kind,
            concrete::TypeKind::Unit
                | concrete::TypeKind::Integer(_)
                | concrete::TypeKind::Boolean
                | concrete::TypeKind::String
                | concrete::TypeKind::Any
        )
    }

    pub(super) fn is_generic_application(&self, ty: concrete::TypeId) -> bool {
        match self.local.types[ty].kind {
            concrete::TypeKind::Struct(id) => !self.local.structs[id].type_arguments.is_empty(),
            concrete::TypeKind::Enum(id) => !self.local.enums[id].type_arguments.is_empty(),
            concrete::TypeKind::Class(id) => !self.local.classes[id].type_arguments.is_empty(),
            concrete::TypeKind::Interface(id) => {
                !self.local.interfaces[id].type_arguments.is_empty()
            }
            concrete::TypeKind::Unit
            | concrete::TypeKind::Integer(_)
            | concrete::TypeKind::Boolean
            | concrete::TypeKind::String
            | concrete::TypeKind::Any
            | concrete::TypeKind::Tuple(_)
            | concrete::TypeKind::Function(_)
            | concrete::TypeKind::Ptr(_)
            | concrete::TypeKind::FunPtr(_) => false,
        }
    }

    pub(super) fn is_locally_owned(&self, ty: concrete::TypeId) -> Result<bool, Error> {
        let exact = self.exact(ty)?;
        Ok(self.root_exacts.contains(&exact)
            || self.declared_origin(ty) == Some(self.local.cone)
            || matches!(
                self.local.types[ty].kind,
                concrete::TypeKind::Tuple(_)
                    | concrete::TypeKind::Function(_)
                    | concrete::TypeKind::Ptr(_)
                    | concrete::TypeKind::FunPtr(_)
            ))
    }

    pub(super) fn dependency_provider(&self, ty: concrete::TypeId) -> Option<ConeIdentity> {
        self.declared_origin(ty)
            .filter(|origin| *origin != self.local.cone)
    }

    fn declared_origin(&self, ty: concrete::TypeId) -> Option<ConeIdentity> {
        let origin = match self.local.types[ty].kind {
            concrete::TypeKind::Struct(id) => &self.local.structs[id].origin,
            concrete::TypeKind::Enum(id) => &self.local.enums[id].origin,
            concrete::TypeKind::Class(id) => &self.local.classes[id].origin,
            concrete::TypeKind::Interface(id) => &self.local.interfaces[id].origin,
            _ if self.is_core_leaf(ty) => {
                return Some(match &self.local.core_protocols {
                    concrete::ConcreteCoreProtocols::Defined(_) => self.local.cone,
                    concrete::ConcreteCoreProtocols::Imported(_) => ConeIdentity::CORE,
                });
            }
            _ => return None,
        };
        origin.source().map(|source| source.declaration().origin())
    }
}
