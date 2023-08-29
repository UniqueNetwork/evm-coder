use proc_macro2::TokenStream;
use quote::quote;

use super::extract_docs;
use crate::structs::common::{BitMath, Endianness, FieldInfo, StructInfo};

fn align_size(name: &syn::Ident, total_bytes: usize) -> syn::Result<usize> {
	Ok(match total_bytes {
		1 => 1,
		2..=4 => 4,
		5..=8 => 8,
		_ => {
			return Err(syn::Error::new(
				name.span(),
				format!("Unsupported struct size: {total_bytes}"),
			))
		}
	})
}

fn align_type(name: &syn::Ident, total_bytes: usize) -> syn::Result<TokenStream> {
	Ok(match total_bytes {
		1 => quote! { u8 },
		2..=4 => quote! { u32 },
		5..=8 => quote! { u64 },
		_ => {
			return Err(syn::Error::new(
				name.span(),
				format!("Unsupported struct size: {total_bytes}"),
			))
		}
	})
}

pub fn impl_struct_abi_type(name: &syn::Ident, total_bytes: usize) -> syn::Result<TokenStream> {
	let align_type = align_type(name, total_bytes)?;
	Ok(quote! {
		impl ::evm_coder::abi::AbiType for #name {
			const SIGNATURE: ::evm_coder::custom_signature::SignatureUnit = <(#align_type) as ::evm_coder::abi::AbiType>::SIGNATURE;
			const IS_DYNAMIC: bool = <(#align_type) as ::evm_coder::abi::AbiType>::IS_DYNAMIC;
			const HEAD_WORDS: u32 = <(#align_type) as ::evm_coder::abi::AbiType>::HEAD_WORDS;
		}
	})
}

pub fn impl_struct_abi_read(name: &syn::Ident, total_bytes: usize) -> syn::Result<TokenStream> {
	let aligned_size = align_size(name, total_bytes)?;
	let bytes = (0..total_bytes).map(|i| {
		quote! { value[#i] }
	});
	Ok(quote!(
		impl ::evm_coder::abi::AbiDecode for #name {
			fn dec(reader: &mut ::evm_coder::abi::AbiDecoder) -> ::evm_coder::abi::Result<Self> {
				use ::evm_coder::abi::ABI_WORD_SIZE;
				let word = reader.get_head()?;
				let mut value = [0; #aligned_size];
				value.copy_from_slice(&word[ABI_WORD_SIZE as usize - #aligned_size..ABI_WORD_SIZE as usize]);
				if word[0..(ABI_WORD_SIZE as usize - #aligned_size)]
					.iter()
					.any(|&b| b != 0)
				{
					return Err(::evm_coder::abi::Error::InvalidRange);
				};
				Ok(#name::from_bytes([#(#bytes),*]))
			}
		}
	))
}

pub fn impl_struct_abi_write(name: &syn::Ident, total_bytes: usize) -> syn::Result<TokenStream> {
	let aligned_size = align_size(name, total_bytes)?;
	Ok(quote!(
		impl ::evm_coder::abi::AbiEncode for #name {
			fn enc(&self, writer: &mut ::evm_coder::abi::AbiEncoder) {
				use ::evm_coder::abi::ABI_WORD_SIZE;
				let value = self.clone().into_bytes();
				let mut word = [0; ABI_WORD_SIZE as usize];
				word[ABI_WORD_SIZE as usize - #aligned_size..ABI_WORD_SIZE as usize].copy_from_slice(&value);
				writer.append_head(word);
			}
		}
	))
}

pub fn impl_struct_solidity_type<'a>(
	name: &syn::Ident,
	docs: &[String],
	total_bytes: usize,
	fields: impl Iterator<Item = &'a FieldInfo> + Clone,
) -> syn::Result<TokenStream> {
	let aligned_size = align_size(name, total_bytes)?;
	let solidity_name = name.to_string();
	let solidity_fields = fields.map(|f| {
		let name = f.ident.as_ref().to_string();
		let docs = f.docs.clone();
		let (amount_of_bits, zeros_on_left, _, starting_inject_byte) = BitMath::from_field(f)
			.map(|math| math.into_tuple())
			.unwrap_or((0, 0, 0, 0));
		assert!(
			aligned_size * 8 >= (zeros_on_left + amount_of_bits),
			"{aligned_size} {zeros_on_left} {amount_of_bits} {starting_inject_byte}"
		);
		let zeros_on_right = aligned_size * 8 - (zeros_on_left + amount_of_bits);
		if amount_of_bits == 0 {
			quote! {
				SolidityFlagsField::Bool(SolidityFlagsBool {
					docs: &[#(#docs),*],
					name: #name,
					value: 0,
				})
			}
		} else if amount_of_bits == 1 {
			quote! {
				SolidityFlagsField::Bool(SolidityFlagsBool {
					docs: &[#(#docs),*],
					name: #name,
					shift: #zeros_on_right,
				})
			}
		} else {
			quote! {
				SolidityFlagsField::Number(SolidityFlagsNumber {
					docs: &[#(#docs),*],
					name: #name,
					start_bit: #zeros_on_right,
					amount_of_bits: #amount_of_bits,
				})
			}
		}
	});
	Ok(quote! {
		#[cfg(feature = "stubgen")]
		impl ::evm_coder::solidity::SolidityStructTy for #name {
			/// Generate solidity definitions for methods described in this struct
			fn generate_solidity_interface(tc: &evm_coder::solidity::TypeCollector) -> String {
				use evm_coder::solidity::*;
				use core::fmt::Write;
				let interface = SolidityLibrary {
					docs: &[#(#docs),*],
					name: #solidity_name,
					total_bytes: #total_bytes,
					fields: Vec::from([#(
						#solidity_fields,
					)*]),
				};
				let mut out = String::new();
				let _ = interface.format(&mut out);
				tc.collect(out);
				#solidity_name.to_string()
			}
		}
	})
}

pub fn impl_struct_solidity_type_name(name: &syn::Ident) -> TokenStream {
	quote! {
		#[cfg(feature = "stubgen")]
		impl ::evm_coder::solidity::SolidityTypeName for #name {
			fn solidity_name(
				writer: &mut impl ::core::fmt::Write,
				tc: &::evm_coder::solidity::TypeCollector,
			) -> ::core::fmt::Result {
				write!(writer, "{}", tc.collect_struct::<Self>())
			}

			fn is_simple() -> bool {
				false
			}

			fn solidity_default(
				writer: &mut impl ::core::fmt::Write,
				tc: &::evm_coder::solidity::TypeCollector,
			) -> ::core::fmt::Result {
				write!(writer, "{}.wrap(0)", tc.collect_struct::<Self>())
			}
		}
	}
}

pub fn expand_flags(ds: &syn::DataStruct, ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
	let name = &ast.ident;
	let docs = extract_docs(&ast.attrs)?;
	let params_count = match ds.fields {
		syn::Fields::Named(ref fields) => Ok(fields.named.len()),
		syn::Fields::Unnamed(ref fields) => Ok(fields.unnamed.len()),
		syn::Fields::Unit => Err(syn::Error::new(name.span(), "Unit structs not supported")),
	}?;

	if params_count == 0 {
		return Err(syn::Error::new(name.span(), "Empty structs not supported"));
	};

	// parse the input into a StructInfo which contains all the information we
	// along with some helpful structures to generate our Bitfield code.
	let struct_info = match StructInfo::parse(ast) {
		Ok(parsed_struct) => parsed_struct,
		Err(err) => {
			return Ok(err.to_compile_error());
		}
	};

	if struct_info.lsb_zero {
		return Err(syn::Error::new(
			struct_info.name.span(),
			"read_from = 'lsb0' is not supported",
		));
	}

	if let Some(field) = struct_info
		.fields
		.iter()
		.find(|field| field.bit_size() > 8 && *field.attrs.endianness != Endianness::Big)
	{
		return Err(syn::Error::new(
			field.name.span(),
			"only big endian fields are supported",
		));
	}

	let total_bytes = struct_info.total_bytes();
	let abi_type = impl_struct_abi_type(name, total_bytes)?;
	let abi_read = impl_struct_abi_read(name, total_bytes)?;
	let abi_write = impl_struct_abi_write(name, total_bytes)?;
	let solidity_type =
		impl_struct_solidity_type(name, &docs, total_bytes, struct_info.fields.iter())?;
	let solidity_type_name = impl_struct_solidity_type_name(name);
	Ok(quote! {
		#abi_type
		#abi_read
		#abi_write
		#solidity_type
		#solidity_type_name
	})
}
