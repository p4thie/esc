// Buffr Glitch: a MIDI-controlled buffer repeater
// Copyright (C) 2022-2023 Robbert van der Helm
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use nih_plug::debug::nih_debug_assert;

/// A super simple ring buffer abstraction that records audio into a buffer until it is full, and
/// then starts looping the already recorded audio. The recording starts when pressing a key so
/// transients are preserved correctly. This needs to be able to store at least the number of
/// samples that correspond to the period size of MIDI note 0.
#[derive(Debug)]

pub struct RingBuffer {
    sample_rate: f32,

    // internal buffers
    audio_buffers: Vec<Vec<f32>>,
    /// The current playback /read position in `playback_buffers`.
    read_sample_pos: Vec<usize>,

}

impl Default for RingBuffer {
    fn default() -> Self {
        Self {
            sample_rate: 0.0,
            audio_buffers: Vec::new(),
            read_sample_pos: Vec::new(),
        }
    }
}

impl RingBuffer {

    pub fn initialize(&mut self, num_channels: usize, sample_rate: f32) {

        nih_debug_assert!(num_channels >= 1);
        nih_debug_assert!(sample_rate > 0.0);

        // allocate 0.1 seconds of audio buffers 
        let buffer_len = (sample_rate * 0.1) as usize;

        // resize buffers
        self.audio_buffers.resize_with(num_channels, Vec::new);
        for buffer in self.audio_buffers.iter_mut() {
            buffer.resize(buffer_len, 0.0);
        }
        self.sample_rate = sample_rate;
        self.read_sample_pos.resize(num_channels, 0);
    }

    /// Zero out the buffers.
    pub fn reset(&mut self) {
        // The current verion's buffers don't need to be reset since they're always initialized
        // before being used
    }

    /// Read or write a sample from or to the ring buffer, and return the output. On the first loop
    /// this will store the input samples into the bufffer and return the input value as is.
    /// Afterwards it will read the previously recorded data from the buffer. The read/write
    /// position is advanced whenever the last channel is written to.
    pub fn process (&mut self, channel_idx: usize, input_sample: f32, delay_time: f32) -> f32 {
        let delay_time_sec = delay_time / 1000.0;
        let delay_time_sample = (delay_time_sec * self.sample_rate) as usize;

        let write_sample_pos = (self.read_sample_pos[channel_idx] + delay_time_sample) % self.audio_buffers[0].len();

        self.audio_buffers[channel_idx][write_sample_pos] = input_sample;

        let curr_read_sample_pos = self.read_sample_pos[channel_idx];
        let result = self.audio_buffers[channel_idx][curr_read_sample_pos];

        // TODO: This can be done more efficiently, but you really won't notice the performance
        //       impact here
        self.read_sample_pos[channel_idx] += 1;
            if self.read_sample_pos[channel_idx] == self.audio_buffers[0].len() {
                self.read_sample_pos[channel_idx] = 0;
        }
        result
    }
}