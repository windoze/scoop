use super::*;
use scoop_identity::{CallableTemplateOrigin, Effect};

pub(super) fn render(output: &hir::DependencyHirOutput, machine: &Table) -> Vec<String> {
    let export = output.output().export.module();
    let public = public_interface(output);
    let mapper = hir::HirSignatureTypeMapper::new(hir::HirTypeIdentityInputs::from_export(export));
    let mut rows = Vec::new();
    for (id, function) in export.functions.iter() {
        if !function.is_suspend {
            continue;
        }
        let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(source)) =
            &export.function_identities[id]
        else {
            panic!("the suspend fixture has a plain source declaration");
        };
        let owner = CallableTemplateOrigin::Function(source.id());
        let record = public.callable_interfaces().get(owner).unwrap();
        assert!(
            machine
                .records()
                .iter()
                .all(|record| { record.declaration() != Declaration::Function(source.id()) })
        );
        assert!(public.source_interfaces().get(owner).is_some());
        assert_eq!(record.effects().execution(), Effect::Suspend);
        assert_eq!(record.modality(), hir::CallableModalityV1::Abstract);
        assert_eq!(
            record.parameters().len_u32() as usize,
            function.params.len() - 1
        );
        for (parameter, source) in record
            .parameters()
            .parameters()
            .iter()
            .zip(&function.params[1..])
        {
            assert_eq!(parameter.value_type(), &mapper.map(source.ty, &[]).unwrap());
        }
        assert_eq!(
            record.result(),
            &mapper.map(function.return_ty, &[]).unwrap()
        );
        let effects = record.effects();
        rows.push(format!(
            "{}: {} params, {:?}, Public, {:?}, {:?}, {:?}, {:?}, {:?}, source-only\n",
            function.name,
            record.parameters().len_u32(),
            record.modality(),
            effects.execution(),
            effects.safety(),
            effects.gc_effect(),
            effects.operator_role(),
            effects.infix(),
        ));
    }
    rows
}
