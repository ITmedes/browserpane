use alloc::vec::Vec;

use super::error::FrameError;

pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], FrameError> {
        if self.remaining() < len {
            return Err(FrameError::BufferTooShort {
                expected: self.pos + len,
                available: self.buf.len(),
            });
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.buf[start..self.pos])
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, FrameError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn read_u16(&mut self) -> Result<u16, FrameError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn read_i16(&mut self) -> Result<i16, FrameError> {
        let bytes = self.take(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, FrameError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn read_u64(&mut self) -> Result<u64, FrameError> {
        let bytes = self.take(8)?;
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(array))
    }

    pub(crate) fn read_bool(&mut self) -> Result<bool, FrameError> {
        Ok(self.read_u8()? != 0)
    }

    pub(crate) fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], FrameError> {
        self.take(len)
    }

    pub(crate) fn read_fixed_array<const N: usize>(&mut self) -> Result<[u8; N], FrameError> {
        let mut array = [0u8; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }

    pub(crate) fn read_vec_u32(&mut self) -> Result<Vec<u8>, FrameError> {
        let len = self.read_u32()? as usize;
        Ok(self.read_bytes(len)?.to_vec())
    }

    pub(crate) fn finish<T>(self, value: T) -> Result<T, FrameError> {
        if self.remaining() > 0 {
            return Err(FrameError::TrailingData(self.remaining()));
        }
        Ok(value)
    }
}

pub(crate) struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub(crate) fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub(crate) fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
    }

    pub(crate) fn write_u8(&mut self, val: u8) {
        self.buf.push(val);
    }

    pub(crate) fn write_u16(&mut self, val: u16) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    pub(crate) fn write_i16(&mut self, val: i16) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    pub(crate) fn write_u32(&mut self, val: u32) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    pub(crate) fn write_u64(&mut self, val: u64) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    pub(crate) fn write_bool(&mut self, val: bool) {
        self.write_u8(u8::from(val));
    }

    pub(crate) fn write_bytes(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub(crate) fn write_vec_u32(&mut self, data: &[u8]) {
        self.write_u32(data.len() as u32);
        self.write_bytes(data);
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.buf
    }
}

pub(crate) fn decode_tagged<T>(
    buf: &[u8],
    decode: impl FnOnce(u8, &mut Reader<'_>) -> Result<T, FrameError>,
) -> Result<T, FrameError> {
    let mut reader = Reader::new(buf);
    let tag = reader.read_u8()?;
    let value = decode(tag, &mut reader)?;
    reader.finish(value)
}
