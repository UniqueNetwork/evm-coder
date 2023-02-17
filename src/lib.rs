//! # evm-coder
//!
//! Library for seamless call translation between Rust and Solidity code
//!
//! By encoding solidity definitions in Rust, this library also provides generation of
//! solidity interfaces for ethereum developers
//!
//! ## Overview
//!
//! Most of this library functionality shouldn't be used directly, but via macros
//!
//! - [`solidity_interface`]
//! - [`ToLog`]
//! - [`AbiCoder`]

#![deny(missing_docs)]
#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;

use abi::{AbiRead, AbiReader, AbiWriter};
pub use evm_coder_procedural::{event_topic, fn_selector};
pub mod abi;
pub use events::{ToLog, ToTopic};
#[macro_use]
pub mod custom_signature;

/// Reexported for macro
#[doc(hidden)]
pub use ethereum;
/// Derives call enum implementing [`crate::Callable`] and [`crate::Call`] from impl block.
///
/// ## Macro syntax
///
/// `#[solidity_interface(name, is, inline_is, events)]`
/// - **`name`** - used in generated code, and for Call enum name
/// - **`is`** - used to provide inheritance in Solidity
/// - **`inline_is`** - same as **`is`**, but `ERC165::SupportsInterface` will work differently: For `is` SupportsInterface(A) will return true
///   if A is one of the interfaces the contract is inherited from (e.g. B is created as `is(A)`). If B is created as `inline_is(A)`
///   SupportsInterface(A) will internally create a new interface that combines all methods of A and B, so SupportsInterface(A) will return
///   false.
///
/// `#[solidity_interface(rename_selector)]`
/// - **`rename_selector`** - by default, selector name will be generated by transforming method name
/// from `snake_case` to `camelCase`. Use this option, if other naming convention is required.
/// I.e: method `token_uri` will be automatically renamed to `tokenUri` in selector, but name
/// required by ERC721 standard is `tokenURI`, thus we need to specify `rename_selector = "tokenURI"`
/// explicitly.
///
/// Both contract and contract methods may have doccomments, which will end up in a generated
/// solidity interface file, thus you should use [solidity syntax](https://docs.soliditylang.org/en/latest/natspec-format.html) for writing documentation in this macro
///
/// ## Example
///
/// ```ignore
/// struct SuperContract;
/// struct InlineContract;
/// struct Contract;
///
/// #[derive(ToLog)]
/// enum ContractEvents {
///     Event(#[indexed] uint32),
/// }
///
/// /// @dev This contract provides function to multiply two numbers
/// #[solidity_interface(name = MyContract, is(SuperContract), inline_is(InlineContract))]
/// impl Contract {
///     /// Multiply two numbers
///     /// @param a First number
///     /// @param b Second number
///     /// @return uint32 Product of two passed numbers
///     /// @dev This function returns error in case of overflow
///     #[weight(200 + a + b)]
///     #[solidity_interface(rename_selector = "mul")]
///     fn mul(&mut self, a: uint32, b: uint32) -> Result<uint32> {
///         Ok(a.checked_mul(b).ok_or("overflow")?)
///     }
/// }
/// ```
pub use evm_coder_procedural::solidity_interface;
/// Macro to include support for structures and enums in Solidity.
///
/// ### Overview
/// This macro is used to include support for structures and enums in Solidity.
/// This allows them to encode and decode in \ from the Solidity Abi format, as well as create their views into Solidity lang.
///
/// ### Implemented trais
/// - [`AbiType`](abi::AbiType)
/// - [`AbiRead`](abi::AbiRead)
/// - [`AbiWrite`](abi::AbiWrite)
/// - [`CanBePlacedInVec`](sealed::CanBePlacedInVec)
/// - [`SolidityTypeName`](solidity::SolidityTypeName)
/// - [`SolidityStructTy`](solidity::SolidityStructTy) - for struct
/// - [`SolidityEnumTy`](solidity::SolidityEnumTy) - for enum
///
/// ### Limitations
/// - All struct fields must implement traits listed above.
/// - Enum must have u8 layout.
/// - Enum must implement folowing traits: Default, Copy, Clone
///
/// ### Example
/// ```
/// use evm_coder::AbiCoder;
///
/// #[derive(AbiCoder)]
/// struct Foo {
///     a: u8,
///     b: String
/// }
///
/// #[derive(AbiCoder, Default, Clone, Copy)]
/// #[repr(u8)]
/// enum Color {
///     Red,
///     Green,
///     #[default]
///     Blue,
/// }
/// ```
pub use evm_coder_procedural::AbiCoder;
/// Derives [`ToLog`] for enum
///
/// Selectors will be derived from variant names, there is currently no way to have custom naming
/// for them
///
/// `#[indexed]`
/// Marks this field as indexed, so it will appear in [`ethereum::Log`] topics instead of data
pub use evm_coder_procedural::ToLog;
/// Reexported for macro
#[doc(hidden)]
pub use sha3_const;

