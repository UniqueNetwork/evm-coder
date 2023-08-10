use proc_macro2::TokenStream;
use quote::quote;

use super::extract_docs;
use crate::structs::common::{BitMath, FieldInfo, StructInfo};

pub fn impl_can_be_placed_in_vec(ident: &syn::Ident) -> TokenStream {
	quote! {
		impl ::evm_coder::sealed::CanBePlacedInVec for #ident {}
	}
}

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

pub fn impl_struct_abi_type(name: &syn::Ident, total_bytes: usize) -> syn::Result<TokenStream> {
	let sub_type = match total_bytes {
		1 => quote! { u8 },
		2..=4 => quote! { u32 },
		5..=8 => quote! { u64 },
		_ => {
			return Err(syn::Error::new(
				name.span(),
				format!("Unsupported struct size: {total_bytes}"),
			))
		}
	};

	Ok(quote! {
		impl ::evm_coder::abi::AbiType for #name {
			const SIGNATURE: ::evm_coder::custom_signature::SignatureUnit = <(#sub_type) as ::evm_coder::abi::AbiType>::SIGNATURE;
			fn is_dynamic() -> bool {
				<(#sub_type) as ::evm_coder::abi::AbiType>::is_dynamic()
			}
			fn size() -> usize {
				<(#sub_type) as ::evm_coder::abi::AbiType>::size()
			}
		}
	})
}

pub fn impl_struct_abi_read(name: &syn::Ident, total_bytes: usize) -> syn::Result<TokenStream> {
	let aligned_size = align_size(name, total_bytes)?;
	let bytes = (0..total_bytes).map(|i| {
		let index = total_bytes - i - 1;
		quote! { value[#index] }
	});
	Ok(quote!(
		impl ::evm_coder::abi::AbiRead for #name {
			fn abi_read(reader: &mut ::evm_coder::abi::AbiReader) -> ::evm_coder::abi::Result<Self> {
				let value = reader.bytes_padleft::<#aligned_size>()?;
				Ok(#name::from_bytes([#(#bytes),*]))
			}
		}
	))
}

pub fn impl_struct_abi_write(name: &syn::Ident, total_bytes: usize) -> syn::Result<TokenStream> {
	let aligned_size = align_size(name, total_bytes)?;
	let bytes = (0..aligned_size).map(|i| {
		if total_bytes < 1 + i {
			quote! { 0 }
		} else {
			let index = total_bytes - 1 - i;
			quote! { value[#index] }
		}
	});
	Ok(quote!(
		impl ::evm_coder::abi::AbiWrite for #name {
			fn abi_write(&self, writer: &mut ::evm_coder::abi::AbiWriter) {
				let value = self.clone().into_bytes();
				writer.bytes_padleft(&[#(#bytes),*]);
			}
		}
	))
}

pub fn impl_struct_solidity_type<'a>(
	name: &syn::Ident,
	docs: &[String],
	total_bytes: usize,
	fields: impl Iterator<Item = &'a FieldInfo> + Clone,
) -> TokenStream {
	let solidity_name = name.to_string();
	let solidity_fields = fields.map(|f| {
		let name = f.ident.as_ref().to_string();
		let docs = f.docs.clone();
		let (amount_of_bits, zeros_on_left, _, _) = BitMath::from_field(f)
			.map(|math| math.into_tuple())
			.unwrap_or((0, 0, 0, 0));
		let zeros_on_right = 8 - (zeros_on_left + amount_of_bits);
		if amount_of_bits == 0 {
			quote! {
				SolidityFlagsField::Bool(SolidityFlagsBool {
					docs: &[#(#docs),*],
					name: #name,
					value: 0,
				})
			}
		} else if amount_of_bits == 1 {
			let value: u8 = 1 << zeros_on_right;
			quote! {
				SolidityFlagsField::Bool(SolidityFlagsBool {
					docs: &[#(#docs),*],
					name: #name,
					value: #value,
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
	quote! {
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
	}
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

	let total_bytes = struct_info.total_bytes();
	let can_be_plcaed_in_vec = impl_can_be_placed_in_vec(name);
	let abi_type = impl_struct_abi_type(name, total_bytes)?;
	let abi_read = impl_struct_abi_read(name, total_bytes)?;
	let abi_write = impl_struct_abi_write(name, total_bytes)?;
	let solidity_type =
		impl_struct_solidity_type(name, &docs, total_bytes, struct_info.fields.iter());
	let solidity_type_name = impl_struct_solidity_type_name(name);
	Ok(quote! {
		#can_be_plcaed_in_vec
		#abi_type
		#abi_read
		#abi_write
		#solidity_type
		#solidity_type_name
	})
}
