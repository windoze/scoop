use crate::Lowerer;
use scoop_hir as hir;

impl Lowerer {
    pub(super) fn imported_bound_call_kind(
        &mut self,
        declaration: Option<&hir::CallableDeclarationRecordV1>,
        declared: hir::CallableTarget,
        args: &mut Vec<hir::Expr>,
        parameter_types: &[hir::TypeId],
        result_type: hir::TypeId,
    ) -> Result<Option<hir::ExprKind>, String> {
        let Some(declaration) = declaration else {
            return Ok(None);
        };
        let Some(callee) = self.imported_bound_member_callee(
            declaration,
            declared,
            args[0].ty,
            parameter_types,
            result_type,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(hir::ExprKind::ImportedMethodCall {
            receiver: Box::new(args.remove(0)),
            callee,
            args: std::mem::take(args),
        }))
    }

    pub(in crate::expr) fn imported_bound_member_callee(
        &mut self,
        declaration: &hir::CallableDeclarationRecordV1,
        declared: hir::CallableTarget,
        receiver_type: hir::TypeId,
        parameter_types: &[hir::TypeId],
        result_type: hir::TypeId,
    ) -> Result<Option<hir::ImportedMethodCallee>, String> {
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = declaration.owner() else {
            return Ok(None);
        };
        let interface = self
            .imported_member_owner_type(receiver_type, owner)
            .ok_or("a selected bound member retains its declaring owner")?;
        let hir::Type::ImportedInterface(source) = &self.types[interface] else {
            return Ok(None);
        };
        let member = declaration.declaration();
        let slot = source
            .methods
            .iter()
            .find(|method| method.declaration.declaration() == member)
            .map(|method| method.slot.id())
            .ok_or("a selected bound member retains its declared slot")?;
        self.require_imported_bound_interface(interface)
            .map_err(|error| error.diagnostic("bound conformance"))?;
        let signature = self.intern_function_type(
            declaration.effects().execution() == scoop_identity::Effect::Suspend,
            parameter_types.to_vec(),
            result_type,
        );
        let hir::Type::Function(signature) = self.types[signature] else {
            unreachable!("an interned bound signature is a function type")
        };
        Ok(Some(hir::ImportedMethodCallee::InterfaceBound(Box::new(
            hir::ImportedInterfaceBoundCallable {
                receiver_type,
                interface,
                member,
                slot,
                declared,
                signature,
            },
        ))))
    }
}
