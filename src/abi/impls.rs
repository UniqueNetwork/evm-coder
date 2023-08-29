use primitive_types::{H160, U256};

use super::{
	AbiDecode, AbiDecodeZero, AbiDecoder, AbiEncode, AbiEncodeZero, AbiEncoder, ABI_WORD_SIZE,
};
use crate::{
	abi::{traits::AbiType, Error, Result},
	custom_signature::SignatureUnit,
	make_signature,
	types::*,
};

impl<T: AbiType> AbiType for Vec<T> {
	const SIGNATURE: SignatureUnit = make_signature!(new nameof(T::SIGNATURE) fixed("[]"));
	const HEAD_WORDS: u32 = 1;
	const IS_DYNAMIC: bool = true;
}
impl<T: AbiEncode> AbiEncode for Vec<T> {
	fn enc(&self, out: &mut AbiEncoder) {
		(self.len() as u32).enc(out);
		if T::IS_DYNAMIC {
			out.reserve_head(self.len() as u32);
			for v in self {
				(self.len() as u32 * ABI_WORD_SIZE + out.tail_size()).enc(out);
				out.encode_tail(v);
			}
		} else {
			for v in self {
				out.encode_tail(v);
			}
		}
	}
}
impl<T: AbiDecode> AbiDecode for Vec<T> {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		let len = u32::dec(input)?;
		// Not using with_capacity, len may be too big
		let mut out = Vec::new();
		let mut input = input.start_frame();
		if T::IS_DYNAMIC {
			for _ in 0..len {
				let offset = u32::dec(&mut input)?;
				out.push(T::dec(&mut input.dynamic_at(offset)?)?)
			}
		} else {
			for _ in 0..len {
				out.push(T::dec(&mut input)?)
			}
		}
		Ok(out)
	}
}

impl<T: AbiType, const S: usize> AbiType for [T; S] {
	const SIGNATURE: SignatureUnit =
		make_signature!(new nameof(T::SIGNATURE) fixed("[") numof(S) fixed("]"));
	const HEAD_WORDS: u32 = if T::IS_DYNAMIC {
		S as u32
	} else {
		T::HEAD_WORDS * S as u32
	};
	const IS_DYNAMIC: bool = T::IS_DYNAMIC;
}
impl<T: AbiEncode, const S: usize> AbiEncode for [T; S] {
	fn enc(&self, out: &mut AbiEncoder) {
		if T::IS_DYNAMIC {
			for v in self {
				(Self::HEAD_WORDS * self.len() as u32 + out.tail_size()).enc(out);
				out.encode_tail(v);
			}
		} else {
			for v in self {
				v.enc(out)
			}
		}
	}
}
impl<T: AbiDecode, const S: usize> AbiDecode for [T; S] {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		let mut out = Vec::with_capacity(S);
		if T::IS_DYNAMIC {
			for _ in 0..S {
				let offset = u32::dec(input)?;
				let mut data = input.dynamic_at(offset)?;
				out.push(T::dec(&mut data)?);
			}
		} else {
			for _ in 0..S {
				out.push(T::dec(input)?);
			}
		}
		out.try_into().map_err(|_| Error::InvalidRange)
	}
}

impl AbiType for &str {
	const SIGNATURE: SignatureUnit = make_signature!(new fixed("string"));
	const HEAD_WORDS: u32 = 1;
	const IS_DYNAMIC: bool = true;
}
impl AbiEncode for &str {
	fn enc(&self, out: &mut AbiEncoder) {
		(self.len() as u32).enc(out);
		for ele in self.as_bytes().chunks(32) {
			let mut word = [0; ABI_WORD_SIZE as usize];
			word[0..ele.len()].copy_from_slice(ele);
			out.append_tail(word);
		}
	}
}

impl AbiType for String {
	const SIGNATURE: SignatureUnit = <&str>::SIGNATURE;
	const HEAD_WORDS: u32 = <&str>::HEAD_WORDS;
	const IS_DYNAMIC: bool = <&str>::IS_DYNAMIC;
}
impl AbiEncode for String {
	fn enc(&self, out: &mut AbiEncoder) {
		self.as_str().enc(out)
	}
}
impl AbiDecode for String {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		let bytes = Bytes::dec(input)?;
		String::from_utf8(bytes.0).map_err(|_| Error::InvalidRange)
	}
}

impl AbiType for Bytes {
	const SIGNATURE: SignatureUnit = make_signature!(new fixed("bytes"));
	const HEAD_WORDS: u32 = 1;
	const IS_DYNAMIC: bool = true;
}
impl AbiEncode for Bytes {
	fn enc(&self, out: &mut AbiEncoder) {
		(self.len() as u32).enc(out);
		for ele in self.0.chunks(32) {
			let mut word = [0; ABI_WORD_SIZE as usize];
			word[0..ele.len()].copy_from_slice(ele);
			out.append_tail(word);
		}
	}
}
impl AbiDecode for Bytes {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		let len = u32::dec(input)?;
		// Not using with_capacity: len might be bad
		let mut out = Vec::new();
		// Next multiple of 32
		let full_words = len / 32;
		for _ in 0..full_words {
			let word = input.get_head()?;
			out.extend_from_slice(&word);
		}
		let leftovers = len % 32;
		if leftovers != 0 {
			let word = input.get_head()?;
			out.extend_from_slice(&word[..leftovers as usize]);
			for i in leftovers..32 {
				if word[i as usize] != 0 {
					return Err(Error::InvalidRange);
				}
			}
		}
		Ok(Self(out))
	}
}

