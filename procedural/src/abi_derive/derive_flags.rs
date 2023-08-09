use proc_macro2::TokenStream;
use quote::quote;

use super::extract_docs;
use crate::structs::common::{
	get_left_and_mask, get_right_and_mask, BitMath, FieldInfo, StructInfo,
};

const ABI_TYPE_SIZE: usize = 4;

pub fn impl_can_be_placed_in_vec(ident: &syn::Ident) -> TokenStream {
	quote! {
		impl ::evm_coder::sealed::CanBePlacedInVec for #ident {}
	}
}

pub fn impl_struct_abi_type(name: &syn::Ident) -> TokenStream {
	quote! {
		impl ::evm_coder::abi::AbiType for #name {
			const SIGNATURE: ::evm_coder::custom_signature::SignatureUnit = <(u32) as ::evm_coder::abi::AbiType>::SIGNATURE;
			fn is_dynamic() -> bool {
				<(u32) as ::evm_coder::abi::AbiType>::is_dynamic()
			}
			fn size() -> usize {
				<(u32) as ::evm_coder::abi::AbiType>::size()
			}
		}
	}
}

pub fn impl_struct_abi_read(name: &syn::Ident, total_bytes: usize) -> TokenStream {
	let bytes = (0..total_bytes).map(|i| {
		let index = ABI_TYPE_SIZE - i - 1;
		quote! { value[#index] }
	});
	quote!(
		impl ::evm_coder::abi::AbiRead for #name {
			fn abi_read(reader: &mut ::evm_coder::abi::AbiReader) -> ::evm_coder::abi::Result<Self> {
				let value = reader.uint32()?.to_le_bytes();
				Ok(#name::from_bytes([#(#bytes),*]))
			}
		}
	)
}

pub fn impl_struct_abi_write(name: &syn::Ident, total_bytes: usize) -> TokenStream {
	let bytes = (0..ABI_TYPE_SIZE).map(|i| {
		let index = ABI_TYPE_SIZE - i - 1;
		if i < ABI_TYPE_SIZE - total_bytes {
			quote! { 0 }
		} else {
			quote! { bytes[#index] }
		}
	});
	quote!(
		impl ::evm_coder::abi::AbiWrite for #name {
			fn abi_write(&self, writer: &mut ::evm_coder::abi::AbiWriter) {
				let bytes = self.clone().into_bytes();
				let value = u32::from_le_bytes([#(#bytes),*]);
				<(u32) as ::evm_coder::abi::AbiWrite>::abi_write(&(value), writer)
			}
		}
	)
}

pub fn impl_struct_solidity_type<'a>(
	name: &syn::Ident,
	docs: &[String],
	fields: impl Iterator<Item = &'a FieldInfo> + Clone,
) -> TokenStream {
	let solidity_name = name.to_string();
	let solidity_fields = fields.map(|f| {
		let name = f.ident.as_ref().to_string();
		let docs = f.docs.clone();
		let value = apply_le_math_to_mask(f);
		quote! {
			SolidityConstant {
				docs: &[#(#docs),*],
				name: #name,
				value: #value,
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

fn apply_le_math_to_mask(field: &FieldInfo) -> TokenStream {
	let (amount_of_bits, zeros_on_left, available_bits_in_first_byte, ..) =
		if let Ok(math) = BitMath::from_field(field) {
			math.into_tuple()
		} else {
			return quote! { 0 };
		};
	if 8 < (zeros_on_left + amount_of_bits) {
		return quote! { 0 };
	}
	let zeros_on_right = 8 - (zeros_on_left + amount_of_bits);
	// combining the left and right masks will give us a mask that keeps the amount og bytes we
	// have in the position we need them to be in for this byte. we use available_bytes for
	// right mask because param is amount of 1's on the side specified (right), and
	// available_bytes is (8 - zeros_on_left) which is equal to ones_on_right.
	let mask =
		get_right_and_mask(available_bits_in_first_byte) & get_left_and_mask(8 - zeros_on_right);
	quote! { #mask }
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
				write!(writer, "{}(0)", tc.collect_struct::<Self>())
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

	let total_bytes = struct_info.total_bytes();
	let can_be_plcaed_in_vec = impl_can_be_placed_in_vec(name);
	let abi_type = impl_struct_abi_type(name);
	let abi_read = impl_struct_abi_read(name, total_bytes);
	let abi_write = impl_struct_abi_write(name, total_bytes);
	let solidity_type = impl_struct_solidity_type(name, &docs, struct_info.fields.iter());
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
