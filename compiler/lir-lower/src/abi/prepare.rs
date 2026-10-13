//! Prepare storage once; target codegen owns all physical passing decisions.

use std::collections::HashMap;

use super::*;

mod fields;

pub(crate) struct SignatureBuilder<'a> {
    pub(crate) input: lir::AbiPlanningInput,
    context: &'a LoweringContext,
    structs: &'a lir::StructDefs,
    enums: &'a lir::EnumDefs,
    values: HashMap<lir::LirType, lir::AbiPlanningValueId>,
}

impl<'a> SignatureBuilder<'a> {
    pub(crate) fn new(
        context: &'a LoweringContext,
        structs: &'a lir::StructDefs,
        enums: &'a lir::EnumDefs,
    ) -> Self {
        Self {
            input: lir::AbiPlanningInput {
                target: context.target_profile(),
                values: Default::default(),
                scoop: Default::default(),
                c: Default::default(),
            },
            context,
            structs,
            enums,
            values: HashMap::new(),
        }
    }

    fn value(&mut self, ty: lir::LirType) -> StorageResult<lir::AbiPlanningValueId> {
        if let Some(&id) = self.values.get(&ty) {
            return Ok(id);
        }
        use lir::AbiPlanningValue as Input;
        let value = match classify_storage(self.context, ty.clone(), self.structs, self.enums)? {
            ValueStorage::ZeroSized(value) => Input::ZeroSized(value),
            ValueStorage::NonZero(value) => match lir::scoop_abi_value_shape(self.enums, &ty)? {
                lir::ScoopAbiValueShape::Scalar => Input::Scalar {
                    value,
                    carrier: scalar_carrier(&ty, self.enums)
                        .expect("scalar storage has its primitive carrier"),
                },
                lir::ScoopAbiValueShape::Interface => Input::Interface(value),
                lir::ScoopAbiValueShape::Aggregate if value.scan() != &lir::RefScan::None => {
                    Input::ScannedAggregate(value)
                }
                lir::ScoopAbiValueShape::Aggregate if matches!(ty, lir::LirType::Enum(_)) => {
                    Input::IntegerStorage(value)
                }
                lir::ScoopAbiValueShape::Aggregate => Input::Aggregate {
                    value,
                    fields: self.aggregate_fields(&ty)?,
                },
            },
        };
        let id = self.input.values.alloc(value);
        self.values.insert(ty, id);
        Ok(id)
    }

    pub(crate) fn scoop(
        &mut self,
        parameters: impl IntoIterator<Item = lir::LirType>,
        result: Option<lir::LirType>,
    ) -> StorageResult<lir::ScoopAbiRequestId> {
        let arguments = parameters
            .into_iter()
            .map(|ty| self.value(ty))
            .collect::<StorageResult<_>>()?;
        let result = match result {
            None => lir::LogicalAbiResult::Void,
            Some(ty) => lir::LogicalAbiResult::Value(self.value(ty)?),
        };
        Ok(self
            .input
            .scoop
            .alloc(lir::ScoopAbiRequest { arguments, result }))
    }

    pub(crate) fn mir_scoop<'ty>(
        &mut self,
        module: &mir::Module,
        parameters: impl IntoIterator<Item = &'ty mir::Type>,
        result: &mir::Type,
    ) -> StorageResult<lir::ScoopAbiRequestId> {
        let arguments: Vec<_> = parameters
            .into_iter()
            .map(|ty| lir_type(module, ty))
            .collect();
        let result = (result != &mir::Type::Unit).then(|| lir_type(module, result));
        self.scoop(arguments, result)
    }

    pub(crate) fn c(
        &mut self,
        signature: &lir::CFunctionType,
    ) -> StorageResult<lir::CAbiRequestId> {
        let mut value = |ty: &lir::CType| -> StorageResult<_> {
            Ok(lir::CAbiValueInput {
                value: self.value(ty.storage_type())?,
                ty: ty.clone(),
            })
        };
        let arguments = signature
            .params
            .iter()
            .map(&mut value)
            .collect::<StorageResult<_>>()?;
        let result = match &signature.return_type {
            lir::CReturnType::Void => lir::LogicalCAbiResult::Void,
            lir::CReturnType::Value(ty) => lir::LogicalCAbiResult::Value(value(ty)?),
        };
        Ok(self.input.c.alloc(lir::CAbiRequest { arguments, result }))
    }
}