impl<const S: usize> AbiType for BytesFixed<S> {
	const SIGNATURE: SignatureUnit = make_signature!(new fixed("bytes") numof(S));
	// Next multiple of 32
	const HEAD_WORDS: u32 = (S as u32 + 31) & !31;
	const IS_DYNAMIC: bool = false;
}
impl<const S: usize> AbiEncode for BytesFixed<S> {
	fn enc(&self, out: &mut AbiEncoder) {
		for ele in self.0.chunks(32) {
			let mut word = [0; ABI_WORD_SIZE as usize];
			word[0..ele.len()].copy_from_slice(ele);
			out.append_tail(word);
		}
	}
}
impl<const S: usize> AbiDecode for BytesFixed<S> {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		// Not using with_capacity: len might be bad
		let mut out = Vec::new();
		// Next multiple of 32
		let full_words = S / 32;
		for _ in 0..full_words {
			let word = input.get_head()?;
			out.extend_from_slice(&word);
		}
		let leftovers = S % 32;
		if leftovers != 0 {
			let word = input.get_head()?;
			out.extend_from_slice(&word[..leftovers]);
			if word[leftovers..ABI_WORD_SIZE as usize]
				.iter()
				.any(|&v| v != 0)
			{
				return Err(Error::InvalidRange);
			}
		}
		out.try_into().map(Self).map_err(|_| Error::InvalidRange)
	}
}

impl AbiType for () {
	const SIGNATURE: SignatureUnit = make_signature!(new fixed("()"));
	const HEAD_WORDS: u32 = 0;
	const IS_DYNAMIC: bool = false;
}
impl AbiEncode for () {
	fn enc(&self, _out: &mut AbiEncoder) {}
}
impl AbiDecode for () {
	fn dec(_input: &mut AbiDecoder<'_>) -> Result<Self> {
		Ok(())
	}
}

const fn tuple_comp_head_words<T: AbiType>() -> u32 {
	if T::IS_DYNAMIC {
		1
	} else {
		T::HEAD_WORDS
	}
}
fn encode_tuple_comp<T: AbiEncode>(comp: &T, total_head: u32, out: &mut AbiEncoder) {
	if T::IS_DYNAMIC {
		let head = total_head * ABI_WORD_SIZE + out.tail_size();
		head.enc(out);
		out.encode_tail(comp);
	} else {
		comp.enc(out);
	}
}
fn decode_tuple_comp<T: AbiDecode>(input: &mut AbiDecoder) -> Result<T> {
	if T::IS_DYNAMIC {
		let head = u32::dec(input)?;
		let mut dynamic = input.dynamic_at(head)?;
		T::dec(&mut dynamic)
	} else {
		T::dec(input)
	}
}
macro_rules! impl_tuples {
	($($gen:ident)+) => {
		impl<$($gen: AbiType,)*> AbiType for ($($gen,)*)
		where $(
			$gen: AbiType,
		)*
		{
			const SIGNATURE: SignatureUnit = make_signature!(
				new fixed("(")
					$(nameof(<$gen>::SIGNATURE) fixed(","))+
					shift_left(1)
					fixed(")")
				);
			const HEAD_WORDS: u32 = 0 $(+ tuple_comp_head_words::<$gen>())*;
			const IS_DYNAMIC: bool = false $(|| $gen::IS_DYNAMIC)*;
		}

		#[allow(non_snake_case)]
		impl<$($gen: AbiEncode,)*> AbiEncode for ($($gen,)*) {
			#[allow(unused_variables)]
			fn enc(&self, out: &mut AbiEncoder) {
				#[allow(non_snake_case)]
				let ($($gen,)*) = self;
				$(encode_tuple_comp($gen, Self::HEAD_WORDS, out);)*
			}
		}

		#[allow(non_snake_case)]
		impl<$($gen: AbiDecode,)*> AbiDecode for ($($gen,)*) {
			fn dec(input: &mut AbiDecoder) -> Result<($($gen,)*)> {
				Ok((
					$({
						#[allow(unused_variables)]
						let $gen = 0;
						decode_tuple_comp::<$gen>(input)?
					},)*
				))
			}
		}
	};
	($($cur:ident)* @ $c:ident $($rest:ident)*) => {
		impl_tuples!($($cur)*);
		impl_tuples!($($cur)* $c @ $($rest)*);
	};
	($($cur:ident)* @) => {
		impl_tuples!($($cur)*);
	};
}
impl_tuples!(A @ B C D E F G H I J K L M N O P);

