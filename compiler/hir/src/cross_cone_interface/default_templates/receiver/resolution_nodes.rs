use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedTemplateReceiverV1, node, children, {
    let DecodedTemplateReceiverV1 {
        local_index,
        value_type,
    } = node;
    children.push(value_type)?;
    children.local_index(*local_index)?;
    children.push(local_index)?;
    Ok(())
});

resource_node!(DecodedOptionalTemplateReceiverV1, node, children, {
    match node {
        Self::Absent => return Ok(()),
        Self::Present(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});
