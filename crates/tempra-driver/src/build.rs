//! Build-mode dispatch logic.
//!
//! Selects the appropriate backend and optimisation pipeline based on the
//! requested [`BuildTarget`] and [`BuildMode`].

use tempra_package::{BuildMode, BuildTarget};

/// Placeholder for future build dispatch.
pub fn describe_build(target: BuildTarget, mode: BuildMode) -> String {
    format!("target={target:?}, mode={mode:?}")
}
