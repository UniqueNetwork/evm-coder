#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use primitive_types::{H160, U256};

use crate::{
	abi::{
		traits::{AbiRead, AbiType, AbiWrite},
		AbiReader, AbiWriter, Result, ABI_ALIGNMENT,
	},
	custom_signature::SignatureUnit,
	make_signature, sealed,
	types::{Bytes, Bytes4, String},
};

macro_rules! impl_abi_type {
	($ty:ty, $name:ident, $dynamic:literal) => {
		impl sealed::CanBePlacedInVec for $ty {}

		impl AbiType for $ty {
			const SIGNATURE: SignatureUnit = make_signature!(new fixed(stringify!($name)));

			fn is_dynamic() -> bool {
				$dynamic
			}

			fn size() -> usize {
				ABI_ALIGNMENT
			}
		}
	};
}

macro_rules! impl_abi_readable {
	($ty:ty, $method:ident) => {
		impl AbiRead for $ty {
			fn abi_read(reader: &mut AbiReader) -> Result<$ty> {
				reader.$method()
			}
		}
	};
}

macro_rules! impl_abi_writeable {
	($ty:ty, $method:ident) => {
		impl AbiWrite for $ty {
			fn abi_write(&self, writer: &mut AbiWriter) {
				writer.$method(&self)
			}
		}
	};
}

macro_rules! impl_abi {
	($ty:ty, $method:ident, $dynamic:literal) => {
		impl_abi_type!($ty, $method, $dynamic);
		impl_abi_readable!($ty, $method);
		impl_abi_writeable!($ty, $method);
	};
}

impl_abi!(bool, bool, false);
impl_abi!(u8, uint8, false);
impl_abi!(u32, uint32, false);
impl_abi!(u64, uint64, false);
impl_abi!(u128, uint128, false);
impl_abi!(U256, uint256, false);
impl_abi!(H160, address, false);
impl_abi!(String, string, true);

impl_abi_writeable!(&str, string);

impl_abi_type!(Bytes, bytes, true);

impl AbiRead for Bytes {
	fn abi_read(reader: &mut AbiReader) -> Result<Bytes> {
		Ok(Bytes(reader.bytes()?))
	}
}

impl AbiWrite for Bytes {
	fn abi_write(&self, writer: &mut AbiWriter) {
		writer.bytes(self.0.as_slice());
	}
}

impl_abi_type!(Bytes4, bytes4, false);
impl AbiRead for Bytes4 {
	fn abi_read(reader: &mut AbiReader) -> Result<Bytes4> {
		reader.bytes4()
	}
}

impl<T: AbiWrite> AbiWrite for &T {
	fn abi_write(&self, writer: &mut AbiWriter) {
		T::abi_write(self, writer);
	}
}

impl<T: AbiType> AbiType for &T {
	const SIGNATURE: SignatureUnit = T::SIGNATURE;

	fn is_dynamic() -> bool {
		T::is_dynamic()
	}

	fn size() -> usize {
		T::size()
	}
}

impl<T: AbiType + AbiRead + sealed::CanBePlacedInVec> AbiRead for Vec<T> {
	fn abi_read(reader: &mut AbiReader) -> Result<Vec<T>> {
		let mut sub = reader.subresult(None)?;
		let size = sub.uint32()? as usize;
		sub.subresult_offset = sub.offset;
		let is_dynamic = <T as AbiType>::is_dynamic();
		let mut out = Vec::with_capacity(size);
		for _ in 0..size {
			out.push(<T as AbiRead>::abi_read(&mut sub)?);
			if !is_dynamic {
				sub.bytes_read(<T as AbiType>::size());
			};
		}
		Ok(out)
	}
}

impl<T: AbiType> AbiType for Vec<T> {
	const SIGNATURE: SignatureUnit = make_signature!(new nameof(T::SIGNATURE) fixed("[]"));

	fn is_dynamic() -> bool {
		true
	}

	fn size() -> usize {
		ABI_ALIGNMENT
	}
}

impl<T: AbiWrite + AbiType> AbiWrite for Vec<T> {
	fn abi_write(&self, writer: &mut AbiWriter) {
		let is_dynamic = T::is_dynamic();
		let mut sub = if is_dynamic {
			AbiWriter::new_dynamic(is_dynamic)
		} else {
			AbiWriter::new()
		};

		// Write items count
		u32::try_from(self.len())
			.expect("only 32bit array length is supported")
			.abi_write(&mut sub);

		for item in self {
			item.abi_write(&mut sub);
		}
		writer.write_subresult(sub);
	}
}

impl AbiWrite for () {
	fn abi_write(&self, _writer: &mut AbiWriter) {}
}

macro_rules! impl_tuples {
	($($ident:ident)+) => {
		impl<$($ident: AbiType,)+> AbiType for ($($ident,)+)
		where
        $(
            $ident: AbiType,
        )+
		{
            const SIGNATURE: SignatureUnit = make_signature!(
                new fixed("(")
                $(nameof(<$ident>::SIGNATURE) fixed(","))+
                shift_left(1)
                fixed(")")
            );

			fn is_dynamic() -> bool {
				false
				$(
					|| <$ident>::is_dynamic()
				)*
			}

			fn size() -> usize {
				0 $(+ <$ident>::size())+
			}
		}

		impl<$($ident),+> sealed::CanBePlacedInVec for ($($ident,)+) {}

		impl<$($ident),+> AbiRead for ($($ident,)+)
		where
			Self: AbiType,
			$($ident: AbiRead + AbiType,)+
		{
			fn abi_read(reader: &mut AbiReader) -> Result<($($ident,)+)> {
				let is_dynamic = <Self>::is_dynamic();
				let size = if !is_dynamic { Some(<Self>::size()) } else { None };
				let mut subresult = reader.subresult(size)?;
				Ok((
					$({
						let value = <$ident>::abi_read(&mut subresult)?;
						if !is_dynamic {subresult.bytes_read(<$ident as AbiType>::size())};
						value
					},)+
				))
			}
		}

		#[allow(non_snake_case)]
		impl<$($ident),+> AbiWrite for ($($ident,)+)
		where
			$($ident: AbiWrite + AbiType,)+
		{
			fn abi_write(&self, writer: &mut AbiWriter) {
				let ($($ident,)+) = self;
				if <Self as AbiType>::is_dynamic() {
					let mut sub = AbiWriter::new();
					$($ident.abi_write(&mut sub);)+
					writer.write_subresult(sub);
				} else {
					$($ident.abi_write(writer);)+
				}
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

//----- impls for Option -----
impl<T: AbiType> AbiType for Option<T> {
	const SIGNATURE: SignatureUnit = <(bool, T)>::SIGNATURE;

	fn is_dynamic() -> bool {
		<(bool, T)>::is_dynamic()
	}

	fn size() -> usize {
		<(bool, T)>::size()
	}
}

impl<T: AbiWrite + AbiType + Default> AbiWrite for Option<T> {
	fn abi_write(&self, writer: &mut AbiWriter) {
		match self {
			Some(value) => (true, value).abi_write(writer),
			None => (false, T::default()).abi_write(writer),
		}
	}
}

impl<T> AbiRead for Option<T>
where
	Self: AbiType,
	T: AbiRead + AbiType,
{
	fn abi_read(reader: &mut AbiReader) -> Result<Self>
	where
		Self: Sized,
	{
		let (status, value) = <(bool, T)>::abi_read(reader)?;
		Ok(if status { Some(value) } else { None })
	}
}
