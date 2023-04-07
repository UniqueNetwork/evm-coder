# evm-coder [![Build Status]][actions] [![Latest Version]][crates.io]

[Build Status]: https://img.shields.io/github/actions/workflow/status/uniquenetwork/evm-coder/ci.yaml?branch=master
[actions]: https://github.com/uniquenetwork/evm-coder/actions?query=branch%3Amaster
[Latest Version]: https://img.shields.io/crates/v/evm-coder.svg
[crates.io]: https://crates.io/crates/evm-coder

## Overview
Library for seamless call translation between Rust and Solidity code.

By encoding Solidity definitions in Rust, this library also provides generation of
Solidity interfaces for Ethereum developers.

## Usage
To create a contract in Substrate, make use of the `solidity_interface` attribute. This attribute should be applied to the implementation of the structure that represents your contract. It offers various parameters that enable features such as inheritance, interface validation during compilation, and other functionalities.

There is also support for function overloading using the atribute `#[solidity(rename="funcName")]`.

## Installation
Add the following line to your `Cargo.toml` project file.
```toml
[dependencies]
evm-coder = "0.3"
```

## Example
Consider this example where we're creating a contract that supports ERC721 along with an additional extension interface.

To begin, we define the interface of our contract using the following Rust code:
```rust
struct ContractHandle;

#[solidity_interface(
	name = MyContract,
	is(
		ERC721,
		CustomContract,
	)
)]
impl ContractHandle{}
```

The code above defines a contract named MyContract that implements two interfaces, namely, ERC721 and CustomContract.

Moving forward, we proceed to actually implement the ERC721 interface:
```rust
// This docs will be included into the generated `sol` file.
/// @title ERC-721 Non-Fungible Token Standard
/// @dev See https://github.com/ethereum/EIPs/blob/master/EIPS/eip-721.md
#[solidity_interface(
	name = ERC721,                  // Contract name
	events(ERC721Events),           // Include events
	expect_selector = 0x80ac58cd    // Expected selector of contract (will be matched at compile time)
)]
impl ContractHandle {

	// This docs will be included into the generated `sol` file.
	/// @notice Count all NFTs assigned to an owner
	/// @dev NFTs assigned to the zero address are considered invalid, and this
	///  function throws for queries about the zero address.
	/// @param owner An address for whom to query the balance
	/// @return The number of NFTs owned by `owner`, possibly zero
	fn balance_of(&self, owner: Address) -> Result<U256> {
		todo!()
	}

	fn owner_of(&self, token_id: U256) -> Result<Address> {
		todo!()
	}

	#[solidity(rename_selector = "safeTransferFrom")]
	fn safe_transfer_from_with_data(&mut self, from: Address, to: Address, token_id: U256, data: Bytes) -> Result<()> {
		todo!()
	}

	fn safe_transfer_from(&mut self, from: Address, to: Address, token_id: U256) -> Result<()> {
		todo!()
	}

	fn transfer_from(&mut self, caller: Caller, from: Address, to: Address, token_id: U256) -> Result<()> {
		todo!()
	}

	fn approve(&mut self, caller: Caller, approved: Address, token_id: U256) -> Result<()> {
		todo!()
	}

	fn set_approval_for_all(&mut self, caller: Caller, operator: Address, approved: bool) -> Result<()> {
		todo!()
	}

	fn get_approved(&self, token_id: U256) -> Result<Address> {
		todo!()
	}

	fn is_approved_for_all(&self, owner: Address, operator: Address) -> Result<bool> {
		todo!()
	}
}
```

In this implementation of the interface, we have included the events of `ERC721Events` that will trigger during the respective calls. To ensure seamless implementation of standard interfaces, the `expect_selector` directive in the `solidity_interface` annotation checks the contract selector at compile time, thereby preventing errors.

Now, let's proceed with the creation of events for ERC721:
```rust
#[derive(ToLog)]
pub enum ERC721Events {
	// This docs will be included into the generated `sol` file.
	/// @dev This emits when ownership of any NFT changes by any mechanism.
	Transfer {
		#[indexed]      // This field will be indexed
		from: Address,
		#[indexed]
		to: Address,
		#[indexed]
		token_id: U256,
	},

	Approval {
		#[indexed]
		owner: Address,
		#[indexed]
		approved: Address,
		#[indexed]
		token_id: U256,
	},

	ApprovalForAll {
		#[indexed]
		owner: Address,
		#[indexed]
		operator: Address,
		approved: bool,
	},
}
```

Let's create our extension:
```rust
#[solidity_interface(name = CustomContract)
impl ContractHandle {
	#[solidity(rename_selector = "doSome")]
	fn do_some_0(&mut self, caller: Caller, param: bool) -> Result<()> {
		todo!()
	}

    #[solidity(rename_selector = "doSome")]
	fn do_some_1(&mut self, caller: Caller, param: u8) -> Result<()> {
		todo!()
	}

	#[solidity(hide)]
	fn do_another(&mut self, caller: Caller, param: bool) -> Result<()> {
		todo!()
	}

	fn do_magic(&mut self, caller: Caller, param1: Enum, param2: Struct) -> Result<Option<U256>> {
		todo!()
	}
}
```
The methods `do_some_0` and `do_some_1` have been annotated with the macro `#[solidity(rename_selector = "doSome")]`. This allows them to be presented in the solidity interface as a **single** overloaded method named doSome. Meanwhile, the `do_another` method will be included in the `.sol` file but commented out. Lastly, the `do_magic` method utilizes custom types -- we can do that too!

