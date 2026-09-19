use super::*;
use scoop_identity::{EnumVariantFieldSelector, FieldIdentityView};
use scoop_wire::{WireErrorKind, encoded_length};

pub(super) trait KeyBudget {
    fn charge(&self, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError>;
}
impl KeyBudget for FieldIdentityKey {
    fn charge(&self, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
        match self.view() {
            FieldIdentityView::SourceDeclared { name, .. } => leaf(name.as_str(), meter, path)?,
            FieldIdentityView::SourcePropertyBacking { .. }
            | FieldIdentityView::SourcePropertyDelegate { .. }
            | FieldIdentityView::Generated { .. } => {}
        }
        // Every role is a fixed product of typed owner/property/local ids and
        // at most the single source field name inspected above.
        fixed_key(self, meter, path)
    }
}
impl KeyBudget for EnumVariantFieldKey {
    fn charge(&self, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
        match self.selector() {
            EnumVariantFieldSelector::Named(name) => leaf(name.as_str(), meter, path)?,
            EnumVariantFieldSelector::Positional { .. } => {}
        }
        fixed_key(self, meter, path)
    }
}
fn fixed_key(
    key: &impl WireEncode,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_semantic_depth(3, path)?;
    meter.charge_nodes(3, path)?;
    meter.charge_edges(3, path)?;
    let bytes = encoded_length(key).map_err(|_| overflow(path))?;
    meter.charge_work(bytes, path)
}
fn leaf(value: &str, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    meter.check_semantic_leaf(value.len() as u64, path)?;
    meter.charge_work(value.len() as u64, path)
}
pub(super) fn overflow(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