// Api of those modules shouldn't be consumed directly, it is only exported for usage in proc macros
#[doc(hidden)]
pub mod events;
#[doc(hidden)]
#[cfg(feature = "stubgen")]
pub mod solidity;

/// Sealed traits.
pub mod sealed {
	/// Not every type should be directly placed in vec.
	/// Vec encoding is not memory efficient, as every item will be padded
	/// to 32 bytes.
	/// Instead you should use specialized types (`bytes` in case of `Vec<u8>`)
	pub trait CanBePlacedInVec {}
}

/// Solidity type definitions (aliases from solidity name to rust type)
/// To be used in [`solidity_interface`] definitions, to make sure there is no
/// type conflict between Rust code and generated definitions
pub mod types {
	#![allow(non_camel_case_types, missing_docs)]

	#[cfg(not(feature = "std"))]
	use alloc::vec::Vec;

	use primitive_types::{H160, H256, U256};

	pub type Address = H160;
	pub type Bytes4 = [u8; 4];
	pub type Topic = H256;

	#[cfg(not(feature = "std"))]
	pub type String = ::alloc::string::String;
	#[cfg(feature = "std")]
	pub type String = ::std::string::String;

	#[derive(Default, Debug, PartialEq, Eq, Clone)]
	pub struct Bytes(pub Vec<u8>);

	//#region Special types
	/// Makes function payable
	pub type Value = U256;
	/// Makes function caller-sensitive
	pub type Caller = Address;
	//#endregion

	/// Ethereum typed call message, similar to solidity
	/// `msg` object.
	pub struct Msg<C> {
		pub call: C,
		/// Address of user, which called this contract.
		pub caller: H160,
		/// Payment amount to contract.
		/// Contract should reject payment, if target call is not payable,
		/// and there is no `receiver()` function defined.
		pub value: U256,
	}

	impl From<Vec<u8>> for Bytes {
		fn from(src: Vec<u8>) -> Self {
			Self(src)
		}
	}

	#[allow(clippy::from_over_into)]
	impl Into<Vec<u8>> for Bytes {
		fn into(self) -> Vec<u8> {
			self.0
		}
	}

	impl Bytes {
		#[must_use]
		pub fn len(&self) -> usize {
			self.0.len()
		}

		#[must_use]
		pub fn is_empty(&self) -> bool {
			self.len() == 0
		}
	}
}

/// Parseable EVM call, this trait should be implemented with [`solidity_interface`] macro
pub trait Call: Sized {
	/// Parse call buffer into typed call enum
	///
	/// # Errors
	///
	/// One of call arguments has bad encoding, or value is invalid for the target type
	fn parse(selector: types::Bytes4, input: &mut AbiReader) -> abi::Result<Option<Self>>;
}

/// Type callable with ethereum message, may be implemented by [`solidity_interface`] macro
/// on interface implementation, or for externally-owned real EVM contract
pub trait Callable<C: Call>: Contract {
	/// Call contract using specified call data
	fn call(&mut self, call: types::Msg<C>) -> ResultWithPostInfoOf<Self, AbiWriter>;
}

/// Contract specific result type
pub type ResultOf<C, R> = <C as Contract>::Result<R, <C as Contract>::Error>;
/// Contract specific result type
pub type ResultWithPostInfoOf<C, R> = <C as Contract>::Result<
	<C as Contract>::WithPostInfo<R>,
	<C as Contract>::WithPostInfo<<C as Contract>::Error>,
>;

/// Contract configuration
pub trait Contract {
	/// Contract error type
	type Error: From<&'static str>;
	/// Wrapper for Result Ok/Err value
	type WithPostInfo<T>;
	/// Return value of [`Callable`], expected to be of [`core::result::Result`] type
	type Result<T, E>;

	/// Map `WithPostInfo` value
	fn map_post<I, O>(
		v: Self::WithPostInfo<I>,
		mapper: impl FnOnce(I) -> O,
	) -> Self::WithPostInfo<O>;
	/// Wrap value with default post info
	fn with_default_post<T>(v: T) -> Self::WithPostInfo<T>;
}

