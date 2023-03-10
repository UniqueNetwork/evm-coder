use core::str::from_utf8;

use crate::{
	abi::{AbiReader, AbiWriter, Result},
	custom_signature::SignatureUnit,
};

/// Helper for type.
pub trait AbiType {
	/// Signature for Ethereum ABI.
	const SIGNATURE: SignatureUnit;

	/// Signature as str.
	#[must_use]
	fn as_str() -> &'static str {
		from_utf8(&Self::SIGNATURE.data[..Self::SIGNATURE.len]).expect("bad utf-8")
	}

	/// Is type dynamic sized.
	fn is_dynamic() -> bool;

	/// Size for type aligned to [`ABI_ALIGNMENT`](crate::abi::ABI_ALIGNMENT).
	fn size() -> usize;
}

/// [`AbiReader`] implements reading of many types.
pub trait AbiRead {
	/// Read item from current position, advanding decoder
	///
	/// # Errors
	///
	/// Value is not decodeable, or is invalid for the target type
	fn abi_read(reader: &mut AbiReader) -> Result<Self>
	where
		Self: Sized;
}

/// For questions about inability to provide custom implementations,
/// see [`AbiRead`]
pub trait AbiWrite {
	/// Write value to end of specified encoder
	fn abi_write(&self, writer: &mut AbiWriter);
}
