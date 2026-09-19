use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedExportDefaultBodyV1, node, children, {
    let DecodedExportDefaultBodyV1 { statements, value } = node;
    children.push(value)?;
    children.push(statements)?;
    Ok(())
});
