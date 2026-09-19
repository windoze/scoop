use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultStringOwnerV1, node, children, {
    match node {
        Self::CurrentInstantiation => return Ok(()),
        Self::Property(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});
