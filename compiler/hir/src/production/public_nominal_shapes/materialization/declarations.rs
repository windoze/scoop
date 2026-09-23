use super::*;
use crate::{
    CallableImplementationV1, DeclaredVisibilityV1, NominalSourceShapeV1, PublicDeclarationOwnerV1,
};
use scoop_identity::{CallableTemplateOrigin, Effect, SignatureTypeKey};

impl Graph<'_> {
    pub(super) fn declarations(
        &mut self,
        nominals: &CanonicalNominalInterfacesV1,
        callables: &CanonicalCallableInterfacesV1,
    ) -> Result<(), WireError> {
        for nominal in nominals.all_records() {
            let Some(owner) = self.position(nominal.declaration())? else {
                continue;
            };
            for field in nominal.source_shape().declared_fields() {
                self.require(owner, field.value_type(), 1)?;
            }
            if let NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
                for variant in shape.variants() {
                    self.meter.charge_work(1, &WirePath::root())?;
                    for field in variant.fields() {
                        self.require(owner, field.value_type(), 1)?;
                    }
                }
            }
            for parent in nominal.exact_supertypes().values() {
                self.require(owner, parent, 1)?;
            }
        }
        for callable in callables.all_declarations() {
            self.meter.charge_work(1, &WirePath::root())?;
            let PublicDeclarationOwnerV1::Nominal(nominal) = callable.owner() else {
                continue;
            };
            let Some(owner) = self.position(nominal)? else {
                continue;
            };
            let constructor = matches!(
                callable.declaration(),
                CallableTemplateOrigin::Constructor(_)
                    | CallableTemplateOrigin::VariantConstructor(_)
            ) && matches!(
                callable.declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            );
            if !constructor && callable.slot_relations().is_empty() {
                continue;
            }
            if !callable.type_parameters().is_empty()
                || callable.effects().execution() == Effect::Suspend
                || matches!(
                    callable.effects().implementation(),
                    CallableImplementationV1::SourceExternScoop
                        | CallableImplementationV1::SourceExternC
                )
            {
                self.block(owner)?;
            }
            for parameter in callable.parameters().parameters() {
                self.require(owner, parameter.value_type(), 1)?;
            }
            self.require(owner, callable.result(), 1)?;
        }
        Ok(())
    }

    fn require(
        &mut self,
        owner: usize,
        ty: &SignatureTypeKey,
        depth: u64,
    ) -> Result<(), WireError> {
        let path = WirePath::root();
        self.meter.check_semantic_depth(depth, &path)?;
        self.meter.charge_nodes(1, &path)?;
        self.meter.charge_work(1, &path)?;
        match ty {
            SignatureTypeKey::Nominal(source) => {
                if let Some(dependency) = self.position(SourceNominalId::Concrete(*source))? {
                    self.edge(dependency, owner)?;
                }
            }
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                self.block(owner)?;
            }
            SignatureTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    self.require(owner, element, depth + 1)?;
                }
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    self.require(owner, parameter, depth + 1)?;
                }
                self.require(owner, result, depth + 1)?;
            }
            SignatureTypeKey::RawPointer(pointee) => self.require(owner, pointee, depth + 1)?,
        }
        Ok(())
    }
}
