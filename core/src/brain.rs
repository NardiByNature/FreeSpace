//! A fixed-topology feed-forward net: N_IN inputs, one tanh hidden layer,
//! N_OUT tanh outputs. No allocation; weights live in the genome.

use crate::fastmath::tanh;
use crate::genome::{N_HID, N_IN, N_OUT, N_W};

/// Input slots, kept in one place so the inspector can label them.
pub const IN_BIAS: usize = 0;
pub const IN_PLANT_X: usize = 1;
pub const IN_PLANT_Y: usize = 2;
pub const IN_PLANT_PROX: usize = 3;
pub const IN_CREATURE_X: usize = 4;
pub const IN_CREATURE_Y: usize = 5;
pub const IN_CREATURE_PROX: usize = 6;
pub const IN_CREATURE_SIZE: usize = 7;
pub const IN_CREATURE_HUE: usize = 8;
pub const IN_ENERGY: usize = 9;
pub const IN_AGE: usize = 10;
pub const IN_OSC: usize = 11;

pub const OUT_TURN: usize = 0;
pub const OUT_THRUST: usize = 1;
pub const OUT_BITE: usize = 2;

pub const INPUT_NAMES: [&str; N_IN] = [
    "bias",
    "plant dx",
    "plant dy",
    "plant near",
    "other dx",
    "other dy",
    "other near",
    "other size",
    "other hue",
    "energy",
    "age",
    "clock",
];
pub const OUTPUT_NAMES: [&str; N_OUT] = ["turn", "thrust", "bite"];

#[inline]
pub fn forward(weights: &[f32; N_W], inputs: &[f32; N_IN], hidden: &mut [f32; N_HID], outputs: &mut [f32; N_OUT]) {
    for h in 0..N_HID {
        let base = h * N_IN;
        let mut acc = 0.0f32;
        for i in 0..N_IN {
            acc += weights[base + i] * inputs[i];
        }
        hidden[h] = tanh(acc);
    }
    let out_base = N_IN * N_HID;
    for o in 0..N_OUT {
        let base = out_base + o * (N_HID + 1);
        let mut acc = weights[base + N_HID]; // bias
        for h in 0..N_HID {
            acc += weights[base + h] * hidden[h];
        }
        outputs[o] = tanh(acc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_weights_give_zero_outputs() {
        let w = [0.0f32; N_W];
        let inp = [1.0f32; N_IN];
        let (mut h, mut o) = ([0.0; N_HID], [0.0; N_OUT]);
        forward(&w, &inp, &mut h, &mut o);
        assert!(o.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn output_bias_is_the_last_weight_of_each_output_row() {
        let mut w = [0.0f32; N_W];
        let row = N_IN * N_HID + OUT_THRUST * (N_HID + 1);
        w[row + N_HID] = 10.0;
        let inp = [0.0f32; N_IN];
        let (mut h, mut o) = ([0.0; N_HID], [0.0; N_OUT]);
        forward(&w, &inp, &mut h, &mut o);
        assert_eq!(o[OUT_THRUST], 1.0);
        assert_eq!(o[OUT_TURN], 0.0);
    }
}
