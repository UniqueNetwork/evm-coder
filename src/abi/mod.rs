//! Implementation of EVM RLP reader/writer
#![allow(clippy::missing_errors_doc)]

mod traits;
use core::{fmt, mem, result};

pub use traits::*;
mod impls;

#[cfg(test)]
mod test;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Aligment for every simple type in bytes.
pub const ABI_ALIGNMENT: usize = 32;
pub const ABI_WORD_SIZE: u32 = 32;
pub type AbiWord = [u8; ABI_WORD_SIZE as usize];

/// Abi parsing result
pub type Result<T, E = Error> = result::Result<T, E>;

/// Generic decode failure
#[derive(Debug)]
pub enum Error {
	/// Input was shorter than expected
	OutOfOffset,
	/// Something is off about paddings
	InvalidRange,
	/// Custom parsing error
	Custom(&'static str),
}
impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Error::OutOfOffset => write!(f, "out of offset"),
			Error::InvalidRange => write!(f, "invalid range"),
			Error::Custom(m) => write!(f, "{m}"),
		}
	}
}
impl From<&'static str> for Error {
	fn from(value: &'static str) -> Self {
		Self::Custom(value)
	}
}

/// New abicoder
#[derive(Debug)]
pub struct AbiEncoder {
	out: Vec<u8>,
	offset: usize,
	dynamic_offset: usize,
}
impl AbiEncoder {
	fn new(out: Vec<u8>, offset: usize, dynamic_offset: usize) -> Self {
		assert_eq!((dynamic_offset - offset) % 32, 0);
		Self {
			out,
			offset,
			dynamic_offset,
		}
	}
	pub fn reserve_head(&mut self, words: u32) {
		assert_eq!(self.offset, self.dynamic_offset);
		assert_eq!(self.dynamic_offset, self.out.len());
		self.out
			.resize(self.out.len() + words as usize * ABI_WORD_SIZE as usize, 0);
		self.dynamic_offset += words as usize * ABI_WORD_SIZE as usize;
	}
	pub fn append_head(&mut self, word: AbiWord) {
		assert!(self.offset < self.dynamic_offset);
		self.out[self.offset..self.offset + 32].copy_from_slice(&word);
		self.offset += 32;
	}
	fn append_tail(&mut self, word: AbiWord) {
		self.out.extend_from_slice(&word);
	}
	fn tail_size(&self) -> u32 {
		self.out.len() as u32 - self.dynamic_offset as u32
	}
	fn encode_tail<T: AbiEncode>(&mut self, data: &T) {
		let offset = self.out.len();
		let mut out = mem::take(&mut self.out);
		let size = T::HEAD_WORDS as usize * ABI_WORD_SIZE as usize;
		out.resize(out.len() + size, 0);
		let mut encoder = AbiEncoder::new(
			out,
			offset,
			offset + T::HEAD_WORDS as usize * ABI_WORD_SIZE as usize,
		);
		data.enc(&mut encoder);
		self.out = encoder.into_inner()
	}
	fn into_inner(self) -> Vec<u8> {
		self.out
	}
}

#[derive(Clone)]
pub struct AbiDecoder<'d> {
	data: &'d [u8],
	offset: usize,
	global_frame_offset: usize,
}
impl<'d> AbiDecoder<'d> {
	fn new(data: &'d [u8], global_frame_offset: usize) -> Result<Self> {
		if data.len() % 32 != 0 {
			return Err(Error::OutOfOffset);
		}
		Ok(Self {
			data,
			offset: 0,
			global_frame_offset,
		})
	}
	pub fn get_head(&mut self) -> Result<AbiWord> {
		if self.offset >= self.data.len() {
			return Err(Error::OutOfOffset);
		}
		let mut word = [0; ABI_WORD_SIZE as usize];
		word.copy_from_slice(&self.data[self.offset..self.offset + 32]);
		self.offset += 32;
		Ok(word)
	}
	pub fn start_frame(&self) -> Self {
		self.dynamic_at(self.offset as u32).expect("not oob")
	}
	pub fn dynamic_at(&self, offset: u32) -> Result<Self> {
		if offset % 32 != 0 || self.data.len() < offset as usize {
			// Technically, allowed by spec, yet nothing has such offsets
			return Err(Error::OutOfOffset);
		}
		Self::new(
			&self.data[offset as usize..],
			self.global_frame_offset + offset as usize,
		)
	}
}
