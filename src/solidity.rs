#[cfg(not(feature = "std"))]
use alloc::{string::String};
use core::{fmt, marker::PhantomData};
use impl_trait_for_tuples::impl_for_tuples;
use crate::types::*;

pub trait SolidityTypeName: 'static {
	fn solidity_name(writer: &mut impl fmt::Write) -> fmt::Result;
	fn is_void() -> bool {
		false
	}
}

macro_rules! solidity_type_name {
    ($($ty:ident => $name:expr),* $(,)?) => {
        $(
            impl SolidityTypeName for $ty {
                fn solidity_name(writer: &mut impl core::fmt::Write) -> core::fmt::Result {
                    write!(writer, $name)
                }
            }
        )*
    };
}

solidity_type_name! {
	uint8 => "uint8",
	uint32 => "uint32",
	uint128 => "uint128",
	uint256 => "uint256",
	address => "address",
	string => "memory string",
	bytes => "memory bytes",
	bool => "bool",
}
impl SolidityTypeName for void {
	fn solidity_name(_writer: &mut impl fmt::Write) -> fmt::Result {
		Ok(())
	}
	fn is_void() -> bool {
		true
	}
}

pub trait SolidityArguments {
	fn solidity_name(&self, writer: &mut impl fmt::Write) -> fmt::Result;
	fn is_empty(&self) -> bool {
		self.len() == 0
	}
	fn len(&self) -> usize;
}

#[derive(Default)]
pub struct UnnamedArgument<T>(PhantomData<*const T>);

impl<T: SolidityTypeName> SolidityArguments for UnnamedArgument<T> {
	fn solidity_name(&self, writer: &mut impl fmt::Write) -> fmt::Result {
		if !T::is_void() {
			T::solidity_name(writer)
		} else {
			Ok(())
		}
	}
	fn len(&self) -> usize {
		if T::is_void() {
			0
		} else {
			1
		}
	}
}

pub struct NamedArgument<T>(&'static str, PhantomData<*const T>);

impl<T> NamedArgument<T> {
	pub fn new(name: &'static str) -> Self {
		Self(name, Default::default())
	}
}

impl<T: SolidityTypeName> SolidityArguments for NamedArgument<T> {
	fn solidity_name(&self, writer: &mut impl fmt::Write) -> fmt::Result {
		if !T::is_void() {
			T::solidity_name(writer)?;
			write!(writer, " {}", self.0)
		} else {
			Ok(())
		}
	}
	fn len(&self) -> usize {
		if T::is_void() {
			0
		} else {
			1
		}
	}
}

impl SolidityArguments for () {
	fn solidity_name(&self, _writer: &mut impl fmt::Write) -> fmt::Result {
		Ok(())
	}
	fn len(&self) -> usize {
		0
	}
}

#[impl_for_tuples(1, 5)]
impl SolidityArguments for Tuple {
	for_tuples!( where #( Tuple: SolidityArguments ),* );

	fn solidity_name(&self, writer: &mut impl fmt::Write) -> fmt::Result {
		let mut first = true;
		for_tuples!( #(
            if !Tuple.is_empty() {
                if !first {
                    write!(writer, ", ")?;
                }
                first = false;
                Tuple.solidity_name(writer)?;
            }
        )* );
		Ok(())
	}
	fn len(&self) -> usize {
		for_tuples!( #( Tuple.len() )+* )
	}
}

pub trait SolidityFunctions {
	fn solidity_name(&self, writer: &mut impl fmt::Write) -> fmt::Result;
}

pub enum SolidityMutability {
	Pure,
	View,
	Mutable,
}
pub struct SolidityFunction<A, R> {
	pub name: &'static str,
	pub args: A,
	pub result: R,
	pub mutability: SolidityMutability,
}
impl<A: SolidityArguments, R: SolidityArguments> SolidityFunctions for SolidityFunction<A, R> {
	fn solidity_name(&self, writer: &mut impl fmt::Write) -> fmt::Result {
		write!(writer, "function {}(", self.name)?;
		self.args.solidity_name(writer)?;
		write!(writer, ") external")?;
		match &self.mutability {
			SolidityMutability::Pure => write!(writer, " pure")?,
			SolidityMutability::View => write!(writer, " view")?,
			SolidityMutability::Mutable => {}
		}
		if !self.result.is_empty() {
			write!(writer, " returns (")?;
			self.result.solidity_name(writer)?;
			write!(writer, ")")?;
		}
		writeln!(writer, ";")
	}
}

#[impl_for_tuples(0, 12)]
impl SolidityFunctions for Tuple {
	for_tuples!( where #( Tuple: SolidityFunctions ),* );

	fn solidity_name(&self, writer: &mut impl fmt::Write) -> fmt::Result {
		let mut first = false;
		for_tuples!( #(
            Tuple.solidity_name(writer)?;
        )* );
		Ok(())
	}
}

pub struct SolidityInterface<F: SolidityFunctions> {
	pub name: &'static str,
	pub functions: F,
}

impl<F: SolidityFunctions> SolidityInterface<F> {
	pub fn format(&self, out: &mut impl fmt::Write) -> fmt::Result {
		writeln!(out, "interface {} {{", self.name)?;
		self.functions.solidity_name(out)?;
		writeln!(out, "}}")?;
		Ok(())
	}
}
