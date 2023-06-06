use nih_plug::{buffer, debug::nih_debug_assert};
use std::collections::vec_deque::VecDeque;

#[derive(Debug)]

pub struct PeakHold {
    sample_rate: f32,
    hold_time: f32,
    deques: Vec<VecDeque<f32>>,
    audio_buffers: Vec<VecDeque<f32>>,
}

impl Default for PeakHold {
    fn default() -> Self {
        Self {
            sample_rate: 0.0,
            hold_time: 0.0,
            deques: Vec::new(),
            audio_buffers: Vec::new(),
        }
    }
}

impl PeakHold {
    pub fn initialize(&mut self, num_channels: usize, sample_rate: f32, hold_time: f32) {
        nih_debug_assert!(num_channels >= 1);
        nih_debug_assert!(sample_rate > 0.0);

        // allocate `hold_time as f32` seconds of audio queues with size `buffer_len as usize`
        let deque_len = (sample_rate * hold_time) as usize;
        let buffer_len = (sample_rate * hold_time) as usize + 1;

        // resize queues
        self.deques.resize_with(num_channels, VecDeque::new());
        for deque in self.deques.iter_mut() {
            deque.resize(deque_len, 0.0);
        }

        self.audio_buffers
            .resize_with(num_channels, VecDeque::new());
        for buffer in self.audio_buffers.iter_mut() {
            buffer.resize(buffer_len, 0.0);
        }

        self.sample_rate = sample_rate;
    }

    pub fn process(&mut self, channel_idx: usize, input_sample: f32) {
        if self.deques[channel_idx].len() > 0 {
            let index = self.deques[channel_idx].len() - 1;
            while index >= 0 {
                if self.deques[channel_idx][index] < input_sample {
                    self.deques[channel_idx].pop_back();
                } else {
                    break;
                }
                index -= 1;
            }
        }
        self.deques[channel_idx].append(input_sample);
        self.audio_buffers[channel_idx].append(input_sample);

        let delay_output = self.audio_buffers[channel_idx].pop_front().unwrap();

        if self.deques[channel_idx].len() > 0 && delay_output == self.deques[channel_idx][0] {
            self.deques[channel_idx].pop_front();
        }

        let result = 0.0;
        if self.deques[channel_idx].len() > 0 {
            result = self.deques[channel_idx][0];
        }
        result
    }
}

// struct OnePole {
//     s1: f32,
//     pub coefficients: Coefficients,
// }
//
// struct OnePoleCoefficients {
//     a0: f32,
//     b1: f32,
// }
//
// impl Default for OnePole {
//     fn default() -> Self {
//         Self {
//             s1: 0.0,
//             coefficients: Coefficients::identity(),
//         }
//     }
// }
//
// impl OnePoleLP {
//     pub fn process(&mut self, sample: f32) -> f32 {
//         let result = self.a0 * sample - self.b1 * s1;
//         let s1 = result;
//         result
//     }
//     pub fn reset(&mut self) {
//         self.s1 = 0.0;
//     }
// }
//
// impl OnePoleCoefficients {
//     pub fn from_f32s(scalar: OnePoleCoefficients<f32>) -> Self {
//         Self {
//             a0: scalar.a0,
//             b1: scalar.b1,
//         }
//     }
//     pub fn identity() -> Self {
//         Self { a0: 1.0, b1: 0.0 }
//     }
// }
