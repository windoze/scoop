//! Loaded source interfaces supply the common declaration parameter view.

use hir::ImportedCallableSource;
use scoop_hir as hir;

use super::ImportedCallableCandidate;
use super::generic::{ImportedGenericTarget, LoadedCallableSignature};
use crate::Lowerer;
use crate::call_resolution::candidates::{
    DeclarationSignature, ValueParameter, ValueParameterCalling, VarargOmission,
};
use crate::imported_core::{ImportedSignatureTypeError, ImportedTypeBindings};

pub(in crate::expr) type DependencySignature =
    DeclarationSignature<hir::ExportDefaultTemplateKeyV1>;

pub(super) struct GenericCallDeclaration {
    pub(super) template: ImportedGenericTarget,
    pub(super) signature: LoadedCallableSignature,
    pub(super) bindings: ImportedTypeBindings,
}

pub(super) enum ImportedCallDeclaration {
    Native(DependencySignature),
    Generic(Box<GenericCallDeclaration>),
}

impl ImportedCallDeclaration {
    pub(super) fn signature(&self) -> &DependencySignature {
        match self {
            Self::Native(signature) => signature,
            Self::Generic(declaration) => &declaration.signature.signature,
        }
    }
}

impl Lowerer {
    pub(super) fn imported_call_declaration(
        &mut self,
        candidate: &ImportedCallableCandidate,
        generic_constructor: bool,
    ) -> Result<ImportedCallDeclaration, String> {
        let interface = candidate.interface();
        let generic = generic_constructor
            || candidate.pointer_intrinsic().is_some()
            || candidate.array_intrinsic().is_some()
            || matches!(
                interface.effects().implementation(),
                hir::CallableImplementationV1::Intrinsic(
                    hir::IntrinsicFunctionKind::CoroutineStart
                        | hir::IntrinsicFunctionKind::CoroutineSuspend
                )
            )
            || matches!(interface.effects().implementation(), hir::CallableImplementationV1::Intrinsic(kind) if kind.is_runtime_gc_call())
            || (interface.modality() == hir::CallableModalityV1::Abstract
                && matches!(
                    interface.owner(),
                    hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::GenericTemplate(
                        _
                    ))
                ))
            || candidate.callable_body().is_some();
        if generic {
            let declaration = self
                .dependencies
                .as_ref()
                .expect("ordinary calls have a dependency catalog")
                .callable_declaration(interface.declaration())
                .expect("a candidate retains its declaration catalog entry");
            let template = ImportedGenericTarget::request(self, declaration)?;
            let (signature, bindings) = template.signature(self);
            Ok(ImportedCallDeclaration::Generic(Box::new(
                GenericCallDeclaration {
                    template,
                    signature,
                    bindings,
                },
            )))
        } else {
            self.imported_native_signature(candidate)
                .map(ImportedCallDeclaration::Native)
                .map_err(|error| error.diagnostic("dependency callable signature"))
        }
    }

    pub(crate) fn imported_parameter_views(
        &self,
        declaration: &impl hir::ImportedCallableSource,
        types: impl IntoIterator<Item = hir::TypeId>,
    ) -> Vec<ValueParameter<hir::ExportDefaultTemplateKeyV1>> {
        if matches!(
            declaration.interface().declaration(),
            scoop_identity::CallableTemplateOrigin::Accessor(_)
        ) {
            // Accessors have only implicit required parameters, never a named
            // call protocol or default/vararg declarations of their own.
            return declaration
                .interface()
                .parameters()
                .parameters()
                .iter()
                .zip(types)
                .map(|(parameter, ty)| ValueParameter {
                    name: parameter.name().as_str().to_owned(),
                    ty,
                    calling: ValueParameterCalling::Required,
                })
                .collect();
        }
        declaration
            .source_interface()
            .expect("a callable retains its source parameter protocol")
            .parameters()
            .parameters()
            .iter()
            .zip(types)
            .map(|(parameter, ty)| ValueParameter {
                name: parameter.name().as_str().to_owned(),
                ty,
                calling: match parameter.calling() {
                    hir::CallableParameterCallingV1::Required => ValueParameterCalling::Required,
                    hir::CallableParameterCallingV1::Default { template } => {
                        ValueParameterCalling::Default(*template)
                    }
                    hir::CallableParameterCallingV1::VarargEmpty { .. }
                    | hir::CallableParameterCallingV1::VarargDefault { .. } => {
                        ValueParameterCalling::Vararg {
                            element_type: self
                                .array_element_ty(ty)
                                .expect("a checked vararg signature has an array element type"),
                            array_type: ty,
                            omission: match parameter.calling() {
                                hir::CallableParameterCallingV1::VarargDefault {
                                    template, ..
                                } => VarargOmission::Default(*template),
                                _ => VarargOmission::EmptyArray,
                            },
                        }
                    }
                },
            })
            .collect()
    }

    pub(in crate::expr) fn imported_native_signature(
        &mut self,
        candidate: &ImportedCallableCandidate,
    ) -> Result<DependencySignature, ImportedSignatureTypeError> {
        let interface = candidate.interface();
        let types = interface
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| self.imported_signature_type(parameter.value_type()))
            .collect::<Result<Vec<_>, _>>()?;
        let value_parameters = self.imported_parameter_views(candidate, types);
        Ok(DeclarationSignature {
            owner_parameters: Vec::new(),
            callable_parameters: Vec::new(),
            value_parameters,
            return_type: self.imported_signature_type(interface.result())?,
        })
    }
}
