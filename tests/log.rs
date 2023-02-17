#![allow(dead_code)]

use evm_coder::{types::*, ToLog};
use primitive_types::U256;

#[derive(ToLog)]
enum ERC721Log {
	Transfer {
		#[indexed]
		from: Address,
		#[indexed]
		to: Address,
		value: U256,
	},
	Eee {
		#[indexed]
		aaa: Address,
		bbb: U256,
	},
}
