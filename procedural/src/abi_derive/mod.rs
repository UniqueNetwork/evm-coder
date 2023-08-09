mod derive_enum;
#[cfg(feature = "bondrewd")]
mod derive_flags;
mod derive_struct;

use derive_enum::expand_enum;
#[cfg(feature = "bondrewd")]
use derive_flags::expand_flags;
use derive_struct::expand_struct;

pub(crate) fn impl_abi_macro(ast: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
	let name = &ast.ident;
	match &ast.data {
		syn::Data::Struct(ds) => expand_struct(ds, ast),
		syn::Data::Enum(de) => expand_enum(de, ast),
		syn::Data::Union(_) => Err(syn::Error::new(name.span(), "Unions not supported")),
	}
}

#[cfg(feature = "bondrewd")]
pub(crate) fn impl_abi_flags_macro(
	ast: &syn::DeriveInput,
) -> syn::Result<proc_macro2::TokenStream> {
	let name = &ast.ident;
	match &ast.data {
		syn::Data::Struct(ds) => expand_flags(ds, ast),
		_ => Err(syn::Error::new(
			name.span(),
			"Unions and enums aren't supported",
		)),
	}
}

pub fn extract_docs(attrs: &[syn::Attribute]) -> syn::Result<Vec<String>> {
	attrs
		.iter()
		.filter_map(|attr| {
			if let Some(ps) = attr.path.segments.first() {
				if ps.ident == "doc" {
					let meta = match attr.parse_meta() {
						Ok(meta) => meta,
						Err(e) => return Some(Err(e)),
					};
					match meta {
						syn::Meta::NameValue(mnv) => match &mnv.lit {
							syn::Lit::Str(ls) => return Some(Ok(ls.value())),
							_ => unreachable!(),
						},
						_ => unreachable!(),
					}
				}
			}
			None
		})
		.collect()
}
