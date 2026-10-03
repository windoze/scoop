//! Dependency references taken from the complete MIR graph.

use scoop_lir as lir;
use scoop_mir as mir;

use super::LayoutProductionError as Error;

pub(in crate::request::preflight) fn collect_mir_references(
    input: &mir::ConeMirInput,
    dependencies: &[&lir::LayoutAbiExportConstituentsV1],
    roots: &mut Vec<lir::LayoutAbiDependencyV1>,
    physical: &mut Vec<(
        scoop_identity::ConeIdentity,
        lir::ExternalStrongShapeSubjectV1,
    )>,
) -> Result<(), Error> {
    for shape in input
        .materialization()
        .dependency_generated_nominal_shapes()
    {
        roots.push(lir::LayoutAbiDependencyV1::new(
            shape.provider(),
            lir::LayoutAbiSemanticTargetV1::ShapeSupport(shape.source()),
        ));
        physical.push((
            shape.provider(),
            lir::ExternalStrongShapeSubjectV1::TypeDescriptor(shape.exact()),
        ));
    }
    for source in input.module().meta.source_exact_types.iter() {
        if !matches!(
            source.ty(),
            mir::Type::String | mir::Type::Class(_) | mir::Type::Interface(_)
        ) {
            continue;
        }
        let mir::SourceExactTypeOwner::Cone(provider) = source.owner() else {
            continue;
        };
        if provider == input.module().cone {
            continue;
        }
        let exact = source.identity_record().id();
        roots.push(lir::LayoutAbiDependencyV1::new(
            provider,
            lir::LayoutAbiSemanticTargetV1::Descriptor(exact),
        ));
        physical.push((
            provider,
            lir::ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        ));
        if source.ty() == &mir::Type::String && !input.materialization().strings().is_empty() {
            physical.push((
                provider,
                lir::ExternalStrongShapeSubjectV1::TypeRegistration(exact),
            ));
        }
    }
    for ty in input
        .module()
        .globals
        .iter()
        .filter(|(_, global)| matches!(global.storage, mir::GlobalStorage::Managed { .. }))
        .map(|(_, global)| &global.ty)
        .chain(
            matches!(input.module().output, mir::MirOutput::Executable { .. })
                .then_some(&mir::Type::Any),
        )
    {
        let Some(source) = input.module().meta.source_exact_types.get(ty) else {
            continue;
        };
        let mir::SourceExactTypeOwner::Cone(provider) = source.owner() else {
            continue;
        };
        if provider == input.module().cone {
            continue;
        }
        let exact = source.identity_record().id();
        let role = match ty {
            mir::Type::Struct(id)
                if matches!(
                    &input.module().structs[*id].representation,
                    mir::StructRepresentation::Declared {
                        c_layout: Some(_),
                        ..
                    }
                ) =>
            {
                scoop_identity::RepresentationRole::CValue
            }
            mir::Type::FunPtr(_) => scoop_identity::RepresentationRole::NativeFunctionPointer,
            _ => scoop_identity::RepresentationRole::ManagedValue,
        };
        let layout = dependencies
            .iter()
            .find(|dependency| dependency.provider() == provider)
            .and_then(|dependency| dependency.layouts().find_exact_role(exact, role))
            .ok_or(Error::LayoutExports(
                scoop_lir_lower::LayoutAbiExportLoweringError::MissingLayout(exact),
            ))?;
        roots.push(lir::LayoutAbiDependencyV1::new(
            provider,
            lir::LayoutAbiSemanticTargetV1::Layout(layout.identity().layout()),
        ));
        physical.push((
            provider,
            lir::ExternalStrongShapeSubjectV1::Scan(layout.scan()),
        ));
    }
    physical.extend(input.module().globals.iter().filter_map(|(_, global)| {
        let mir::GlobalStorage::Imported { provider, storage } = global.storage else {
            return None;
        };
        Some((
            provider,
            lir::ExternalStrongShapeSubjectV1::StaticStorage(storage),
        ))
    }));
    Ok(())
}
