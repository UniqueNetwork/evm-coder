//! This test only checks that macros is not panicking
#![allow(dead_code)]

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
