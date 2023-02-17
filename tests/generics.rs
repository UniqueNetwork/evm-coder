use std::marker::PhantomData;

use evm_coder::{dummy_contract, generate_stubgen, solidity_interface, types::*};
use primitive_types::U256;

type Result<T> = core::result::Result<T, String>;

pub struct Generic<T>(PhantomData<T>);

dummy_contract! {
	macro_rules! Generic_result {...}
	impl<T> Contract for Generic<T> {...}
}

#[solidity_interface(name = GenericIs)]
impl<T> Generic<T> {
	fn test_1(&self) -> Result<U256> {
		unreachable!()
	}
}

#[solidity_interface(name = Generic, is(GenericIs))]
impl<T: Into<u32>> Generic<T> {
	fn test_2(&self) -> Result<U256> {
		unreachable!()
	}
}

generate_stubgen!(gen_iface, GenericCall<()>, false);

#[solidity_interface(name = GenericWhere)]
impl<T> Generic<T>
where
	T: core::fmt::Debug,
{
	fn test_3(&self) -> U256 {
		unreachable!()
	}
}

generate_stubgen!(gen_where_iface, GenericWhereCall<()>, false);