/// Example of `PostInfo`, used in tests
pub struct DummyPost<T>(pub T);
/// Implement dummy Contract trait, used for tests
/// Allows contract methods to return either T, or Result<T, String> for any T
#[macro_export]
macro_rules! dummy_contract {
	(
		macro_rules! $res:ident {...}
		impl$(<$($gen:ident),+ $(,)?>)? Contract for $ty:ty {...}
	) => {
		/// Generate macro to convert function return value into Contract result
		/// This macro uses autoref specialization technique, described here: https://github.com/dtolnay/case-studies/blob/master/autoref-specialization/README.md
		macro_rules! $res {
			($i:expr) => {{
				use ::evm_coder::DummyPost;
				struct Wrapper<T>(core::cell::Cell<Option<T>>);
				type O<T> = ::core::result::Result<DummyPost<T>, DummyPost<String>>;
				trait Matcher<T> {
					fn convert(&self) -> O<T>;
				}
				impl<T> Matcher<T> for &Wrapper<::core::result::Result<T, String>> {
					fn convert(&self) -> O<T> {
						let i = self.0.take().unwrap();
						i.map(DummyPost).map_err(DummyPost)
					}
				}
				impl<T> Matcher<T> for Wrapper<T> {
					fn convert(&self) -> O<T> {
						let i = self.0.take().unwrap();
						Ok(DummyPost(i))
					}
				}
				(&&Wrapper(core::cell::Cell::new(Some($i)))).convert()
			}};
		}
		impl $(<$($gen),+>)? $crate::Contract for $ty {
			type Error = String;
			type WithPostInfo<RR> = $crate::DummyPost<RR>;
			type Result<RR, EE> = core::result::Result<RR, EE>;
			fn map_post<II, OO>(v: Self::WithPostInfo<II>, mapper: impl FnOnce(II) -> OO) -> Self::WithPostInfo<OO> {
				$crate::DummyPost(mapper(v.0))
			}
			/// Wrap value with default post info
			fn with_default_post<TT>(v: TT) -> Self::WithPostInfo<TT> {
				$crate::DummyPost(v)
			}
		}
	};
}

/// Implementation of ERC165 is implicitly generated for all interfaces in [`solidity_interface`],
/// this structure holds parsed data for `ERC165Call` subvariant
///
/// Note: no [`Callable`] implementation is provided, call implementation is inlined into every
/// implementing contract
///
/// See <https://eips.ethereum.org/EIPS/eip-165>
#[derive(Debug, PartialEq)]
pub enum ERC165Call {
	/// ERC165 provides single method, which returns true, if contract
	/// implements specified interface
	SupportsInterface {
		/// Requested interface
		interface_id: types::Bytes4,
	},
}

impl ERC165Call {
	/// ERC165 selector is provided by standard
	pub const INTERFACE_ID: types::Bytes4 = u32::to_be_bytes(0x01ff_c9a7);
}

impl Call for ERC165Call {
	fn parse(selector: types::Bytes4, input: &mut AbiReader) -> abi::Result<Option<Self>> {
		if selector != Self::INTERFACE_ID {
			return Ok(None);
		}
		Ok(Some(Self::SupportsInterface {
			interface_id: types::Bytes4::abi_read(input)?,
		}))
	}
}

/// Generate "tests", which will generate solidity code on execution and print it to stdout
/// Script at `.maintain/scripts/generate_api.sh` can split this output from test runtime
///
/// This macro receives type usage as second argument, but you can use anything as generics,
/// because no bounds are implied
#[macro_export]
macro_rules! generate_stubgen {
	($name:ident, $decl:ty, $is_impl:literal) => {
		#[cfg(feature = "stubgen")]
		#[test]
		#[ignore]
		fn $name() {
			use evm_coder::solidity::TypeCollector;
			let mut out = TypeCollector::new();
			<$decl>::generate_solidity_interface(&mut out, $is_impl);
			println!("=== SNIP START ===");
			println!("// SPDX-License-Identifier: OTHER");
			println!("// This code is automatically generated");
			println!();
			println!("pragma solidity >=0.8.0 <0.9.0;");
			println!();
			for b in out.finish() {
				println!("{}", b);
			}
			println!("=== SNIP END ===");
		}
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn function_selector_generation() {
		assert_eq!(fn_selector!(transfer(address, uint256)), 0xa9059cbb);
	}

	#[test]
	fn event_topic_generation() {
		assert_eq!(
			hex::encode(&event_topic!(Transfer(address, address, uint256))[..]),
			"ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
		);
	}
}
