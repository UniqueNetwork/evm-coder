// Copyright 2019-2022 Unique Network (Gibraltar) Ltd.
// This file is part of Unique Network.

// Unique Network is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Unique Network is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Unique Network. If not, see <http://www.gnu.org/licenses/>.

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;

use abi::{AbiRead, AbiReader, AbiWriter};
pub use evm_coder_procedural::{event_topic, fn_selector};
pub mod abi;
pub use events::{ToLog, ToTopic};
use execution::DispatchInfo;
pub mod execution;
#[macro_use]
pub mod custom_signature;

/// Derives call enum implementing [`crate::Callable`], [`crate::Weighted`]
/// and [`crate::Call`] from impl block.
///
/// ## Macro syntax
///
/// `#[solidity_interface(name, is, inline_is, events)]`
/// - *name* - used in generated code, and for Call enum name
/// - *is* - used to provide inheritance in Solidity
/// - *inline_is* - same as `is`, but ERC165::SupportsInterface will work differently: For `is` SupportsInterface(A) will return true
///   if A is one of the interfaces the contract is inherited from (e.g. B is created as `is(A)`). If B is created as `inline_is(A)`
///   SupportsInterface(A) will internally create a new interface that combines all methods of A and B, so SupportsInterface(A) will return
///   false.
///
/// `#[weight(value)]`
/// Can be added to every method of impl block, used for deriving [`crate::Weighted`], which
/// is used by substrate bridge.
/// - *value*: expression, which evaluates to weight required to call this method.
/// This expression can use call arguments to calculate non-constant execution time.
/// This expression should evaluate faster than actual execution does, and may provide worse case
/// than one is called.
///
/// `#[solidity_interface(rename_selector)]`
/// - *rename_selector* - by default, selector name will be generated by transforming method name
/// from snake_case to camelCase. Use this option, if other naming convention is required.
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
/// See [`solidity_interface`]
pub use evm_coder_procedural::solidity;
/// See [`solidity_interface`]
pub use evm_coder_procedural::weight;
pub use evm_coder_procedural::AbiCoder;
pub use sha3_const;

/// Derives [`ToLog`] for enum
///
/// Selectors will be derived from variant names, there is currently no way to have custom naming
/// for them
///
/// `#[indexed]`
/// Marks this field as indexed, so it will appear in [`ethereum::Log`] topics instead of data
pub use evm_coder_procedural::ToLog;

// Api of those modules shouldn't be consumed directly, it is only exported for usage in proc macros
#[doc(hidden)]
pub mod events;
#[doc(hidden)]
#[cfg(feature = "stubgen")]
pub mod solidity;

/// Solidity type definitions (aliases from solidity name to rust type)
/// To be used in [`solidity_interface`] definitions, to make sure there is no
/// type conflict between Rust code and generated definitions
pub mod types {
	#![allow(non_camel_case_types, missing_docs)]

	#[cfg(not(feature = "std"))]
	use alloc::{vec::Vec};
	use primitive_types::{U256, H160, H256};

	pub type address = H160;
	pub type uint8 = u8;
	pub type uint16 = u16;
	pub type uint32 = u32;
	pub type uint64 = u64;
	pub type uint128 = u128;
	pub type uint256 = U256;
	pub type bytes4 = [u8; 4];
	pub type topic = H256;

	#[cfg(not(feature = "std"))]
	pub type string = ::alloc::string::String;
	#[cfg(feature = "std")]
	pub type string = ::std::string::String;

	#[derive(Default, Debug, PartialEq, Clone)]
	pub struct bytes(pub Vec<u8>);

	/// Solidity doesn't have `void` type, however we have special implementation
	/// for empty tuple return type
	pub type void = ();

	//#region Special types
	/// Makes function payable
	pub type value = U256;
	/// Makes function caller-sensitive
	pub type caller = address;
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

	impl From<Vec<u8>> for bytes {
		fn from(src: Vec<u8>) -> Self {
			Self(src)
		}
	}

	#[allow(clippy::from_over_into)]
	impl Into<Vec<u8>> for bytes {
		fn into(self) -> Vec<u8> {
			self.0
		}
	}

	impl bytes {
		#[must_use]
		pub fn len(&self) -> usize {
			self.0.len()
		}

		#[must_use]
		pub fn is_empty(&self) -> bool {
			self.len() == 0
		}
	}

	#[derive(Debug, Default)]
	pub struct Property {
		pub key: string,
		pub value: bytes,
	}
}

/// Parseable EVM call, this trait should be implemented with [`solidity_interface`] macro
pub trait Call: Sized {
	/// Parse call buffer into typed call enum
	fn parse(selector: types::bytes4, input: &mut AbiReader) -> execution::Result<Option<Self>>;
}

/// Intended to be used as `#[weight]` output type
/// Should be same between evm-coder and substrate to avoid confusion
///
/// Isn't same thing as gas, some mapping is required between those types
pub type Weight = frame_support::weights::Weight;

/// In substrate, we have benchmarking, which allows
/// us to not rely on gas metering, but instead predict amount of gas to execute call
pub trait Weighted: Call {
	/// Predict weight of this call
	fn weight(&self) -> DispatchInfo;
}

/// Type callable with ethereum message, may be implemented by [`solidity_interface`] macro
/// on interface implementation, or for externally-owned real EVM contract
pub trait Callable<C: Call> {
	/// Call contract using specified call data
	fn call(&mut self, call: types::Msg<C>) -> execution::ResultWithPostInfo<AbiWriter>;
}

/// Implementation of ERC165 is implicitly generated for all interfaces in [`solidity_interface`],
/// this structure holds parsed data for ERC165Call subvariant
///
/// Note: no [`Callable`] implementation is provided, call implementation is inlined into every
/// implementing contract
///
/// See <https://eips.ethereum.org/EIPS/eip-165>
#[derive(Debug)]
pub enum ERC165Call {
	/// ERC165 provides single method, which returns true, if contract
	/// implements specified interface
	SupportsInterface {
		/// Requested interface
		interface_id: types::bytes4,
	},
}

impl ERC165Call {
	/// ERC165 selector is provided by standard
	pub const INTERFACE_ID: types::bytes4 = u32::to_be_bytes(0x01ffc9a7);
}

impl Call for ERC165Call {
	fn parse(selector: types::bytes4, input: &mut AbiReader) -> execution::Result<Option<Self>> {
		if selector != Self::INTERFACE_ID {
			return Ok(None);
		}
		Ok(Some(Self::SupportsInterface {
			interface_id: types::bytes4::abi_read(input)?,
		}))
	}
}

/// Generate "tests", which will generate solidity code on execution and print it to stdout
/// Script at .maintain/scripts/generate_api.sh can split this output from test runtime
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
