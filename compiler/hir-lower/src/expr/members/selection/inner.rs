use super::*;
use crate::call_resolution::candidates::NominalConstructorSource;
use crate::constructor_resolution::NominalConstructorCall;

impl Lowerer {
    pub(in crate::expr) fn bound_inner_constructor(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        expected: Option<TypeId>,
    ) -> Result<Option<(hir::Expr, hir::SourceNominalId, Option<TypeId>)>, ()> {
        let Some(mut receiver) = self.enclosing_receiver_class(receiver) else {
            return Ok(None);
        };
        let mut seen = Vec::new();
        loop {
            let Type::Class(application) = self.types[receiver.ty] else {
                return Ok(None);
            };
            if seen.contains(&application) {
                return Ok(None);
            };
            seen.push(application);
            let host = self.class_applications[application].clone();
            let nested = match self.resolve_nested_type_name(
                host.template,
                name,
                &[],
                Some(receiver.ty),
                None,
            ) {
                Ok(value) => value,
                Err(()) => return Err(()),
            };
            let nested = match nested {
                Some(crate::types::ResolvedTypeName::Nominal(owner)) => Some((owner, expected)),
                Some(crate::types::ResolvedTypeName::Applied(ty)) => self
                    .nominal_application(ty)
                    .map(|value| (value.template, Some(ty))),
                _ => None,
            };
            if let Some((owner, expected)) = nested {
                if self.nominal_enclosing_owner(owner) == Some(host.template) {
                    return Ok(Some((receiver, owner, expected)));
                }
                return Ok(None);
            };
            if matches!(
                self.nominal_owners.get(&host.template),
                Some(crate::Owner::Object(_))
            ) {
                return Ok(None);
            };
            let Some(base) = self.class_definition(host.template).base_class else {
                return Ok(None);
            };
            let base = self.instantiate_ty(base, &host.arguments);
            receiver = self.adapt_to(receiver, base);
        }
    }

    pub(super) fn inner_member_constructor_probes(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        expected: Option<TypeId>,
    ) -> (Vec<NamedFunctionLikeProbe>, Option<Box<Lowerer>>) {
        let (receiver, owner, expected) =
            match self.bound_inner_constructor(receiver, name, expected) {
                Ok(Some(found)) => found,
                Ok(None) => return (Vec::new(), None),
                Err(()) => return (Vec::new(), Some(Box::new(self.clone()))),
            };
        let mut probes = Vec::new();
        let mut failure = None;
        if let Some(crate::Owner::Class(class)) = self.nominal_owners.get(&owner).copied() {
            if self.classes[class].modifier == hir::ClassModifier::Abstract {
                self.error(
                    name.span,
                    format!("abstract class `{}` cannot be instantiated", name.text),
                );
                return (probes, Some(Box::new(self.clone())));
            }
            let Some(explicit) = self.resolve_call_type_args(call.type_args) else {
                return (probes, Some(Box::new(self.clone())));
            };
            let constructors = self.source_class_constructors(class).collect::<Vec<_>>();
            for constructor in constructors {
                let source = NominalConstructorSource::Class(constructor);
                if !self.constructor_is_accessible(source) {
                    continue;
                }
                let view = self.nominal_constructor_view(source, call.span);
                let expected =
                    self.named_nominal_expected_arguments(view.signature.return_type, expected);
                match self.probe_named_nominal(
                    view,
                    NominalConstructorCall {
                        enclosing: Some(&receiver),
                        explicit_type_args: &explicit,
                        expected_type_args: expected.as_deref(),
                        arguments: call.args,
                        span: call.span,
                    },
                ) {
                    Ok(probe) => probes.push(NamedFunctionLikeProbe::Nominal(Box::new(probe))),
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
        } else {
            let declaration = self
                .dependencies
                .as_ref()
                .expect("an imported inner has a catalog")
                .nominal_declaration(owner)
                .expect("a resolved inner has a declaration");
            if declaration.interface.declaration_details().modality()
                == hir::NominalInheritanceModalityV1::Abstract
            {
                self.error(
                    name.span,
                    format!("abstract class `{}` cannot be instantiated", name.text),
                );
                return (probes, Some(Box::new(self.clone())));
            }
            let candidates = match self
                .dependencies
                .as_ref()
                .expect("an imported inner has its declaration catalog")
                .constructor_candidates(owner)
            {
                Ok(candidates) => candidates,
                Err(error) => {
                    self.error(
                        name.span,
                        format!("invalid dependency constructor: {error}"),
                    );
                    return (probes, Some(Box::new(self.clone())));
                }
            };
            let call = ast::CallExpr {
                callee: name.clone(),
                type_args: call.type_args.to_vec(),
                args: call.args.to_vec(),
                span: call.span,
            };
            for candidate in candidates {
                match self.probe_imported_inner_constructor(
                    candidate,
                    &call,
                    expected,
                    receiver.clone(),
                ) {
                    Ok(probe) => {
                        probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)))
                    }
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
        }
        if probes.is_empty() && failure.is_none() {
            self.error(
                name.span,
                format!(
                    "constructor of inner class `{}` is not accessible here",
                    name.text
                ),
            );
            failure = Some(Box::new(self.clone()));
        }
        (probes, failure)
    }
}
