use inflector::cases;
use proc_macro::TokenStream;
use quote::quote;
use sha3::{Digest, Keccak256};
use syn::{
	parse_macro_input, spanned::Spanned, DeriveInput, Ident, ItemImpl, Pat, Path, PathSegment, Type,
};

mod abi_derive;
mod solidity_interface;
#[cfg(feature = "bondrewd")]
mod structs;
mod to_log;

fn fn_selector_str(input: &str) -> u32 {
	let mut hasher = Keccak256::new();
	hasher.update(input.as_bytes());
	let result = hasher.finalize();

	let mut selector_bytes = [0; 4];
	selector_bytes.copy_from_slice(&result[0..4]);

	u32::from_be_bytes(selector_bytes)
}

/// Returns solidity function selector (first 4 bytes of hash) by its
/// textual representation
///
/// ```ignore
/// use evm_coder_macros::fn_selector;
///
/// assert_eq!(fn_selector!(transfer(address, uint256)), 0xa9059cbb);
/// ```
#[proc_macro]
pub fn fn_selector(input: TokenStream) -> TokenStream {
	let input = input.to_string().replace(' ', "");
	let selector = fn_selector_str(&input);

	(quote! {
		::evm_coder::types::BytesFixed(u32::to_be_bytes(#selector))
	})
	.into()
}

fn event_selector_str(input: &str) -> [u8; 32] {
	let mut hasher = Keccak256::new();
	hasher.update(input.as_bytes());
	let result = hasher.finalize();

	let mut selector_bytes = [0; 32];
	selector_bytes.copy_from_slice(&result[0..32]);
	selector_bytes
}

/// Returns solidity topic (hash) by its textual representation
///
/// ```ignore
/// use evm_coder_macros::event_topic;
///
/// assert_eq!(
///     format!("{:x}", event_topic!(Transfer(address, address, uint256))),
///     "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
/// );
/// ```
#[proc_macro]
pub fn event_topic(stream: TokenStream) -> TokenStream {
	let input = stream.to_string().replace(' ', "");
	let selector_bytes = event_selector_str(&input);

	(quote! {
		::primitive_types::H256([#(
			#selector_bytes,
		)*])
	})
	.into()
}

pub(crate) fn parse_path(ty: &Type) -> syn::Result<&Path> {
	match &ty {
		syn::Type::Path(pat) => {
			if let Some(qself) = &pat.qself {
				return Err(syn::Error::new(qself.ty.span(), "no receiver expected"));
			}
			Ok(&pat.path)
		}
		_ => Err(syn::Error::new(ty.span(), "expected ty to be path")),
	}
}

fn parse_path_segment(path: &Path) -> syn::Result<&PathSegment> {
	if path.segments.len() != 1 {
		return Err(syn::Error::new(
			path.span(),
			"expected path to have only one segment",
		));
	}
	let last_segment = &path.segments.last().unwrap();
	Ok(last_segment)
}

fn parse_ident_from_pat(pat: &Pat) -> syn::Result<&Ident> {
	match pat {
		Pat::Ident(i) => Ok(&i.ident),
		_ => Err(syn::Error::new(pat.span(), "expected pat ident")),
	}
}

fn parse_ident_from_segment(segment: &PathSegment, allow_generics: bool) -> syn::Result<&Ident> {
	if !segment.arguments.is_none() && !allow_generics {
		return Err(syn::Error::new(
			segment.arguments.span(),
			"unexpected generic type",
		));
	}
	Ok(&segment.ident)
}

fn parse_ident_from_path(path: &Path, allow_generics: bool) -> syn::Result<&Ident> {
	let segment = parse_path_segment(path)?;
	parse_ident_from_segment(segment, allow_generics)
}

fn parse_ident_from_type(ty: &Type, allow_generics: bool) -> syn::Result<&Ident> {
	let path = parse_path(ty)?;
	parse_ident_from_path(path, allow_generics)
}

fn pascal_ident_to_call(ident: &Ident) -> Ident {
	let name = format!("{ident}Call");
	Ident::new(&name, ident.span())
}
fn snake_ident_to_pascal(ident: &Ident) -> Ident {
	let name = ident.to_string();
	let name = cases::pascalcase::to_pascal_case(&name);
	Ident::new(&name, ident.span())
}
fn snake_ident_to_screaming(ident: &Ident) -> Ident {
	let name = ident.to_string();
	let name = cases::screamingsnakecase::to_screaming_snake_case(&name);
	Ident::new(&name, ident.span())
}

/// See documentation for this proc-macro reexported in `evm-coder` crate
#[proc_macro_attribute]
pub fn solidity_interface(args: TokenStream, stream: TokenStream) -> TokenStream {
	let args = parse_macro_input!(args as solidity_interface::InterfaceInfo);

	let mut input: ItemImpl = match syn::parse(stream) {
		Ok(t) => t,
		Err(e) => return e.to_compile_error().into(),
	};

	let expanded = match solidity_interface::SolidityInterface::try_from(args, &mut input) {
		Ok(v) => v.expand(),
		Err(e) => e.to_compile_error(),
	};

	(quote! {
		#input

		#expanded
	})
	.into()
}

/// See documentation for this proc-macro reexported in `evm-coder` crate
#[proc_macro_derive(ToLog, attributes(indexed))]
pub fn to_log(value: TokenStream) -> TokenStream {
	let input = parse_macro_input!(value as DeriveInput);

	match to_log::Events::try_from(&input) {
		Ok(e) => e.expand(),
		Err(e) => e.to_compile_error(),
	}
	.into()
}

/// See documentation for this proc-macro reexported in `evm-coder` crate
#[proc_macro_derive(AbiCoder)]
pub fn abi_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let ts = match abi_derive::impl_abi_macro(&ast) {
		Ok(e) => e,
		Err(e) => e.to_compile_error(),
	};
	ts.into()
}

#[cfg(feature = "bondrewd")]
/// See documentation for this proc-macro reexported in `evm-coder` crate
#[proc_macro_derive(AbiCoderFlags, attributes(bondrewd,))]
pub fn abi_flags_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let ast = parse_macro_input!(input as DeriveInput);
	let ts = match abi_derive::impl_abi_flags_macro(&ast) {
		Ok(e) => e,
		Err(e) => e.to_compile_error(),
	};
	ts.into()
}