Let's make our types available in *solidity* (`Option` is available by default):
```rust
#[derive(AbiCoder)]
struct Struct {
	a: u8,
	b: String
}

#[derive(AbiCoder, Default, Clone, Copy)]
#[repr(u8)]
enum Enum {
	First,
	Second,
	#[default]
	Third,
}
```
It's so easy to maintain your types with the `AbiCoder` derived macro.

And at the end we will specify the generators of the `sol` files:
```rust
generate_stubgen!(gen_impl, ContractHandleCall<()>, true);
generate_stubgen!(gen_iface, ContractHandleCall<()>, false);
```

The *scripts* folder contains a set of scripts for generating the interface, `sol` stub, `json abi` and the compiled contract. To do this, create the following `make` file:
```make
MyContract.sol:
	PACKAGE=package-name NAME=erc::gen_iface OUTPUT=/path/to/iface/$@ $(PATH_TO_SCRIPTS)/generate_sol.sh
	PACKAGE=package-name NAME=erc::gen_impl OUTPUT=/patch/to/stub/$@ $(PATH_TO_SCRIPTS)/generate_sol.sh

MyContract: MyContract.sol
	INPUT=/patch/to/stub/$< OUTPUT=/patch/to/compiled/contract/MyContract.raw ./.maintain/scripts/compile_stub.sh
	INPUT=/patch/to/stub/$< OUTPUT=/patch/to/abi ./.maintain/scripts/generate_abi.sh
```

As a result, we get the following `sol` interface file:
```sol
// SPDX-License-Identifier: OTHER
// This code is automatically generated

pragma solidity >=0.8.0 <0.9.0;

/// @dev common stubs holder
contract Dummy {
}

contract ERC165 is Dummy {
	function supportsInterface(bytes4 interfaceID) external view returns (bool);
}

struct Struct {
	a uint8;
	b string;
}

enum Enum {
	First,
	Second,
	Third
}

/// Optional value
struct OptionUint256 {
	/// Shows the status of accessibility of value
	bool status;
	/// Actual value if `status` is true
	uint256 value;
}

/// @title A contract that allows you to work with collections.
/// @dev the ERC-165 identifier for this interface is 0x738a0043
contract CustomContract is Dummy, ERC165 {
	/// @dev EVM selector for this function is: 0x5465a527,
	///  or in textual repr: doSome(bool)
	function doSome(bool param) public;

	/// @dev EVM selector for this function is: 0x58a93f40,
	///  or in textual repr: doSome(uint8)
	function doSome(uint8 param) public;

	// /// @dev EVM selector for this function is: 0xf41a813e,
	// ///  or in textual repr: doAnother(bool)
	// function doAnother(bool param) public;

	/// @dev EVM selector for this function is: 0x8b5c1b1a,
	///  or in textual repr: doMagic(uint8,(uint8,string))
	function doSome(Enum param1, Struct param2) public returns (OptionUint256);
}

/// @dev inlined interface
contract ERC721Events {
	event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);
	event Approval(address indexed owner, address indexed approved, uint256 indexed tokenId);
	event ApprovalForAll(address indexed owner, address indexed operator, bool approved);
}

/// @title ERC-721 Non-Fungible Token Standard
/// @dev See https://github.com/ethereum/EIPs/blob/master/EIPS/eip-721.md
/// @dev the ERC-165 identifier for this interface is 0x80ac58cd
contract ERC721 is Dummy, ERC165, ERC721Events {
	/// @notice Count all NFTs assigned to an owner
	/// @dev NFTs assigned to the zero address are considered invalid, and this
	///  function throws for queries about the zero address.
	/// @param owner An address for whom to query the balance
	/// @return The number of NFTs owned by `owner`, possibly zero
	/// @dev EVM selector for this function is: 0x70a08231,
	///  or in textual repr: balanceOf(address)
	function balanceOf(address owner) public view returns (uint256);

	/// @dev EVM selector for this function is: 0x6352211e,
	///  or in textual repr: ownerOf(uint256)
	function ownerOf(uint256 tokenId) public view returns (address);

	/// @dev EVM selector for this function is: 0xb88d4fde,
	///  or in textual repr: safeTransferFrom(address,address,uint256,bytes)
	function safeTransferFrom(
		address from,
		address to,
		uint256 tokenId,
		bytes memory data
	) public;

	/// @dev EVM selector for this function is: 0x42842e0e,
	///  or in textual repr: safeTransferFrom(address,address,uint256)
	function safeTransferFrom(
		address from,
		address to,
		uint256 tokenId
	) public;

	/// @dev EVM selector for this function is: 0x23b872dd,
	///  or in textual repr: transferFrom(address,address,uint256)
	function transferFrom(
		address from,
		address to,
		uint256 tokenId
	) public;

	/// @dev EVM selector for this function is: 0x095ea7b3,
	///  or in textual repr: approve(address,uint256)
	function approve(address approved, uint256 tokenId) public;

	/// @dev EVM selector for this function is: 0xa22cb465,
	///  or in textual repr: setApprovalForAll(address,bool)
	function setApprovalForAll(address operator, bool approved) public;

	/// @dev EVM selector for this function is: 0x081812fc,
	///  or in textual repr: getApproved(uint256)
	function getApproved(uint256 tokenId) public view returns (address);

	/// @dev EVM selector for this function is: 0xe985e9c5,
	///  or in textual repr: isApprovedForAll(address,address)
	function isApprovedForAll(address owner, address operator) public view returns (bool);
}

contract MyContract is
	Dummy,
	ERC165,
	ERC721,
	CustomContract
{}

```

## License
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in evm-coder by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
