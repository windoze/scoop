use super::*;

pub(super) fn project(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
    committed: &[mir::MirTypeBridgeDependencyV1],
    meter: &mut BudgetMeter,
) -> Result<Vec<lir::LayoutAbiDependencyV1>, Error> {
    let mut uses = Vec::new();
    let layouts = lookup::Layouts {
        local: expected.layouts(),
        dependencies: dependencies.layouts,
    };
    meter.charge_work(committed.len() as u64, &WirePath::root())?;
    for usage in committed {
        let provider = usage.provider();
        if provider == input.mir.module().cone {
            return Err(Error::LocalDependency(usage.target()));
        }
        let target = match usage.target() {
            mir::MirTypeBridgeTargetV1::Type(exact) => {
                let layout = layouts.value(exact, meter)?;
                let actual = layout.identity().physical_definition().provider();
                if actual != provider {
                    return Err(Error::TypeProvider {
                        exact,
                        expected: provider,
                        actual,
                    });
                }
                lir::LayoutAbiSemanticTargetV1::Layout(layout.identity().layout())
            }
            mir::MirTypeBridgeTargetV1::ShapeSupport(source) => {
                lir::LayoutAbiSemanticTargetV1::ShapeSupport(source)
            }
            // The other target kinds have separate callable, dispatch, object
            // and initialization contracts; they are not type/layout roots.
            mir::MirTypeBridgeTargetV1::Callable(_)
            | mir::MirTypeBridgeTargetV1::Dispatch(_)
            | mir::MirTypeBridgeTargetV1::Object(_)
            | mir::MirTypeBridgeTargetV1::InitializationUnit(_) => continue,
        };
        push(
            &mut uses,
            lir::LayoutAbiDependencyV1::new(provider, target),
            meter,
        )?;
    }
    let module = input.lir.module();
    meter.charge_work(
        module.meta.external_type_descriptors.len() as u64,
        &WirePath::root(),
    )?;
    for (id, descriptor) in module.meta.external_type_descriptors.iter() {
        if module.meta.well_known_type_descriptors.string == lir::TypeDescriptorRef::External(id) {
            continue;
        }
        push(
            &mut uses,
            lir::LayoutAbiDependencyV1::new(
                descriptor.provider(),
                lir::LayoutAbiSemanticTargetV1::Descriptor(descriptor.target()),
            ),
            meter,
        )?;
    }
    meter.charge_work(
        module.meta.external_callables.len() as u64,
        &WirePath::root(),
    )?;
    for (_, callable) in module.meta.external_callables.iter() {
        if callable.origin() == lir::ExternalCallableOrigin::LayoutV1 {
            push(
                &mut uses,
                lir::LayoutAbiDependencyV1::new(
                    callable.provider(),
                    lir::LayoutAbiSemanticTargetV1::Callable(callable.target()),
                ),
                meter,
            )?;
        }
    }
    sort_cost(uses.len(), meter)?;
    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}