//----- impls for Option -----
impl<T: AbiType> AbiType for Option<T> {
	const SIGNATURE: SignatureUnit = <(bool, T)>::SIGNATURE;
	const HEAD_WORDS: u32 = <(bool, T)>::HEAD_WORDS;
	const IS_DYNAMIC: bool = <(bool, T)>::IS_DYNAMIC;
}
impl<T: AbiEncode + AbiEncodeZero> AbiEncode for Option<T> {
	fn enc(&self, out: &mut AbiEncoder) {
		match self {
			Some(v) => (true, v).enc(out),
			None => (false, <Zero<T>>::new()).enc(out),
		}
	}
}
impl<T: AbiDecode + AbiDecodeZero> AbiDecode for Option<T> {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		let has_value = bool::dec(input)?;
		if T::IS_DYNAMIC {
			let off = u32::dec(input)?;
			let mut input = input.dynamic_at(off)?;
			if has_value {
				Some(T::dec(&mut input)).transpose()
			} else {
				<Zero<T>>::dec(&mut input)?;
				Ok(None)
			}
		} else if has_value {
			Some(T::dec(input)).transpose()
		} else {
			<Zero<T>>::dec(input)?;
			Ok(None)
		}
	}
}

impl<T: AbiType> AbiType for Zero<T> {
	const SIGNATURE: SignatureUnit = T::SIGNATURE;
	const HEAD_WORDS: u32 = T::HEAD_WORDS;
	const IS_DYNAMIC: bool = T::IS_DYNAMIC;
}
impl<T: AbiEncodeZero> AbiEncode for Zero<T> {
	fn enc(&self, out: &mut AbiEncoder) {
		T::enc_zero(out)
	}
}
impl<T: AbiDecodeZero> AbiDecode for Zero<T> {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		T::dec_zero(input)?;
		Ok(Self::new())
	}
}

macro_rules! impl_num_abicode {
	($pref:literal $($t:ty)*) => {$(
		impl AbiType for $t {
			const SIGNATURE: SignatureUnit = make_signature!(new fixed($pref) numof(<$t>::BITS));
			const IS_DYNAMIC: bool = false;
			const HEAD_WORDS: u32 = 1;
		}
		impl AbiEncode for $t {
			// const HEAD_WORDS: u32 = 1;
			// const IS_DYNAMIC: bool = false;
			fn enc(&self, out: &mut AbiEncoder) {
				let bytes = self.to_be_bytes();
				let mut word = [0; ABI_WORD_SIZE as usize];
				word[ABI_WORD_SIZE as usize - bytes.len()..ABI_WORD_SIZE as usize].copy_from_slice(&bytes);
				out.append_head(word);
			}
		}
		impl AbiDecode for $t {
			fn dec(input: &mut AbiDecoder) -> Result<$t> {
				let head = input.get_head()?;
				let mut bytes = [0; <$t>::BITS as usize / 8];
				let offset = 32-(<$t>::BITS as usize / 8);
				for i in 0..offset {
					if head[i] != 0 {
						return Err(Error::InvalidRange);
					}
				}
				bytes.copy_from_slice(&head[offset..32]);
				Ok(<$t>::from_be_bytes(bytes))
			}
		}
	)*};
}
impl_num_abicode!("uint" u8 u16 u32 u64 u128);
impl_num_abicode!("int" i8 i16 i32 i64 i128);

impl AbiType for bool {
	const SIGNATURE: SignatureUnit = make_signature!(new fixed("bool"));
	const IS_DYNAMIC: bool = false;
	const HEAD_WORDS: u32 = 1;
}
impl AbiEncode for bool {
	fn enc(&self, out: &mut AbiEncoder) {
		(*self as u32).enc(out)
	}
}
impl AbiDecode for bool {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		let v = u32::dec(input)?;
		Ok(match v {
			0 => false,
			1 => true,
			_ => return Err(Error::InvalidRange),
		})
	}
}
impl AbiType for H160 {
	const SIGNATURE: SignatureUnit = make_signature!(new fixed("address"));
	const HEAD_WORDS: u32 = 1;
	const IS_DYNAMIC: bool = false;
}
impl AbiEncode for H160 {
	fn enc(&self, out: &mut AbiEncoder) {
		let mut word = [0; ABI_WORD_SIZE as usize];
		word[12..].copy_from_slice(&self.0);
		out.append_head(word)
	}
}
impl AbiDecode for H160 {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		let data = input.get_head()?;
		let mut out = [0; 20];
		out.copy_from_slice(&data[12..]);
		if data[0..12].iter().any(|&b| b != 0) {
			return Err(Error::InvalidRange);
		}
		Ok(H160(out))
	}
}

impl AbiType for U256 {
	const SIGNATURE: SignatureUnit = make_signature!(new fixed("uint256"));
	const HEAD_WORDS: u32 = 1;
	const IS_DYNAMIC: bool = false;
}
impl AbiEncode for U256 {
	fn enc(&self, out: &mut AbiEncoder) {
		let mut word = [0; ABI_WORD_SIZE as usize];
		self.to_big_endian(&mut word);
		out.append_head(word)
	}
}
impl AbiDecode for U256 {
	fn dec(input: &mut AbiDecoder<'_>) -> Result<Self> {
		let word = input.get_head()?;
		Ok(U256::from_big_endian(&word))
	}
}
