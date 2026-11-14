//! Layer handling for RayEtch.
//! Mirrors LightBurn's CUT_PARAMS per‑layer defaults (fnd 1).

use serde::{Deserialize, Serialize};
use super::cut_params::CutSetting;
use super::PALETTE;

/// A drawing layer.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Layer {
    /// Index in the 32‑slot palette.
    pub index: usize,
    /// Human readable name (e.g. "C01", "T1").
    pub name: String,
    /// Whether the layer is visible.
    pub visible: bool,
    /// Whether the layer is locked for editing.
    pub locked: bool,
    /// Cut parameters for this layer.
    pub setting: CutSetting,
}

impl Layer {
    pub fn new(index: usize) -> Self {
        let name = super::layer_name(index);
        Layer {
            index,
            name,
            visible: true,
            locked: false,
            setting: CutSetting::default(),
        }
    }
}

/// Return a vector with the default 32 layers (30 cut + 2 tool).
pub fn default_layers() -> Vec<Layer> {
    (0..super::LAYER_COUNT).map(Layer::new).collect()
}
