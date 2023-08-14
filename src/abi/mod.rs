//! Implementation of EVM RLP reader/writer
#![allow(clippy::missing_errors_doc)]

mod traits;
use core::{fmt, result};

pub use traits::*;
mod impls;

#[cfg(test)]
mod test;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use primitive_types::{H160, U256};

use crate::types::{Bytes4, String};

/// Aligment for every simple type in bytes.
pub const ABI_ALIGNMENT: usize = 32;

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

/// View into RLP data, which provides method to read typed items from it
#[derive(Clone)]
#[allow(clippy::module_name_repetitions)]
pub struct AbiReader<'i> {
	buf: &'i [u8],
	subresult_offset: usize,
	offset: usize,
}
impl<'i> AbiReader<'i> {
	/// Start reading RLP buffer, assuming there is no padding bytes
	#[must_use]
	pub fn new(buf: &'i [u8]) -> Self {
		Self {
			buf,
			subresult_offset: 0,
			offset: 0,
		}
	}

	/// Start reading RLP buffer, parsing first 4 bytes as selector
	///
	/// # Errors
	///
	/// Fails if provided buffer has not enough length for selector+data
	pub fn new_call(buf: &'i [u8]) -> Result<(Bytes4, Self)> {
		if buf.len() < 4 {
			return Err(Error::OutOfOffset);
		}
		let mut method_id = [0; 4];
		method_id.copy_from_slice(&buf[0..4]);

		Ok((
			method_id,
			Self {
				buf,
				subresult_offset: 4,
				offset: 4,
			},
		))
	}

	fn read_pad<const S: usize>(
		buf: &[u8],
		offset: usize,
		pad_start: usize,
		pad_end: usize,
		block_start: usize,
		block_end: usize,
	) -> Result<[u8; S]> {
		if buf.len() - offset < ABI_ALIGNMENT {
			return Err(Error::OutOfOffset);
		}
		let mut block = [0; S];
		let is_pad_zeroed = buf[pad_start..pad_end].iter().all(|&v| v == 0);
		if !is_pad_zeroed {
			return Err(Error::InvalidRange);
		}
		block.copy_from_slice(&buf[block_start..block_end]);
		Ok(block)
	}

	fn read_padleft<const S: usize>(&mut self) -> Result<[u8; S]> {
		let offset = self.offset;
		self.offset += ABI_ALIGNMENT;
		Self::read_pad(
			self.buf,
			offset,
			offset,
			offset + ABI_ALIGNMENT - S,
			offset + ABI_ALIGNMENT - S,
			offset + ABI_ALIGNMENT,
		)
	}

	fn read_padright<const S: usize>(&mut self) -> Result<[u8; S]> {
		let offset = self.offset;
		self.offset += ABI_ALIGNMENT;
		Self::read_pad(
			self.buf,
			offset,
			offset + S,
			offset + ABI_ALIGNMENT,
			offset,
			offset + S,
		)
	}

	/// Read [`H160`] at current position, then advance
	pub fn address(&mut self) -> Result<H160> {
		Ok(H160(self.read_padleft()?))
	}

	/// Read [`bool`] at current position, then advance
	pub fn bool(&mut self) -> Result<bool> {
		let data: [u8; 1] = self.read_padleft()?;
		match data[0] {
			0 => Ok(false),
			1 => Ok(true),
			_ => Err(Error::InvalidRange),
		}
	}

	/// Read [`[u8; 4]`] at current position, then advance
	pub fn bytes4(&mut self) -> Result<[u8; 4]> {
		self.read_padright()
	}

	/// Read [`[u8; S]`] padded left at current position, then advance
	///
	/// # Panics
	///
	/// Panics if attempting to read more bytes then `ABI_ALIGNMENT`
	pub fn bytes_padleft<const S: usize>(&mut self) -> Result<[u8; S]> {
		assert!(S <= ABI_ALIGNMENT);
		let offset = self.offset;
		self.offset += ABI_ALIGNMENT;
		Self::read_pad(
			self.buf,
			offset,
			offset,
			offset + ABI_ALIGNMENT - S,
			offset + ABI_ALIGNMENT - S,
			offset + ABI_ALIGNMENT,
		)
	}

