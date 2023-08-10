use bondrewd::Bitfields;
use evm_coder::{abi::AbiType, dummy_contract, generate_stubgen, solidity_interface, types::*};
use evm_coder_procedural::AbiCoderFlags;

pub struct CollectionHelper;
dummy_contract! {
	macro_rules! CollectionHelper_result {...}
	impl Contract for CollectionHelper {...}
}

#[solidity_interface(name = CollectionHelper)]
impl CollectionHelper {
	fn create_collection(_flags: CollectionFlags) -> u8 {
		unreachable!()
	}
}

#[derive(Bitfields, Debug, AbiCoderFlags, Clone, Copy)]
pub struct CollectionFlags {
	#[bondrewd(bits = "0..1")]
	pub foreign: bool,
	#[bondrewd(bits = "7..8")]
	pub external: bool,
	#[bondrewd(bits = "2..7")]
	pub reserved: u8,
}

generate_stubgen!(gen_impl, CollectionHelperCall, true);
generate_stubgen!(gen_iface, CollectionHelperCall, false);
