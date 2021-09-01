use std::ops::Deref;

use bytes::{Buf, BufMut, Bytes, BytesMut};

pub struct BitReader {
    bit_position: u8,
    bytes: Bytes,
    bits: u8,
}

impl BitReader {
    pub fn new(bytes: Bytes) -> Self {
        Self {
            bytes,
            bit_position: 8,
            bits: 0,
        }
    }

    pub fn get_bit(&mut self) -> bool {
        if self.bit_position == 8 {
            self.bits = self.bytes.get_u8();
            self.bit_position = 0;
        }
        let value = (self.bits & (1 << self.bit_position)) != 0;
        self.bit_position += 1;
        value
    }
}

impl Deref for BitReader {
    type Target = Bytes;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

pub struct BitWriter {
    byte_position: usize,
    bit_position: u8,
    bytes: BytesMut,
}

impl BitWriter {
    pub fn new(bytes: BytesMut) -> Self {
        Self {
            bytes,
            bit_position: 8,
            byte_position: 0,
        }
    }

    pub fn put_bit(&mut self, value: bool) {
        if self.bit_position == 8 {
            self.byte_position = self.bytes.len();
            self.bit_position = 0;
            self.bytes.put_u8(0);
        }
        self.bytes[self.byte_position] |= if value { 1 << self.bit_position } else { 0 };
        self.bit_position += 1;
    }
}

impl Deref for BitWriter {
    type Target = BytesMut;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}
