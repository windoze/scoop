use super::*;

impl Lowerer {
    pub(super) fn imported_atomic_constructor_view(
        &mut self,
        target: NominalConstructorSource,
        owner: hir::SourceNominalId,
        span: Span,
    ) -> NominalConstructorView {
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .cloned()
            .expect("a resolved atomic retains its dependency declaration");
        let binders = declaration
            .interface
            .type_parameters()
            .binders()
            .iter()
            .collect::<Vec<_>>();
        let signatures = (0..binders.len())
            .map(|index| scoop_identity::SignatureTypeKey::Binder {
                depth: 0,
                index: index as u32,
            })
            .collect::<Vec<_>>();
        let (parameters, _) = self
            .prepare_imported_type_parameters(&binders, &signatures, span)
            .expect("the checked atomic contract retains its type parameter bounds");
        let arguments = parameters
            .iter()
            .map(|parameter| self.intern_type(hir::Type::Param(parameter.id)))
            .collect();
        let result = self
            .imported_nominal_application(owner, arguments)
            .expect("a checked atomic declaration has a complete application");
        self.atomic_constructor_view(target, parameters, result)
    }

    pub(super) fn atomic_constructor_view(
        &mut self,
        target: NominalConstructorSource,
        owner_parameters: Vec<hir::TypeParamDecl>,
        return_type: hir::TypeId,
    ) -> NominalConstructorView {
        let (_, ty) = self
            .atomic_value_type(return_type)
            .expect("an atomic constructor retains its complete application");
        NominalConstructorView {
            target,
            signature: DeclarationSignature {
                owner_parameters,
                callable_parameters: Vec::new(),
                value_parameters: vec![ValueParameter {
                    name: "initial".into(),
                    calling: ValueParameterCalling::Required,
                    ty,
                }],
                return_type,
            },
            argument_mode: ArgumentMode::Mixed,
        }
    }
}
