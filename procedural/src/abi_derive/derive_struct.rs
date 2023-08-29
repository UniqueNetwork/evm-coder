use proc_macro2::TokenStream;
use quote::quote;
use syn::Field;

use super::extract_docs;

pub fn tuple_type<'a>(field_types: impl Iterator<Item = &'a syn::Type> + Clone) -> TokenStream {
	let field_types = field_types.map(|ty| quote!(#ty,));
	quote! {(#(#field_types)*)}
}

pub fn tuple_ref_type<'a>(field_types: impl Iterator<Item = &'a syn::Type> + Clone) -> TokenStream {
	let field_types = field_types.map(|ty| quote!(&#ty,));
	quote! {(#(#field_types)*)}
}

pub fn tuple_data_as_ref(
	is_named_fields: bool,
	field_names: impl Iterator<Item = syn::Ident> + Clone,
) -> TokenStream {
	let field_names = field_names.enumerate().map(|(i, field)| {
		if is_named_fields {
			quote!(&self.#field,)
		} else {
			let field = proc_macro2::Literal::usize_unsuffixed(i);
			quote!(&self.#field,)
		}
	});
	quote! {(#(#field_names)*)}
}

pub fn tuple_names(
	is_named_fields: bool,
	field_names: impl Iterator<Item = syn::Ident> + Clone,
) -> TokenStream {
	let field_names = field_names.enumerate().map(|(i, field)| {
		if is_named_fields {
			quote!(#field,)
		} else {
			let field = proc_macro2::Ident::new(
				format!("field{i}").as_str(),
				proc_macro2::Span::call_site(),
			);
			quote!(#field,)
		}
	});
	quote! {(#(#field_names)*)}
}

pub fn struct_from_tuple(
	name: &syn::Ident,
	is_named_fields: bool,
	field_names: impl Iterator<Item = syn::Ident> + Clone,
) -> TokenStream {
	let field_names = field_names.enumerate().map(|(i, field)| {
		if is_named_fields {
			quote!(#field,)
		} else {
			let field = proc_macro2::Ident::new(
				format!("field{i}").as_str(),
				proc_macro2::Span::call_site(),
			);
			quote!(#field,)
		}
	});

	if is_named_fields {
		quote! {#name {#(#field_names)*}}
	} else {
		quote! {#name (#(#field_names)*)}
	}
}

pub fn map_field_to_name(field: (usize, &syn::Field)) -> syn::Ident {
	if let Some(name) = field.1.ident.as_ref() {
		return name.clone();
	}
	let mut name = "field".to_string();
	name.push_str(field.0.to_string().as_str());
	syn::Ident::new(name.as_str(), proc_macro2::Span::call_site())
}

pub fn map_field_to_type(field: &syn::Field) -> &syn::Type {
	&field.ty
}

pub fn impl_struct_abi_type(name: &syn::Ident, tuple_type: &TokenStream) -> TokenStream {
	quote! {
		impl ::evm_coder::abi::AbiType for #name {
			const SIGNATURE: ::evm_coder::custom_signature::SignatureUnit = <#tuple_type as ::evm_coder::abi::AbiType>::SIGNATURE;
			const IS_DYNAMIC: bool = <#tuple_type as ::evm_coder::abi::AbiType>::IS_DYNAMIC;
			const HEAD_WORDS: u32 = <#tuple_type as ::evm_coder::abi::AbiType>::HEAD_WORDS;
		}
	}
}

pub fn impl_struct_abi_read(
	name: &syn::Ident,
	tuple_type: &TokenStream,
	tuple_names: &TokenStream,
	struct_from_tuple: &TokenStream,
) -> TokenStream {
	quote!(
		impl ::evm_coder::abi::AbiDecode for #name {
			fn dec(reader: &mut ::evm_coder::abi::AbiDecoder) -> ::evm_coder::abi::Result<Self> {
				let #tuple_names = <#tuple_type as ::evm_coder::abi::AbiDecode>::dec(reader)?;
				Ok(#struct_from_tuple)
			}
		}
	)
}

pub fn impl_struct_abi_write(
	name: &syn::Ident,
	_is_named_fields: bool,
	tuple_type: &TokenStream,
	tuple_data: &TokenStream,
) -> TokenStream {
	quote!(
		impl ::evm_coder::abi::AbiEncode for #name {
			fn enc(&self, writer: &mut ::evm_coder::abi::AbiEncoder) {
				<#tuple_type as ::evm_coder::abi::AbiEncode>::enc(&#tuple_data, writer)
			}
		}
	)
}

pub fn impl_struct_solidity_type<'a>(
	name: &syn::Ident,
	docs: &[String],
	fields: impl Iterator<Item = &'a Field> + Clone,
) -> TokenStream {
	let solidity_name = name.to_string();
	let solidity_fields = fields.enumerate().map(|(i, f)| {
		let name = f
			.ident
			.as_ref()
			.map_or_else(|| format!("field_{i}"), ToString::to_string);
		let ty = &f.ty;
		let docs = extract_docs(&f.attrs).expect("TODO: handle bad docs");
		quote! {
			SolidityStructField::<#ty> {
				docs: &[#(#docs),*],
				name: #name,
				ty: ::core::marker::PhantomData,
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
				let interface = SolidityStruct {
					docs: &[#(#docs),*],
					name: #solidity_name,
					fields: (#(
						#solidity_fields,
					)*),
				};
				let mut out = String::new();
				let _ = interface.format(&mut out, tc);
				tc.collect(out);
				#solidity_name.to_string()
			}
		}
	}
}

pub fn impl_struct_solidity_type_name<'a>(
	name: &syn::Ident,
	field_types: impl Iterator<Item = &'a syn::Type> + Clone,
	params_count: usize,
) -> TokenStream {
	let arg_dafaults = field_types.enumerate().map(|(i, ty)| {
		let mut defult_value = quote!(<#ty as ::evm_coder::solidity::SolidityTypeName
			>::solidity_default(writer, tc)?;);
		let last_item = params_count - 1;
		if i != last_item {
			defult_value.extend(quote! {write!(writer, ",")?;});
		}
		defult_value
	});

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
				write!(writer, "{}(", tc.collect_struct::<Self>())?;

				#(#arg_dafaults)*

				write!(writer, ")")
			}
		}
	}
}

pub fn expand_struct(
	ds: &syn::DataStruct,
	ast: &syn::DeriveInput,
) -> syn::Result<proc_macro2::TokenStream> {
	let name = &ast.ident;
	let docs = extract_docs(&ast.attrs)?;
	let (is_named_fields, field_names, field_types, params_count) = match ds.fields {
		syn::Fields::Named(ref fields) => Ok((
			true,
			fields.named.iter().enumerate().map(map_field_to_name),
			fields.named.iter().map(map_field_to_type),
			fields.named.len(),
		)),
		syn::Fields::Unnamed(ref fields) => Ok((
			false,
			fields.unnamed.iter().enumerate().map(map_field_to_name),
			fields.unnamed.iter().map(map_field_to_type),
			fields.unnamed.len(),
		)),
		syn::Fields::Unit => Err(syn::Error::new(name.span(), "Unit structs not supported")),
	}?;

	if params_count == 0 {
		return Err(syn::Error::new(name.span(), "Empty structs not supported"));
	};

	let tuple_type = tuple_type(field_types.clone());
	let tuple_ref_type = tuple_ref_type(field_types.clone());
	let tuple_data = tuple_data_as_ref(is_named_fields, field_names.clone());
	let tuple_names = tuple_names(is_named_fields, field_names.clone());
	let struct_from_tuple = struct_from_tuple(name, is_named_fields, field_names.clone());

	let abi_type = impl_struct_abi_type(name, &tuple_type);
	let abi_read = impl_struct_abi_read(name, &tuple_type, &tuple_names, &struct_from_tuple);
	let abi_write = impl_struct_abi_write(name, is_named_fields, &tuple_ref_type, &tuple_data);
	let solidity_type = impl_struct_solidity_type(name, &docs, ds.fields.iter());
	let solidity_type_name =
		impl_struct_solidity_type_name(name, field_types.clone(), params_count);

	Ok(quote! {
		#abi_type
		#abi_read
		#abi_write
		#solidity_type
		#solidity_type_name
	})
}
