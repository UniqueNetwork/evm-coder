use core::str::from_utf8;

use super::{AbiDecoder, AbiEncoder, Error};
use crate::{abi::Result, custom_signature::SignatureUnit, types::*};

/// Helper for type.
pub trait AbiType {
	/// Signature for Ethereum ABI.
	const SIGNATURE: SignatureUnit;
	/// Is this a dynamic type, per spec.
	const IS_DYNAMIC: bool;
	/// How many AbiWords static data this type should occupy
	const HEAD_WORDS: u32;

	/// Signature as str.
	#[must_use]
	fn signature() -> &'static str {
		from_utf8(&Self::SIGNATURE.data[..Self::SIGNATURE.len]).expect("bad utf-8")
	}
}
impl<T> AbiType for &T
where
	T: AbiType,
{
	const SIGNATURE: SignatureUnit = T::SIGNATURE;
	const IS_DYNAMIC: bool = T::IS_DYNAMIC;
	const HEAD_WORDS: u32 = T::HEAD_WORDS;
}

/// Encode value using ABI encoding.
pub trait AbiEncode: Sized + AbiType {
	fn enc(&self, out: &mut AbiEncoder);
	fn abi_encode(&self) -> Vec<u8> {
		let mut encoder = AbiEncoder::new(vec![], 0, 0);
		encoder.encode_tail(self);
		encoder.into_inner()
	}
	fn abi_encode_call(&self, selector: Bytes4) -> Vec<u8> {
		let mut encoder = AbiEncoder::new(selector.into(), 4, 4);
		encoder.encode_tail(self);
		encoder.into_inner()
	}
}
impl<T> AbiEncode for &T
where
	T: AbiEncode,
{
	fn enc(&self, out: &mut AbiEncoder) {
		(*self).enc(out);
	}
}
pub trait AbiEncodeZero: AbiEncode {
	fn enc_zero(out: &mut AbiEncoder);
}
impl<T: Default + AbiEncode> AbiEncodeZero for T {
	fn enc_zero(out: &mut AbiEncoder) {
		T::default().enc(out)
	}
}

/// Decode ABI value.
pub trait AbiDecode: Sized + AbiType {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self>;
	fn abi_decode(input: &[u8]) -> Result<Self> {
		let mut decoder = AbiDecoder::new(input, 0)?;
		Self::dec(&mut decoder)
	}
	fn abi_decode_call(input: &[u8]) -> Result<(Bytes4, Self)> {
		let mut num = [0; 4];
		num.copy_from_slice(&input[..4]);
		Ok((BytesFixed(num), Self::abi_decode(&input[4..])?))
	}
}
/// Assert read value is zero.
pub trait AbiDecodeZero: AbiDecode {
	fn dec_zero(input: &mut AbiDecoder<'_>) -> Result<()>;
}
impl<T: Default + AbiDecode + PartialEq> AbiDecodeZero for T {
	fn dec_zero(input: &mut AbiDecoder<'_>) -> Result<()> {
		let value = T::dec(input)?;
		if value != T::default() {
			return Err(Error::InvalidRange);
		}
		Ok(())
	}
}
