use super::{TypeCollector, SolidityTypeName, SolidityTupleType, sealed};
use crate::types::*;
use core::fmt;

impl sealed::CanBePlacedInVec for uint256 {}
impl sealed::CanBePlacedInVec for string {}
impl sealed::CanBePlacedInVec for address {}

macro_rules! solidity_type_name {
    ($($ty:ty => $name:literal $simple:literal = $default:literal),* $(,)?) => {
        $(
            impl SolidityTypeName for $ty {
                fn solidity_name(writer: &mut impl core::fmt::Write, _tc: &TypeCollector) -> core::fmt::Result {
                    write!(writer, $name)
                }
				fn is_simple() -> bool {
					$simple
				}
				fn solidity_default(writer: &mut impl core::fmt::Write, _tc: &TypeCollector) -> core::fmt::Result {
					write!(writer, $default)
				}
            }
        )*
    };
}

solidity_type_name! {
	uint8 => "uint8" true = "0",
	uint32 => "uint32" true = "0",
	uint64 => "uint64" true = "0",
	uint128 => "uint128" true = "0",
	uint256 => "uint256" true = "0",
	bytes4 => "bytes4" true = "bytes4(0)",
	address => "address" true = "0x0000000000000000000000000000000000000000",
	string => "string" false = "\"\"",
	bytes => "bytes" false = "hex\"\"",
	bool => "bool" true = "false",
}

impl SolidityTypeName for void {
	fn solidity_name(_writer: &mut impl fmt::Write, _tc: &TypeCollector) -> fmt::Result {
		Ok(())
	}
	fn is_simple() -> bool {
		true
	}
	fn solidity_default(_writer: &mut impl fmt::Write, _tc: &TypeCollector) -> fmt::Result {
		Ok(())
	}
	fn is_void() -> bool {
		true
	}
}

impl<T: SolidityTypeName + sealed::CanBePlacedInVec> SolidityTypeName for Vec<T> {
	fn solidity_name(writer: &mut impl fmt::Write, tc: &TypeCollector) -> fmt::Result {
		T::solidity_name(writer, tc)?;
		write!(writer, "[]")
	}
	fn is_simple() -> bool {
		false
	}
	fn solidity_default(writer: &mut impl fmt::Write, tc: &TypeCollector) -> fmt::Result {
		write!(writer, "new ")?;
		T::solidity_name(writer, tc)?;
		write!(writer, "[](0)")
	}
}

macro_rules! count {
    () => (0usize);
    ( $x:tt $($xs:tt)* ) => (1usize + count!($($xs)*));
}

macro_rules! impl_tuples {
	($($ident:ident)+) => {
		impl<$($ident),+> sealed::CanBePlacedInVec for ($($ident,)+) {}
		impl<$($ident: SolidityTypeName + 'static),+> SolidityTupleType for ($($ident,)+) {
			fn names(tc: &TypeCollector) -> Vec<string> {
				let mut collected = Vec::with_capacity(Self::len());
				$({
					let mut out = string::new();
					$ident::solidity_name(&mut out, tc).expect("no fmt error");
					collected.push(out);
				})*;
				collected
			}

			fn len() -> usize {
				count!($($ident)*)
			}
		}
		impl<$($ident: SolidityTypeName + 'static),+> SolidityTypeName for ($($ident,)+) {
			fn solidity_name(writer: &mut impl fmt::Write, tc: &TypeCollector) -> fmt::Result {
				write!(writer, "{}", tc.collect_tuple::<Self>())
			}
			fn is_simple() -> bool {
				false
			}
			#[allow(unused_assignments)]
			fn solidity_default(writer: &mut impl fmt::Write, tc: &TypeCollector) -> fmt::Result {
				write!(writer, "{}(", tc.collect_tuple::<Self>())?;
				let mut first = true;
				$(
					if !first {
						write!(writer, ",")?;
					} else {
						first = false;
					}
					<$ident>::solidity_default(writer, tc)?;
				)*
				write!(writer, ")")
			}
		}
	};
}

impl_tuples! {A}
impl_tuples! {A B}
impl_tuples! {A B C}
impl_tuples! {A B C D}
impl_tuples! {A B C D E}
impl_tuples! {A B C D E F}
impl_tuples! {A B C D E F G}
impl_tuples! {A B C D E F G H}
impl_tuples! {A B C D E F G H I}
impl_tuples! {A B C D E F G H I J}

impl sealed::CanBePlacedInVec for Property {}
impl StructCollect for Property {
	fn name() -> String {
		"Property".into()
	}

	fn declaration() -> String {
		let mut str = String::new();
		writeln!(str, "/// @dev Property struct").unwrap();
		writeln!(str, "struct {} {{", Self::name()).unwrap();
		writeln!(str, "\tstring key;").unwrap();
		writeln!(str, "\tbytes value;").unwrap();
		writeln!(str, "}}").unwrap();
		str
	}
}

impl SolidityTypeName for Property {
	fn solidity_name(writer: &mut impl fmt::Write, tc: &TypeCollector) -> fmt::Result {
		write!(writer, "{}", tc.collect_struct::<Self>())
	}

	fn is_simple() -> bool {
		false
	}

	fn solidity_default(writer: &mut impl fmt::Write, tc: &TypeCollector) -> fmt::Result {
		write!(writer, "{}(", tc.collect_struct::<Self>())?;
		address::solidity_default(writer, tc)?;
		write!(writer, ",")?;
		uint256::solidity_default(writer, tc)?;
		write!(writer, ")")
	}
}

impl SolidityTupleType for Property {
	fn names(tc: &TypeCollector) -> Vec<string> {
		let mut collected = Vec::with_capacity(Self::len());
		{
			let mut out = string::new();
			string::solidity_name(&mut out, tc).expect("no fmt error");
			collected.push(out);
		}
		{
			let mut out = string::new();
			bytes::solidity_name(&mut out, tc).expect("no fmt error");
			collected.push(out);
		}
		collected
	}

	fn len() -> usize {
		2
	}
}