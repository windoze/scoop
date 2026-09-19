use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultBindingTemporaryV1, node, children, {
    let DecodedDefaultBindingTemporaryV1 {
        local_index,
        value_type,
    } = node;
    children.push(value_type)?;
    children.local_index(*local_index)?;
    children.push(local_index)?;
    Ok(())
});

resource_node!(DecodedDefaultBindingLeafV1, node, children, {
    let DecodedDefaultBindingLeafV1 {
        local_index,
        value_type,
        mutable,
    } = node;
    children.push(mutable)?;
    children.push(value_type)?;
    children.local_index(*local_index)?;
    children.push(local_index)?;
    Ok(())
});
