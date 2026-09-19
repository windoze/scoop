use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultBindingActionV1, node, children, {
    match node {
        Self::Project {
            source,
            result,
            projection,
            definition_origin,
        } => {
            children.push(definition_origin)?;
            children.push(projection)?;
            children.push(result)?;
            children.push(source)?;
        }
        Self::Component {
            source,
            index,
            result,
            setup,
            call,
            definition_origin,
        } => {
            children.push(definition_origin)?;
            children.push(call)?;
            children.push(setup)?;
            children.push(result)?;
            children.push(index)?;
            children.push(source)?;
        }
        Self::Bind {
            source,
            target,
            definition_origin,
        } => {
            children.push(definition_origin)?;
            children.push(target)?;
            children.push(source)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultBindingPlanV1, node, children, {
    let DecodedDefaultBindingPlanV1 {
        subject,
        shape,
        actions,
    } = node;
    children.push(actions)?;
    children.push(shape)?;
    children.push(subject)?;
    Ok(())
});

resource_node!(DecodedDefaultIteratorConformanceV1, node, children, {
    let DecodedDefaultIteratorConformanceV1 {
        source,
        iterator,
        interface_type,
        definition_origin,
    } = node;
    children.push(definition_origin)?;
    children.push(interface_type)?;
    children.push(iterator)?;
    children.push(source)?;
    Ok(())
});

resource_node!(DecodedDefaultAppliedOptionV1, node, children, {
    let DecodedDefaultAppliedOptionV1 { some_payload, none } = node;
    children.push(none)?;
    children.push(some_payload)?;
    Ok(())
});

resource_node!(DecodedDefaultIteratorNextV1, node, children, {
    let DecodedDefaultIteratorNextV1 {
        callable,
        result,
        option,
        element,
        definition_origin,
    } = node;
    children.push(definition_origin)?;
    children.push(element)?;
    children.push(option)?;
    children.push(result)?;
    children.push(callable)?;
    Ok(())
});

resource_node!(DecodedDefaultForIterationPlanV1, node, children, {
    let DecodedDefaultForIterationPlanV1 {
        source_setup,
        source,
        source_init,
        iterator_setup,
        iterator_call,
        conformance,
        next,
        binding,
        body,
    } = node;
    children.push(body)?;
    children.push(binding)?;
    children.push(next)?;
    children.push(conformance)?;
    children.push(iterator_call)?;
    children.push(iterator_setup)?;
    children.push(source_init)?;
    children.push(source)?;
    children.push(source_setup)?;
    Ok(())
});
