use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedTemplateValueParameterV1, node, children, {
    let DecodedTemplateValueParameterV1 {
        position,
        local_index,
    } = node;
    children.local_index(*local_index)?;
    children.push(local_index)?;
    children.push(position)?;
    Ok(())
});

resource_node!(DecodedCanonicalTemplateValueParametersV1, node, children, {
    let DecodedCanonicalTemplateValueParametersV1 { parameters } = node;
    children.push(parameters)?;
    Ok(())
});
