use evm_coder::{abi::AbiType, dummy_contract, generate_stubgen, solidity_interface, types::*};
use primitive_types::U256;

type Result<T> = core::result::Result<T, String>;

pub struct ERC20;
dummy_contract! {
	macro_rules! ERC20_result {...}
	impl Contract for ERC20 {...}
}

#[solidity_interface(name = ERC20)]
impl ERC20 {
	fn decimals(&self) -> Result<u8> {
		unreachable!()
	}
	/// Get balance of specified owner
	fn balance_of(&self, _owner: Address) -> Result<U256> {
		unreachable!()
	}
	fn transfer(&mut self, _caller: Caller, _to: Address, _value: U256) -> Result<bool> {
		unreachable!()
	}
	fn transfer_from(
		&mut self,
		_caller: Caller,
		_from: Address,
		_to: Address,
		_value: U256,
	) -> Result<bool> {
		unreachable!()
	}
	fn approve(&mut self, _caller: Caller, _spender: Address, _value: U256) -> Result<bool> {
		unreachable!()
	}
	fn allowance(&self, _owner: Address, _spender: Address) -> Result<U256> {
		unreachable!()
	}
}

generate_stubgen!(gen_impl, ERC20Call, true);
generate_stubgen!(gen_iface, ERC20Call, false);
