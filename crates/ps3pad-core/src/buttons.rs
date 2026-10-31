use serde::{Deserialize, Serialize};

/// Digital button state of a DualShock 3.
///
/// The bit assignment is internal to this crate; use [`Button`] and the
/// accessor methods instead of relying on raw integer values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Buttons(u32);

impl Buttons {
    pub const NONE: Self = Self(0);
    pub const SELECT: Self = Self(1 << 0);
    pub const L3: Self = Self(1 << 1);
    pub const R3: Self = Self(1 << 2);
    pub const START: Self = Self(1 << 3);
    pub const UP: Self = Self(1 << 4);
    pub const RIGHT: Self = Self(1 << 5);
    pub const DOWN: Self = Self(1 << 6);
    pub const LEFT: Self = Self(1 << 7);
    pub const PS: Self = Self(1 << 8);
    pub const L1: Self = Self(1 << 9);
    pub const R1: Self = Self(1 << 10);
    pub const TRIANGLE: Self = Self(1 << 11);
    pub const CIRCLE: Self = Self(1 << 12);
    pub const CROSS: Self = Self(1 << 13);
    pub const SQUARE: Self = Self(1 << 14);
    pub const L2: Self = Self(1 << 15);
    pub const R2: Self = Self(1 << 16);

    pub const ALL: Self = Self((1 << 17) - 1);

    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits & Self::ALL.0)
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[inline]
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Iterates over every pressed digital button.
    pub fn iter(self) -> impl Iterator<Item = Button> + 'static {
        Button::ALL
            .into_iter()
            .filter(move |button| self.contains(button.bit()))
    }
}

/// A digital button on the DualShock 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Button {
    Cross,
    Circle,
    Square,
    Triangle,
    Up,
    Down,
    Left,
    Right,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    Select,
    Start,
    Ps,
}

impl Button {
    /// Every button, in a stable order suitable for UI rendering.
    pub const ALL: [Button; 17] = [
        Button::Cross,
        Button::Circle,
        Button::Square,
        Button::Triangle,
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
        Button::L1,
        Button::R1,
        Button::L2,
        Button::R2,
        Button::L3,
        Button::R3,
        Button::Select,
        Button::Start,
        Button::Ps,
    ];

    #[inline]
    pub const fn bit(self) -> Buttons {
        match self {
            Button::Select => Buttons::SELECT,
            Button::L3 => Buttons::L3,
            Button::R3 => Buttons::R3,
            Button::Start => Buttons::START,
            Button::Up => Buttons::UP,
            Button::Right => Buttons::RIGHT,
            Button::Down => Buttons::DOWN,
            Button::Left => Buttons::LEFT,
            Button::Ps => Buttons::PS,
            Button::L1 => Buttons::L1,
            Button::R1 => Buttons::R1,
            Button::Triangle => Buttons::TRIANGLE,
            Button::Circle => Buttons::CIRCLE,
            Button::Cross => Buttons::CROSS,
            Button::Square => Buttons::SQUARE,
            Button::L2 => Buttons::L2,
            Button::R2 => Buttons::R2,
        }
    }

    /// PlayStation symbol used on the physical controller.
    pub const fn symbol(self) -> &'static str {
        match self {
            Button::Cross => "\u{2715}",    // ✕
            Button::Circle => "\u{25cb}",   // ○
            Button::Square => "\u{25a1}",   // □
            Button::Triangle => "\u{25b3}", // △
            Button::Up => "\u{2191}",
            Button::Down => "\u{2193}",
            Button::Left => "\u{2190}",
            Button::Right => "\u{2192}",
            Button::L1 => "L1",
            Button::R1 => "R1",
            Button::L2 => "L2",
            Button::R2 => "R2",
            Button::L3 => "L3",
            Button::R3 => "R3",
            Button::Select => "SEL",
            Button::Start => "STA",
            Button::Ps => "PS",
        }
    }

    /// Human-readable English name.
    pub const fn name(self) -> &'static str {
        match self {
            Button::Cross => "Cross",
            Button::Circle => "Circle",
            Button::Square => "Square",
            Button::Triangle => "Triangle",
            Button::Up => "D-pad up",
            Button::Down => "D-pad down",
            Button::Left => "D-pad left",
            Button::Right => "D-pad right",
            Button::L1 => "L1",
            Button::R1 => "R1",
            Button::L2 => "L2",
            Button::R2 => "R2",
            Button::L3 => "L3",
            Button::R3 => "R3",
            Button::Select => "Select",
            Button::Start => "Start",
            Button::Ps => "PS",
        }
    }
}
