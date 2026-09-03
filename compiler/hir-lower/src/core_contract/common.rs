use super::*;

impl Lowerer {
    pub(crate) fn require_intrinsic(
        &mut self,
        kind: hir::IntrinsicFunctionKind,
        files: &[ast::SourceFile],
    ) -> Option<FunctionId> {
        if let Some(&(function, _provider)) = self.intrinsic_functions.get(&kind) {
            return Some(function);
        }
        self.current_file = 0;
        self.error(
            files[0].span,
            format!(
                "scoop.core must define exactly one `{}` intrinsic",
                kind.name()
            ),
        );
        None
    }

    pub(crate) fn is_type_param(&self, ty: TypeId, index: u32) -> bool {
        matches!(
            self.types[ty],
            Type::Param(param) if param.into_raw() == index
        )
    }

    pub(crate) fn is_ptr_param(&self, ty: TypeId, index: u32) -> bool {
        matches!(
            self.types[ty],
            Type::Ptr(pointee) if self.is_type_param(pointee, index)
        )
    }

    pub(crate) fn is_interface_param(
        &self,
        ty: TypeId,
        interface: InterfaceId,
        index: u32,
    ) -> bool {
        let Type::Interface(application) = self.types[ty] else {
            return false;
        };
        let application = &self.interface_applications[application];
        application.template == interface
            && matches!(application.arguments.as_slice(), [arg] if self.is_type_param(*arg, index))
    }

    pub(crate) fn class_descends_from(&self, class: ClassId, root: ClassId) -> bool {
        let mut current = Some(class);
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if id == root {
                return true;
            }
            if !visited.insert(id) {
                return false;
            }
            current = self.classes[id].base_class.as_ref().map(|base| {
                let Type::Class(application) = self.types[*base] else {
                    unreachable!("resolved class bases are class applications")
                };
                self.class_applications[application].template
            });
        }
        false
    }
}