	/// Read [`Vec<u8>`] at current position, then advance
	pub fn bytes(&mut self) -> Result<Vec<u8>> {
		let mut subresult = self.subresult(None)?;
		let length = subresult.uint32()? as usize;
		if subresult.buf.len() < subresult.offset + length {
			return Err(Error::OutOfOffset);
		}
		Ok(subresult.buf[subresult.offset..subresult.offset + length].into())
	}

	/// Read [`String`] at current position, then advance
	pub fn string(&mut self) -> Result<String> {
		String::from_utf8(self.bytes()?).map_err(|_| Error::InvalidRange)
	}

	/// Read [`u8`] at current position, then advance
	pub fn uint8(&mut self) -> Result<u8> {
		Ok(self.read_padleft::<1>()?[0])
	}

	/// Read [`u32`] at current position, then advance
	pub fn uint32(&mut self) -> Result<u32> {
		Ok(u32::from_be_bytes(self.read_padleft()?))
	}

	/// Read [`u128`] at current position, then advance
	pub fn uint128(&mut self) -> Result<u128> {
		Ok(u128::from_be_bytes(self.read_padleft()?))
	}

	/// Read [`U256`] at current position, then advance
	pub fn uint256(&mut self) -> Result<U256> {
		let buf: [u8; 32] = self.read_padleft()?;
		Ok(U256::from_big_endian(&buf))
	}

	/// Read [`u64`] at current position, then advance
	pub fn uint64(&mut self) -> Result<u64> {
		Ok(u64::from_be_bytes(self.read_padleft()?))
	}

	/// Read [`usize`] at current position, then advance
	#[deprecated = "dangerous, as usize may have different width in wasm and native execution"]
	pub fn read_usize(&mut self) -> Result<usize> {
		Ok(usize::from_be_bytes(self.read_padleft()?))
	}

	/// Slice recursive buffer, advance one word for buffer offset
	/// If `size` is [`None`] then offsets evals from internal buffer.
	///
	/// # Errors
	///
	/// Fails on bad subresult offset
	pub fn subresult(&mut self, size: Option<usize>) -> Result<AbiReader<'i>> {
		let subresult_offset = self.subresult_offset;
		let offset = if let Some(size) = size {
			self.offset += size;
			0
		} else {
			self.uint32()? as usize
		};

		if offset + self.subresult_offset > self.buf.len() {
			return Err(Error::InvalidRange);
		}

		let new_offset = offset + subresult_offset;
		Ok(AbiReader {
			buf: self.buf,
			subresult_offset: new_offset,
			offset: new_offset,
		})
	}

	/// Notify about readed data portion.
	pub fn bytes_read(&mut self, size: usize) {
		self.subresult_offset += size;
	}

	/// Is this parser reached end of buffer?
	#[must_use]
	pub fn is_finished(&self) -> bool {
		self.buf.len() == self.offset
	}
}

/// Writer for RLP encoded data
#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct AbiWriter {
	static_part: Vec<u8>,
	dynamic_part: Vec<(usize, AbiWriter)>,
	had_call: bool,
	is_dynamic: bool,
}
impl AbiWriter {
	/// Initialize internal buffers for output data, assuming no padding required
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Initialize internal buffers with data size
	#[must_use]
	pub fn new_dynamic(is_dynamic: bool) -> Self {
		Self {
			is_dynamic,
			..Default::default()
		}
	}
	/// Initialize internal buffers, inserting method selector at beginning
	#[must_use]
	pub fn new_call(method_id: u32) -> Self {
		let mut val = Self::new();
		val.static_part.extend(method_id.to_be_bytes());
		val.had_call = true;
		val
	}

