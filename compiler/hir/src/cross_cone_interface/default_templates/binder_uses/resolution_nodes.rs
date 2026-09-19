use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedCanonicalBinderUseListV1, node, children, {
    let DecodedCanonicalBinderUseListV1 { arguments } = node;
    children.push(arguments)?;
    Ok(())
});
