use super::*;
use crate::constructor_resolution::ResolvedNominalConstructor;

impl Lowerer {
    fn array_generate_expression(&mut self, args: Vec<hir::Expr>) -> ExprKind {
        let [count, initializer]: [hir::Expr; 2] = args
            .try_into()
            .expect("the solved array generator has two materialized arguments");
        ExprKind::ArrayGenerate {
            count: Box::new(count),
            initializer: Box::new(initializer),
        }
    }

    pub(crate) fn finish_named_nominal(
        &mut self,
        resolved: ResolvedNominalConstructor,
        span: Span,
    ) -> Option<hir::Expr> {
        if matches!(
            resolved.source,
            NominalConstructorSource::ArrayGenerate(_)
                | NominalConstructorSource::ImportedArrayGenerate(_)
        ) && let Err(error) = self.prepare_array_size_exception_type()
        {
            self.error(span, error.diagnostic("array size exception type"));
            return None;
        }
        let (kind, ty) = match resolved.source {
            NominalConstructorSource::Struct(source) => {
                let owner = self.struct_constructors[source].owner;
                let application = self.struct_application_id(owner, resolved.type_args);
                let constructor = self.struct_constructor_application(source, application);
                (
                    ExprKind::StructInit {
                        constructor,
                        args: resolved.args,
                    },
                    self.struct_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::Class(source) => {
                let owner = self.class_constructors[source].owner;
                let application = self.class_application_id(owner, resolved.type_args);
                let constructor = self.class_constructor_application(source, application);
                (
                    ExprKind::ClassInit {
                        constructor,
                        args: resolved.args,
                    },
                    self.class_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::Variant(target) => {
                let application =
                    self.enum_application_id(target.enumeration(), resolved.type_args);
                let variant = hir::AppliedEnumVariantRef::checked(
                    &self.enums,
                    &self.enum_applications,
                    self.nominal_identities
                        .as_ref()
                        .expect("nominal identities precede application references"),
                    application,
                    target,
                )
                .expect("the chosen nominal application owns its variant");
                (
                    ExprKind::VariantConstruct {
                        variant: self.enum_variant_reference(variant),
                        args: resolved.args,
                    },
                    self.enum_applications[application].canonical_type,
                )
            }
            NominalConstructorSource::ArrayGenerate(class) => {
                let ty = self.class_application(class, resolved.type_args);
                (self.array_generate_expression(resolved.args), ty)
            }
            NominalConstructorSource::ImportedArrayGenerate(owner) => {
                let ty = self
                    .imported_nominal_application(owner, resolved.type_args)
                    .expect("a solved array generator retains its complete application");
                (self.array_generate_expression(resolved.args), ty)
            }
            NominalConstructorSource::IntrinsicClass(class) => {
                let ty = self.class_application(class, resolved.type_args);
                let [argument]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("array conversion has one materialized argument");
                (ExprKind::ArrayClone(Box::new(argument)), ty)
            }
            NominalConstructorSource::Atomic(class) => {
                let ty = self.class_application(class, resolved.type_args);
                let [initial]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("an atomic constructor has one initial value");
                (ExprKind::AtomicNew(Box::new(initial)), ty)
            }
            NominalConstructorSource::ImportedAtomic(owner) => {
                let ty = self
                    .imported_nominal_application(owner, resolved.type_args)
                    .expect("a solved atomic constructor retains its complete application");
                let [initial]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("an atomic constructor has one initial value");
                (ExprKind::AtomicNew(Box::new(initial)), ty)
            }
            NominalConstructorSource::ImportedArray(owner) => {
                let ty = self
                    .imported_nominal_application(owner, resolved.type_args)
                    .expect("an applicable array conversion has a complete result type");
                let [argument]: [hir::Expr; 1] = resolved
                    .args
                    .try_into()
                    .expect("array conversion has one materialized argument");
                (ExprKind::ArrayClone(Box::new(argument)), ty)
            }
        };
        Some(hir::Expr {
            kind,
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}
