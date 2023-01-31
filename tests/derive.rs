use evm_coder::{dummy_contract, solidity_interface};
use derivative::Derivative;

type Result<T> = core::result::Result<T, String>;

pub struct Contract(bool);

#[solidity_interface(name = A, enum(derive(Derivative)), enum(derivative(PartialEq)))]
impl Contract {
	fn method_a() -> Result<()> {
		Ok(())
	}
}