	fn write_padleft(&mut self, block: &[u8]) {
		assert!(block.len() <= ABI_ALIGNMENT);
		self.static_part
			.extend(&[0; ABI_ALIGNMENT][0..ABI_ALIGNMENT - block.len()]);
		self.static_part.extend(block);
	}

	fn write_padright(&mut self, block: &[u8]) {
		assert!(block.len() <= ABI_ALIGNMENT);
		self.static_part.extend(block);
		self.static_part
			.extend(&[0; ABI_ALIGNMENT][0..ABI_ALIGNMENT - block.len()]);
	}

	/// Write [`H160`] to end of buffer
	pub fn address(&mut self, address: &H160) {
		self.write_padleft(&address.0);
	}

	/// Write [`bool`] to end of buffer
	pub fn bool(&mut self, value: &bool) {
		self.write_padleft(&[u8::from(*value)]);
	}

	/// Write [`u8`] to end of buffer
	pub fn uint8(&mut self, value: &u8) {
		self.write_padleft(&[*value]);
	}

	/// Write [`u32`] to end of buffer
	pub fn uint32(&mut self, value: &u32) {
		self.write_padleft(&u32::to_be_bytes(*value));
	}

	/// Write [`u64`] to end of buffer
	pub fn uint64(&mut self, value: &u64) {
		self.write_padleft(&u64::to_be_bytes(*value));
	}

	/// Write [`u128`] to end of buffer
	pub fn uint128(&mut self, value: &u128) {
		self.write_padleft(&u128::to_be_bytes(*value));
	}

	/// Write [`U256`] to end of buffer
	pub fn uint256(&mut self, value: &U256) {
		let mut out = [0; 32];
		value.to_big_endian(&mut out);
		self.write_padleft(&out);
	}

	/// Write [`usize`] to end of buffer
	#[deprecated = "dangerous, as usize may have different width in wasm and native execution"]
	pub fn write_usize(&mut self, value: &usize) {
		self.write_padleft(&usize::to_be_bytes(*value));
	}

	/// Append recursive data, writing pending offset at end of buffer
	pub fn write_subresult(&mut self, result: Self) {
		self.dynamic_part.push((self.static_part.len(), result));
		// Empty block, to be filled later
		self.write_padleft(&[]);
	}

	fn memory(&mut self, value: &[u8]) {
		let mut sub = Self::new();
		sub.uint32(&u32::try_from(value.len()).expect("only 32bit array length is supported"));
		for chunk in value.chunks(ABI_ALIGNMENT) {
			sub.write_padright(chunk);
		}
		self.write_subresult(sub);
	}

	/// Append recursive [`str`] at end of buffer
	pub fn string(&mut self, value: &str) {
		self.memory(value.as_bytes());
	}

	/// Append recursive [`[u8]`] at end of buffer
	pub fn bytes(&mut self, value: &[u8]) {
		self.memory(value);
	}

	/// Write [`bytes`] to end of buffer
	pub fn bytes_padleft(&mut self, block: &[u8]) {
		self.write_padleft(block);
	}

	/// Finish writer, concatenating all internal buffers
	#[must_use]
	pub fn finish(mut self) -> Vec<u8> {
		for (static_offset, part) in self.dynamic_part {
			let part_offset = self.static_part.len()
				- if self.had_call { 4 } else { 0 }
				- if self.is_dynamic { ABI_ALIGNMENT } else { 0 };

			let encoded_dynamic_offset = usize::to_be_bytes(part_offset);
			let start = static_offset + ABI_ALIGNMENT - encoded_dynamic_offset.len();
			let stop = static_offset + ABI_ALIGNMENT;
			self.static_part[start..stop].copy_from_slice(&encoded_dynamic_offset);
			self.static_part.extend(part.finish());
		}
		self.static_part
	}
}
