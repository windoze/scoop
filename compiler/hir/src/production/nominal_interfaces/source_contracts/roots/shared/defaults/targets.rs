use super::*;

impl SourceRoots {
    pub(super) fn default_callable(
        &mut self,
        export: &ExportHir,
        target: ExportDefaultCallableTarget,
        index: &super::super::super::Index,
        roots: &mut Roots,
    ) -> Result<(), Error> {
        match target {
            ExportDefaultCallableTarget::Callable(callable) => {
                self.function(export, callable.function(export), roots)
            }
            ExportDefaultCallableTarget::FunctionAddress(function) => {
                self.function(export, function, roots)
            }
            ExportDefaultCallableTarget::Bound(id) => self.bound(export, id, index, roots),
            ExportDefaultCallableTarget::DerivedEquality(id) => roots.require_field_type(
                export,
                index,
                export.derived_equality_applications[id].owner_ty,
            ),
            ExportDefaultCallableTarget::CallableReference(id) => {
                match &export.callable_references[id].target {
                    CallableReferenceTarget::Named(callable)
                    | CallableReferenceTarget::BoundExtension {
                        callee: callable, ..
                    } => self.function(export, callable.function(export), roots),
                    CallableReferenceTarget::BoundMember { callee, .. } => match *callee {
                        MethodCallee::Callable(callable) => {
                            self.function(export, callable.function(export), roots)
                        }
                        MethodCallee::Bound(id) => self.bound(export, id, index, roots),
                        MethodCallee::DerivedEquality(id) => roots.require_field_type(
                            export,
                            index,
                            export.derived_equality_applications[id].owner_ty,
                        ),
                    },
                    // The lifted declaration and its dependencies are attached
                    // to this default body, rather than a top-level record.
                    CallableReferenceTarget::Local { .. }
                    | CallableReferenceTarget::Imported(_) => Ok(()),
                }
            }
            // Imported calls keep the actual provider; attached lexical bodies
            // keep their own generated identity and reference closure.
            ExportDefaultCallableTarget::ImportedDependency(_)
            | ExportDefaultCallableTarget::ImportedGeneric(_)
            | ExportDefaultCallableTarget::ImportedBound(_)
            | ExportDefaultCallableTarget::LocalFunction(_)
            | ExportDefaultCallableTarget::Lambda(_)
            | ExportDefaultCallableTarget::AnonymousFunction(_) => Ok(()),
        }
    }

    fn bound(
        &mut self,
        export: &ExportHir,
        id: BoundCallableRefId,
        index: &super::super::super::Index,
        roots: &mut Roots,
    ) -> Result<(), Error> {
        let (ty, function) = match export.bound_callable_refs[id].source {
            BoundCallableSource::Class { bound, callable } => (
                export.class_applications[bound].canonical_type,
                callable.function(export),
            ),
            BoundCallableSource::Interface { bound, member } => (
                export.interface_applications[bound].canonical_type,
                export.interface_methods[member].function,
            ),
        };
        roots.require_field_type(export, index, ty)?;
        self.function(export, function, roots)
    }
}
