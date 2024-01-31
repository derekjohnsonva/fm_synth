const TABLE_SIZE: usize = 1024;

fn linear_interpolation(value1: f32, value2: f32, fraction: f32) -> f32 {
    value1.mul_add(1.0 - fraction, value2 * fraction)
}

/// Represents a sine wave oscillator.
#[derive(Debug)]
pub struct SinOsc {
    table: [f32; TABLE_SIZE], // Lookup table for storing precomputed sine values
}

#[allow(clippy::cast_precision_loss)]
impl SinOsc {
    /// Creates a new `SinOsc` instance.
    ///
    /// # Returns
    ///
    /// A `SinOsc` instance with an initialized lookup table and phase set to 0.0.
    pub fn new() -> Self {
        let mut table = [0.0; TABLE_SIZE];

        table.iter_mut().enumerate().for_each(|(i, phase)| {
            *phase = (i as f32 / TABLE_SIZE as f32 * 2.0 * std::f32::consts::PI).sin();
        });

        Self { table }
    }

    /// Reads the oscillator and returns the current sample.
    ///
    /// # Arguments
    ///
    /// * `normalized_phase_inc` - The normalized phase increment. Will be in the range [0.0, 1.0]
    ///
    /// # Returns
    ///
    /// The current sample value of the oscillator.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn read_osc(&mut self, normalized_phase_inc: f32) -> f32 {
        let table_index = normalized_phase_inc * TABLE_SIZE as f32;
        let table_index_wrap = table_index % TABLE_SIZE as f32; // for some reason SynthLab does not do this

        let table_index_low = table_index_wrap.floor() as usize;
        let table_index_high = table_index_wrap.ceil() as usize % TABLE_SIZE;
        let frac = table_index_wrap.fract();

        // Linear interpolation
        linear_interpolation(
            self.table[table_index_low],
            self.table[table_index_high],
            frac,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::{assert_relative_eq, assert_relative_ne};
    use rstest::*;

    #[fixture]
    fn setup() {
        #[allow(clippy::unwrap_used)]
        color_eyre::install().unwrap();
    }
    #[rstest]
    fn test_sin_vals() {
        // Make sure the last value in the table is not 0.0 so we don't get pops
        let osc = SinOsc::new();
        assert_relative_ne!(osc.table[TABLE_SIZE - 1], 0.0);
    }

    #[rstest]
    fn test_linear_interpolation() {
        // Test case 1: value1 = 0.0, value2 = 1.0, fraction = 0.5
        let result1 = linear_interpolation(0.0, 1.0, 0.5);
        assert_relative_eq!(result1, 0.5);

        // Test case 2: value1 = -1.0, value2 = 1.0, fraction = 0.25
        let result2 = linear_interpolation(-1.0, 1.0, 0.25);
        assert_relative_eq!(result2, -0.5);

        // Test case 3: value1 = 10.0, value2 = 20.0, fraction = 0.75
        let result3 = linear_interpolation(10.0, 20.0, 0.75);
        assert_relative_eq!(result3, 17.5);
    }

    #[rstest]
    fn read_osc_test() {
        let mut osc = SinOsc::new();

        // Test case 1: normalized_phase_inc = 0.0
        let result1 = osc.read_osc(0.0);
        assert_relative_eq!(result1, 0.0);

        // Test case 2: normalized_phase_inc = 0.25
        let result2 = osc.read_osc(0.25);
        assert_relative_eq!(result2, 1.0);

        // Test case 3: normalized_phase_inc = 0.5
        let result3 = osc.read_osc(0.5);
        assert_relative_eq!(result3, 0.0);

        // Test case 4: normalized_phase_inc = 0.75
        let result4 = osc.read_osc(0.75);
        assert_relative_eq!(result4, -1.0);

        // Test case 5: normalized_phase_inc = 1.0
        let result5 = osc.read_osc(1.0);
        assert_relative_eq!(result5, 0.0);
    }
}
