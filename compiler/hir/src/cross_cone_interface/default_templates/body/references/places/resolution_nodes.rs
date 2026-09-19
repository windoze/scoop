use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultPlaceV1, node, children, {
    match node {
        Self::Local { local_index } => {
            children.local_index(*local_index)?;
            children.push(local_index)?;
        }
        Self::Global { property } => {
            children.push(property)?;
        }
    }
    Ok(())
});
