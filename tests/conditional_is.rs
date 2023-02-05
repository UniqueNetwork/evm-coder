use evm_coder::{dummy_contract, solidity_interface, types::*};

type Result<T> = core::result::Result<T, String>;

pub struct Contract(bool);
dummy_contract! {
	macro_rules! Contract_result {...}
	impl Contract for Contract {...}
}

#[solidity_interface(name = A)]
impl Contract {
	fn method_a() -> Result<()> {
		Ok(())
	}
}

#[solidity_interface(name = B)]
impl Contract {
	fn method_b() -> Result<()> {
		Ok(())
	}
}

#[solidity_interface(name = Contract, is(
	A(if(this.0)),
	B(if(!this.0)),
))]
impl Contract {}

#[test]
fn conditional_erc165() {
	assert!(ContractCall::supports_interface(
		&Contract(true),
		ACall::METHOD_A
	));
	assert!(!ContractCall::supports_interface(
		&Contract(false),
		ACall::METHOD_A
	));

	assert!(ContractCall::supports_interface(
		&Contract(false),
		BCall::METHOD_B
	));
	assert!(!ContractCall::supports_interface(
		&Contract(true),
		BCall::METHOD_B
	));
}
