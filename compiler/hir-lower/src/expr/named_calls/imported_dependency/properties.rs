use super::*;

pub(in crate::expr) struct ImportedExtensionAccessorSelection {
    pub template: hir::ImportedGenericCallableTemplateId,
    pub arguments: hir::NonEmptyVec<hir::TypeId>,
    pub receiver: hir::Expr,
    pub static_receiver_type: hir::TypeId,
    pub value_type: hir::TypeId,
    pub declaration_file: usize,
    pub declaration_span: ast::Span,
}

impl Lowerer {
    pub(in crate::expr) fn probe_imported_extension_accessor(
        &self,
        declaration: hir::ImportedCallableDeclaration,
        receiver: hir::Expr,
        name: &ast::Ident,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let static_type = receiver.ty;
        Box::new(self.clone()).probe_imported_generic(
            ImportedCallableCandidate::Declaration(Box::new(declaration)),
            name,
            ImportedProbeCall::lowered(&[], name.span),
            None,
            ImportedCallReceiver::Member {
                value: ImportedMemberReceiver::Value(receiver),
                static_type,
            },
            ImportedArgumentMap::lowered(&[], 0)
                .expect("a property getter accepts no explicit source arguments"),
        )
    }

    pub(in crate::expr) fn commit_imported_extension_accessor_signature(
        &mut self,
        probe: ImportedDependencyCallProbe,
    ) -> ImportedExtensionAccessorSelection {
        let ImportedDependencyCallProbe {
            implementation,
            state,
            receiver,
            result_type,
            declaration_file,
            declaration_span,
            ..
        } = probe;
        let ImportedCallImplementation::Generic {
            template: generic::ImportedGenericTarget::Function(template),
            arguments: hir::ImportedCallableArguments::Function(arguments),
        } = implementation
        else {
            unreachable!("a generic extension accessor retains its property binders")
        };
        let ImportedCallReceiver::Member {
            value: ImportedMemberReceiver::Value(receiver),
            static_type,
        } = receiver
        else {
            unreachable!("a property candidate has its actual receiver expression")
        };
        *self = *state;
        ImportedExtensionAccessorSelection {
            template,
            arguments: hir::NonEmptyVec::from_vec(arguments)
                .expect("a generic extension property has its declared binders"),
            receiver,
            static_receiver_type: static_type,
            value_type: result_type,
            declaration_file,
            declaration_span,
        }
    }
}
