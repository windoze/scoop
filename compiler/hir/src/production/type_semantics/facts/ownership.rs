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
            || self.core_owned(ty)
            || matches!(
                self.local.types[ty].kind,
                concrete::TypeKind::Tuple(_)
                    | concrete::TypeKind::Function(_)
                    | concrete::TypeKind::Ptr(_)
                    | concrete::TypeKind::FunPtr(_)
            ))
    }

    fn core_owned(&self, ty: concrete::TypeId) -> bool {
        if !matches!(self.provider, FactProvider::CoreBootstrap) {
            return false;
        }
        let origin = match self.local.types[ty].kind {
            concrete::TypeKind::Struct(id) => &self.local.structs[id].origin,
            concrete::TypeKind::Enum(id) => &self.local.enums[id].origin,
            concrete::TypeKind::Class(id) => &self.local.classes[id].origin,
            concrete::TypeKind::Interface(id) => &self.local.interfaces[id].origin,
            _ => return self.is_core_leaf(ty),
        };
        origin
            .source()
            .is_some_and(|source| source.declaration().origin() == ConeIdentity::CORE)
    }
}
