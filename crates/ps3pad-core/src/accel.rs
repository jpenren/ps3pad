use serde::{Deserialize, Serialize};

/// Accelerometer calibration.
///
/// The DualShock 3 uses a Kionix KXPC4-compatible accelerometer. Public
/// documentation and both Linux and SDL use a zero-g point of 511.5 counts and
/// roughly 113 counts per g. The controller does not expose a per-unit
/// calibration report, so these defaults are the best available.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccelCalibration {
    pub zero_g_counts: f32,
    pub counts_per_g: f32,
}

impl Default for AccelCalibration {
    fn default() -> Self {
        Self {
            zero_g_counts: 511.0,
            counts_per_g: 113.0,
        }
    }
}

impl AccelCalibration {
    /// Corrects the raw zero-g point, for example after measuring a resting
    /// controller.
    pub fn with_zero_g(mut self, counts: f32) -> Self {
        self.zero_g_counts = counts;
        self
    }
}

/// One accelerometer sample, in raw device counts, already axis-corrected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccelSample {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

impl AccelSample {
    /// Converts the sample to units of g.
    ///
    /// Axis signs follow the Linux `hid-sony` and SDL conventions so that a
    /// controller lying flat on its back reads approximately `(0, 0, +1)`.
    pub fn to_g(self, calibration: &AccelCalibration) -> [f32; 3] {
        let zero = calibration.zero_g_counts;
        let scale = calibration.counts_per_g.max(1.0);
        [
            (self.x as f32 - zero) / scale,
            (zero - self.y as f32) / scale,
            (zero - self.z as f32) / scale,
        ]
    }

    /// Pitch and roll in degrees, derived from the gravity vector.
    pub fn tilt(self, calibration: &AccelCalibration) -> Tilt {
        let [x, y, z] = self.to_g(calibration);
        let pitch = y.atan2((x * x + z * z).sqrt()).to_degrees();
        let roll = x.atan2((y * y + z * z).sqrt()).to_degrees();
        Tilt { pitch, roll }
    }
}

/// Pitch/roll pair in degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tilt {
    pub pitch: f32,
    pub roll: f32,
}
