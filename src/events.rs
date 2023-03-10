use ethereum::Log;
use primitive_types::{H160, H256, U256};

use crate::types::Address;

/// Implementation of this trait should not be written manually,
/// instead use [`crate::ToLog`] proc macros.
///
/// See also [`evm_coder_procedural::ToLog`], [solidity docs on events](https://docs.soliditylang.org/en/develop/contracts.html#events)
pub trait ToLog {
	/// Convert event to [`ethereum::Log`].
	/// Because event by itself doesn't contains current contract
	/// address, it should be specified manually.
	fn to_log(&self, contract: H160) -> Log;
}

/// Only items implementing `ToTopic` may be used as `#[indexed]` field
/// in [`crate::ToLog`] macro usage.
///
/// See also (solidity docs on events)[<https://docs.soliditylang.org/en/develop/contracts.html#events>]
pub trait ToTopic {
	/// Convert value to topic to be used in [`ethereum::Log`]
	fn to_topic(&self) -> H256;
}

impl ToTopic for H256 {
	fn to_topic(&self) -> H256 {
		*self
	}
}

impl ToTopic for U256 {
	fn to_topic(&self) -> H256 {
		let mut out = [0u8; 32];
		self.to_big_endian(&mut out);
		H256(out)
	}
}

impl ToTopic for Address {
	fn to_topic(&self) -> H256 {
		let mut out = [0u8; 32];
		out[12..32].copy_from_slice(&self.0);
		H256(out)
	}
}

impl ToTopic for u32 {
	fn to_topic(&self) -> H256 {
		let mut out = [0u8; 32];
		out[28..32].copy_from_slice(&self.to_be_bytes());
		H256(out)
	}
}
