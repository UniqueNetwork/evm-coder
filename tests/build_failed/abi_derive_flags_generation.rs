use evm_coder_procedural::AbiCoderFlags;
use bondrewd::Bitfields;

#[derive(AbiCoderFlags, Bitfields)]
struct EmptyStruct {}


#[derive(AbiCoderFlags, Bitfields)]
struct OneField {
	#[bondrewd(bits = "0..1")]
	a: bool,
}

#[derive(AbiCoderFlags, Bitfields)]
struct ReserveField {
	#[bondrewd(reserve, bits = "0..1")]
	a: bool,
}

#[derive(AbiCoderFlags, Bitfields)]
struct MultipleFields {
	#[bondrewd(bits = "0..1")]
	a: bool,
	#[bondrewd(bits = "1..2")]
	b: bool,
	#[bondrewd(bits = "2..8")]
	c: u8,
}

fn main() {
	assert!(false);
}
