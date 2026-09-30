use super::*;
use crate::types::ArrayKind;

impl Lowerer {
    pub(super) fn imported_array_constructor_view(
        &mut self,
        owner: hir::SourceNominalId,
        span: Span,
    ) -> NominalConstructorView {
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .cloned()
            .expect("a resolved array retains its dependency declaration");
        let binders = declaration
            .interface
            .type_parameters()
            .binders()
            .iter()
            .collect::<Vec<_>>();
        let (parameters, _) = self
            .prepare_imported_type_parameters(
                &binders,
                &[scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }],
                span,
            )
            .expect("the checked array contract has one unconstrained binder");
        let element = self.intern_type(hir::Type::Param(parameters[0].id));
        let result_type = self
            .imported_nominal_application(owner, vec![element])
            .expect("the checked array protocol has a complete dependency declaration");
        self.array_constructor_view(
            NominalConstructorSource::ImportedArray(owner),
            parameters,
            result_type,
        )
    }

    pub(super) fn array_constructor_view(
        &mut self,
        target: NominalConstructorSource,
        owner_parameters: Vec<hir::TypeParamDecl>,
        result_type: hir::TypeId,
    ) -> NominalConstructorView {
        let array = self
            .array_type_info(result_type)
            .expect("an array conversion retains its complete result application");
        let source_kind = match array.kind {
            ArrayKind::Immutable => ArrayKind::Mutable,
            ArrayKind::Mutable => ArrayKind::Immutable,
        };
        let source_type = self.array_type(source_kind, array.element);
        NominalConstructorView {
            target,
            signature: crate::call_resolution::candidates::DeclarationSignature {
                owner_parameters,
                callable_parameters: Vec::new(),
                value_parameters: vec![ValueParameter {
                    name: "source".to_owned(),
                    calling: ValueParameterCalling::Required,
                    ty: source_type,
                }],
                return_type: result_type,
            },
            argument_mode: ArgumentMode::Mixed,
        }
    }
}
