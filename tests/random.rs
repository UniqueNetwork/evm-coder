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

#![allow(dead_code)] // This test only checks that macros is not panicking

use evm_coder::{abi::AbiType, dummy_contract, solidity_interface, types::*, ToLog};
use primitive_types::U256;

type Result<T> = core::result::Result<T, String>;

pub struct Impls;
dummy_contract! {
	macro_rules! Impls_result {...}
	impl Contract for Impls {...}
}

#[solidity_interface(name = OurInterface)]
impl Impls {
	fn fn_a(&self, _input: U256) -> Result<bool> {
		unreachable!()
	}
}

#[solidity_interface(name = OurInterface1)]
impl Impls {
	fn fn_b(&self, _input: u128) -> Result<u32> {
		unreachable!()
	}
}

#[derive(ToLog)]
enum OurEvents {
	Event1 {
		field1: u32,
	},
	Event2 {
		field1: u32,
		#[indexed]
		field2: u32,
	},
}

#[solidity_interface(
	name = OurInterface2,
	is(OurInterface),
	inline_is(OurInterface1),
	events(OurEvents)
)]
impl Impls {
	#[solidity(rename_selector = "fnK")]
	fn fn_c(&self, _input: u32) -> Result<u8> {
		unreachable!()
	}
	fn fn_d(&self, _value: u32) -> Result<u32> {
		unreachable!()
	}

	fn caller_sensitive(&self, _caller: Caller) -> Result<u8> {
		unreachable!()
	}
	fn payable(&mut self, _value: Value) -> Result<u8> {
		unreachable!()
	}

	/// Doccoment example
	fn with_doc(&self) -> Result<()> {
		unreachable!()
	}
}

#[solidity_interface(
	name = ValidSelector,
	expect_selector = 0x00000000,
)]
impl Impls {}
