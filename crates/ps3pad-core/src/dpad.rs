use serde::{Deserialize, Serialize};

use crate::{Button, Buttons};

/// Four-direction hat state.
///
/// The DualShock 3 reports the d-pad as four independent bits, which makes
/// diagonals possible. A HID hat switch is used when exposing a virtual pad.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DPad {
    pub up: bool,
    pub right: bool,
    pub down: bool,
    pub left: bool,
}

impl DPad {
    #[inline]
    pub const fn is_centered(self) -> bool {
        !self.up && !self.right && !self.down && !self.left
    }

    /// Converts the four bits into a HID hat switch value.
    ///
    /// `0` is up, then values increase clockwise, and `8` is the HID "null"
    /// value used for the centred position.
    pub const fn hat(self) -> u8 {
        // Opposing directions cancel each other, matching the way the kernel
        // and SDL treat malformed reports.
        let up = self.up && !self.down;
        let down = self.down && !self.up;
        let left = self.left && !self.right;
        let right = self.right && !self.left;

        match (up, right, down, left) {
            (true, false, false, false) => 0,
            (true, true, false, false) => 1,
            (false, true, false, false) => 2,
            (false, true, true, false) => 3,
            (false, false, true, false) => 4,
            (false, false, true, true) => 5,
            (false, false, false, true) => 6,
            (true, false, false, true) => 7,
            _ => 8,
        }
    }

    /// Computes the d-pad state from the digital button set.
    pub fn from_buttons(buttons: Buttons) -> Self {
        Self {
            up: buttons.contains(Button::Up.bit()),
            right: buttons.contains(Button::Right.bit()),
            down: buttons.contains(Button::Down.bit()),
            left: buttons.contains(Button::Left.bit()),
        }
    }
}
