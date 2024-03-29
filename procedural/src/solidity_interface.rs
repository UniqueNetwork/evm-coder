// NOTE: In order to understand this Rust macro better, first read this chapter
// about Procedural Macros in Rust book:
// https://doc.rust-lang.org/reference/procedural-macros.html

use std::collections::BTreeSet;

use inflector::cases;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
	parenthesized,
	parse::{Parse, ParseStream},
	spanned::Spanned,
	Expr, FnArg, Generics, Ident, ImplItem, ImplItemMethod, Index, ItemImpl, Lit, LitInt, LitStr,
	Meta, MetaNameValue, PatType, Path, PathArguments, ReturnType, Token, Type,
};

use crate::{
	parse_ident_from_pat, parse_ident_from_path, parse_path, parse_path_segment,
	pascal_ident_to_call, snake_ident_to_pascal, snake_ident_to_screaming,
};

struct Is {
	name: Ident,
	pascal_call_name: Ident,
	via: Option<(Type, Ident)>,
	condition: Option<Expr>,
}
impl Is {
	fn expand_call_def(&self, gen_ref: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
		let name = &self.name;
		let pascal_call_name = &self.pascal_call_name;
		quote! {
			#name(#pascal_call_name #gen_ref)
		}
	}

	fn expand_interface_id(&self) -> proc_macro2::TokenStream {
		let pascal_call_name = &self.pascal_call_name;
		quote! {
			interface_id ^= u32::from_be_bytes(#pascal_call_name::interface_id().0);
		}
	}

	fn expand_supports_interface(
		&self,
		generics: &proc_macro2::TokenStream,
	) -> proc_macro2::TokenStream {
		let pascal_call_name = &self.pascal_call_name;
		let condition = self.condition.as_ref().map(|condition| {
			quote! {
				(#condition) &&
			}
		});
		quote! {
			#condition <#pascal_call_name #generics>::supports_interface(this, interface_id)
		}
	}

	fn expand_variant_call(
		&self,
		call_name: &proc_macro2::Ident,
		generics: &proc_macro2::TokenStream,
	) -> proc_macro2::TokenStream {
		let name = &self.name;
		let pascal_call_name = &self.pascal_call_name;
		let via_typ = self
			.via
			.as_ref()
			.map_or_else(|| quote! {Self}, |(t, _)| quote! {#t});
		let via_map = self
			.via
			.as_ref()
			.map(|(_, i)| quote! {.#i()})
			.unwrap_or_default();
		let condition = self.condition.as_ref().map(|condition| {
			quote! {
				if ({let this = &self; (#condition)})
			}
		});
		quote! {
			#call_name::#name(call) #condition => return <#via_typ as ::evm_coder::Callable<#pascal_call_name #generics>>::call(self #via_map, ::evm_coder::types::Msg {
				call,
				caller: c.caller,
				value: c.value,
			})
		}
	}

	fn expand_parse(&self, generics: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
		let name = &self.name;
		let pascal_call_name = &self.pascal_call_name;
		quote! {
			if let Some(parsed_call) = <#pascal_call_name #generics>::parse(method_id, reader)? {
				return Ok(Some(Self::#name(parsed_call)))
			}
		}
	}

	fn expand_generator(&self, generics: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
		let pascal_call_name = &self.pascal_call_name;
		quote! {
			<#pascal_call_name #generics>::generate_solidity_interface(tc, is_impl);
		}
	}

	fn expand_event_generator(&self) -> proc_macro2::TokenStream {
		let name = &self.name;
		quote! {
			#name::generate_solidity_interface(tc, is_impl);
		}
	}
}

#[derive(Default)]
struct IsList(Vec<Is>);
impl Parse for IsList {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut out = vec![];
		loop {
			if input.is_empty() {
				break;
			}
			let name = input.parse::<Ident>()?;
			let lookahead = input.lookahead1();

			let mut condition: Option<Expr> = None;
			let mut via: Option<(Type, Ident)> = None;

			if lookahead.peek(syn::token::Paren) {
				let contents;
				parenthesized!(contents in input);
				let input = contents;

				while !input.is_empty() {
					let lookahead = input.lookahead1();
					if lookahead.peek(Token![if]) {
						input.parse::<Token![if]>()?;
						let contents;
						parenthesized!(contents in input);
						let contents = contents.parse::<Expr>()?;

						if condition.replace(contents).is_some() {
							return Err(syn::Error::new(input.span(), "condition is already set"));
						}
					} else if lookahead.peek(kw::via) {
						input.parse::<kw::via>()?;
						let contents;
						parenthesized!(contents in input);

						let method = contents.parse::<Ident>()?;
						contents.parse::<kw::returns>()?;
						let ty = contents.parse::<Type>()?;

						if via.replace((ty, method)).is_some() {
							return Err(syn::Error::new(input.span(), "via is already set"));
						}
					} else {
						return Err(lookahead.error());
					}

					if input.peek(Token![,]) {
						input.parse::<Token![,]>()?;
					} else if !input.is_empty() {
						return Err(syn::Error::new(input.span(), "expected end"));
					}
				}
			} else if lookahead.peek(Token![,]) || input.is_empty() {
				// Pass
			} else {
				return Err(lookahead.error());
			};
			out.push(Is {
				pascal_call_name: pascal_ident_to_call(&name),
				name,
				via,
				condition,
			});
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
				continue;
			}
			break;
		}
		Ok(Self(out))
	}
}

pub struct InterfaceInfo {
	name: Ident,
	is: IsList,
	inline_is: IsList,
	events: IsList,
	expect_selector: Option<u32>,
	enum_attrs: Vec<TokenStream>,
	enum_variant_attrs: BTreeSet<Ident>,
}
impl Parse for InterfaceInfo {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut name = None;
		let mut is = None;
		let mut inline_is = None;
		let mut events = None;
		let mut expect_selector = None;
		let mut enum_attrs = Vec::new();
		let mut enum_variant_attrs = BTreeSet::new();
		// TODO: create proc-macro to optimize proc-macro boilerplate? :D
		loop {
			let lookahead = input.lookahead1();
			if lookahead.peek(kw::name) {
				let k = input.parse::<kw::name>()?;
				input.parse::<Token![=]>()?;
				if name.replace(input.parse::<Ident>()?).is_some() {
					return Err(syn::Error::new(k.span(), "name is already set"));
				}
			} else if lookahead.peek(kw::is) {
				let k = input.parse::<kw::is>()?;
				let contents;
				parenthesized!(contents in input);
				if is.replace(contents.parse::<IsList>()?).is_some() {
					return Err(syn::Error::new(k.span(), "is is already set"));
				}
			} else if lookahead.peek(kw::inline_is) {
				let k = input.parse::<kw::inline_is>()?;
				let contents;
				parenthesized!(contents in input);
				if inline_is.replace(contents.parse::<IsList>()?).is_some() {
					return Err(syn::Error::new(k.span(), "inline_is is already set"));
				}
			} else if lookahead.peek(kw::events) {
				let k = input.parse::<kw::events>()?;
				let contents;
				parenthesized!(contents in input);
				if events.replace(contents.parse::<IsList>()?).is_some() {
					return Err(syn::Error::new(k.span(), "events is already set"));
				}
			} else if lookahead.peek(kw::expect_selector) {
				let k = input.parse::<kw::expect_selector>()?;
				input.parse::<Token![=]>()?;
				let value = input.parse::<LitInt>()?;
				if expect_selector
					.replace(value.base10_parse::<u32>()?)
					.is_some()
				{
					return Err(syn::Error::new(k.span(), "expect_selector is already set"));
				}
			} else if lookahead.peek(Token![enum]) {
				input.parse::<Token![enum]>()?;
				let contents;
				parenthesized!(contents in input);
				enum_attrs.push(contents.parse()?);
			} else if lookahead.peek(kw::enum_attr) {
				input.parse::<kw::enum_attr>()?;
				let contents;
				parenthesized!(contents in input);
				enum_variant_attrs.insert(contents.parse()?);
			} else if input.is_empty() {
				break;
			} else {
				return Err(lookahead.error());
			}
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			} else {
				break;
			}
		}
		Ok(Self {
			name: name.ok_or_else(|| syn::Error::new(input.span(), "missing name"))?,
			is: is.unwrap_or_default(),
			inline_is: inline_is.unwrap_or_default(),
			events: events.unwrap_or_default(),
			expect_selector,
			enum_attrs,
			enum_variant_attrs,
		})
	}
}

struct MethodInfo {
	rename_selector: Option<String>,
	hide: bool,
	enum_attrs: Vec<TokenStream>,
}
impl Parse for MethodInfo {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut rename_selector = None;
		let mut hide = false;
		let mut enum_attrs = Vec::new();
		while !input.is_empty() {
			let lookahead = input.lookahead1();
			if lookahead.peek(kw::rename_selector) {
				let k = input.parse::<kw::rename_selector>()?;
				input.parse::<Token![=]>()?;
				if rename_selector
					.replace(input.parse::<LitStr>()?.value())
					.is_some()
				{
					return Err(syn::Error::new(k.span(), "rename_selector is already set"));
				}
			} else if lookahead.peek(kw::hide) {
				input.parse::<kw::hide>()?;
				hide = true;
			} else if lookahead.peek(Token![enum]) {
				input.parse::<Token![enum]>()?;
				let contents;
				parenthesized!(contents in input);
				enum_attrs.push(contents.parse()?);
			} else {
				return Err(lookahead.error());
			}

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			} else if !input.is_empty() {
				return Err(syn::Error::new(input.span(), "expected end"));
			}
		}
		Ok(Self {
			rename_selector,
			hide,
			enum_attrs,
		})
	}
}

trait AbiTypeHelper {
	fn plain(&self) -> syn::Result<&Ident>;
	fn is_value(&self) -> bool;
	fn is_caller(&self) -> bool;
	fn is_special(&self) -> bool;
}

impl AbiTypeHelper for Type {
	fn plain(&self) -> syn::Result<&Ident> {
		let path = parse_path(self)?;
		let segment = parse_path_segment(path)?;
		if !segment.arguments.is_empty() {
			return Err(syn::Error::new(self.span(), "Not plain type"));
		}
		Ok(&segment.ident)
	}

	fn is_value(&self) -> bool {
		if let Ok(ident) = self.plain() {
			return ident == "Value";
		}
		false
	}

	fn is_caller(&self) -> bool {
		if let Ok(ident) = self.plain() {
			return ident == "Caller";
		}
		false
	}

	fn is_special(&self) -> bool {
		self.is_caller() || self.is_value()
	}
}

struct MethodArg {
	name: Ident,
	camel_name: String,
	ty: Type,
}
impl MethodArg {
	fn try_from(value: &PatType) -> syn::Result<Self> {
		let name = parse_ident_from_pat(&value.pat)?.clone();
		Ok(Self {
			camel_name: cases::camelcase::to_camel_case(&name.to_string()),
			name,
			ty: value.ty.as_ref().clone(),
		})
	}
	fn is_value(&self) -> bool {
		self.ty.is_value()
	}
	fn is_caller(&self) -> bool {
		self.ty.is_caller()
	}
	fn is_special(&self) -> bool {
		self.ty.is_special()
	}

	fn expand_call_def(&self) -> proc_macro2::TokenStream {
		assert!(!self.is_special());
		let name = &self.name;
		let ty = &self.ty;

		quote! {
			#name: #ty
		}
	}

	fn expand_extract_parsed(&self, i: usize) -> proc_macro2::TokenStream {
		assert!(!self.is_special());
		let name = &self.name;
		let i = Index::from(i);
		quote!(#name: parsed.#i)
	}

	fn expand_call_arg(&self) -> proc_macro2::TokenStream {
		if self.is_value() {
			quote! {
				c.value.clone()
			}
		} else if self.is_caller() {
			quote! {
				c.caller.clone()
			}
		} else {
			let name = &self.name;
			quote! {
				#name
			}
		}
	}

	fn expand_solidity_argument(&self) -> proc_macro2::TokenStream {
		let camel_name = &self.camel_name.to_string();
		let ty = &self.ty;
		quote! {
			<NamedArgument<#ty>>::new(#camel_name)
		}
	}
}

#[derive(PartialEq)]
enum Mutability {
	Mutable,
	View,
	Pure,
}

/// Group all keywords for this macro. Usage example:
/// `#[solidity_interface(name = "B", inline_is(A))]`
mod kw {
	syn::custom_keyword!(via);
	syn::custom_keyword!(returns);
	syn::custom_keyword!(name);
	syn::custom_keyword!(is);
	syn::custom_keyword!(inline_is);
	syn::custom_keyword!(events);
	syn::custom_keyword!(expect_selector);
	syn::custom_keyword!(enum_attr);

	syn::custom_keyword!(rename_selector);
	syn::custom_keyword!(hide);
}

/// Rust methods are parsed into this structure when Solidity code is generated
struct Method {
	name: Ident,
	camel_name: String,
	pascal_name: Ident,
	screaming_name: Ident,
	hide: bool,
	args: Vec<MethodArg>,
	has_normal_args: bool,
	has_value_args: bool,
	mutability: Mutability,
	result: Box<Type>,
	docs: Vec<String>,
	enum_attrs: Vec<TokenStream>,
}
impl Method {
	fn try_from(value: &mut ImplItemMethod, variant_attrs: &BTreeSet<Ident>) -> syn::Result<Self> {
		let mut info = MethodInfo {
			rename_selector: None,
			hide: false,
			enum_attrs: Vec::new(),
		};
		let mut docs = Vec::new();

		let mut to_remove = Vec::new();
		let mut extra_enum_attrs = Vec::new();
		for (i, attr) in value.attrs.iter().enumerate() {
			let ident = parse_ident_from_path(&attr.path, false)?;
			if ident == "solidity" {
				info = attr.parse_args::<MethodInfo>()?;
				to_remove.push(i);
			} else if ident == "doc" {
				let args = attr.parse_meta().unwrap();
				let value = match args {
					Meta::NameValue(MetaNameValue {
						lit: Lit::Str(str), ..
					}) => str.value(),
					_ => unreachable!(),
				};
				docs.push(value);
			} else if variant_attrs.contains(ident) {
				let path = &attr.path;
				let tokens = &attr.tokens;
				extra_enum_attrs.push(quote! {#path #tokens});
				to_remove.push(i);
			}
		}
		for i in to_remove.iter().rev() {
			value.attrs.remove(*i);
		}

		let ident = &value.sig.ident;
		let ident_str = ident.to_string();
		if !cases::snakecase::is_snake_case(&ident_str) {
			return Err(syn::Error::new(ident.span(), "method name should be snake_cased\nif alternative solidity name needs to be set - use #[solidity] attribute"));
		}

		let mut mutability = Mutability::Pure;

		if let Some(FnArg::Receiver(receiver)) = value
			.sig
			.inputs
			.iter()
			.find(|arg| matches!(arg, FnArg::Receiver(_)))
		{
			if receiver.reference.is_none() {
				return Err(syn::Error::new(
					receiver.span(),
					"receiver should be by ref",
				));
			}
			if receiver.mutability.is_some() {
				mutability = Mutability::Mutable;
			} else {
				mutability = Mutability::View;
			}
		}
		let mut args = Vec::new();
		for typ in value
			.sig
			.inputs
			.iter()
			.filter(|arg| matches!(arg, FnArg::Typed(_)))
		{
			let typ = match typ {
				FnArg::Typed(typ) => typ,
				FnArg::Receiver(_) => unreachable!(),
			};
			args.push(MethodArg::try_from(typ)?);
		}

		if mutability != Mutability::Mutable && args.iter().any(MethodArg::is_value) {
			return Err(syn::Error::new(
				args.iter().find(|arg| arg.is_value()).unwrap().ty.span(),
				"payable function should be mutable",
			));
		}

		let result = match &value.sig.output {
			ReturnType::Type(_, ty) => ty,
			ReturnType::Default => return Err(syn::Error::new(value.sig.output.span(), "interface method should return Result<value>\nif there is no value to return - specify void (which is alias to unit)")),
		};

		let camel_name = info
			.rename_selector
			.unwrap_or_else(|| cases::camelcase::to_camel_case(&ident.to_string()));
		let has_normal_args = args.iter().filter(|arg| !arg.is_special()).count() != 0;
		let has_value_args = args.iter().any(MethodArg::is_value);

		Ok(Self {
			name: ident.clone(),
			camel_name,
			pascal_name: snake_ident_to_pascal(ident),
			screaming_name: snake_ident_to_screaming(ident),
			hide: info.hide,
			args,
			has_normal_args,
			has_value_args,
			mutability,
			result: result.clone(),
			docs,
			enum_attrs: [info.enum_attrs, extra_enum_attrs].concat(),
		})
	}
	fn expand_call_def(&self) -> proc_macro2::TokenStream {
		let defs = self
			.args
			.iter()
			.filter(|a| !a.is_special())
			.map(MethodArg::expand_call_def);
		let pascal_name = &self.pascal_name;
		let docs = &self.docs;
		let enum_attrs = &self.enum_attrs;

		if self.has_normal_args {
			quote! {
				#(#[doc = #docs])*
				#(#[#enum_attrs])*
				#[allow(missing_docs)]
				#pascal_name {
					#(
						#defs,
					)*
				}
			}
		} else {
			quote! {
				#(#[doc = #docs])*
				#(#[#enum_attrs])*
				#[allow(missing_docs)]
				#pascal_name
			}
		}
	}

	fn expand_const(&self) -> proc_macro2::TokenStream {
		let screaming_name = &self.screaming_name;
		let screaming_name_signature = format_ident!("{}_SIGNATURE", &self.screaming_name);
		let custom_signature = self.expand_custom_signature();
		quote! {
			const #screaming_name_signature: ::evm_coder::custom_signature::SignatureUnit = #custom_signature;
			const #screaming_name: ::evm_coder::types::Bytes4 = {
				let mut sum = ::evm_coder::sha3_const::Keccak256::new();
				let mut pos = 0;
				while pos < Self::#screaming_name_signature.len {
					sum = sum.update(&[Self::#screaming_name_signature.data[pos]; 1]);
					pos += 1;
				}
				let a = sum.finalize();
				::evm_coder::types::BytesFixed([a[0], a[1], a[2], a[3]])
			};
		}
	}

	fn expand_interface_id(&self) -> proc_macro2::TokenStream {
		let screaming_name = &self.screaming_name;
		quote! {
			interface_id ^= u32::from_be_bytes(Self::#screaming_name.0);
		}
	}

	fn expand_parse(&self) -> proc_macro2::TokenStream {
		let pascal_name = &self.pascal_name;
		let screaming_name = &self.screaming_name;
		if self.has_normal_args {
			let args_iter = self.args.iter().filter(|a| !a.is_special());
			let arg_type = args_iter.clone().map(|a| &a.ty);
			let parsers = args_iter
				.enumerate()
				.map(|(i, m)| m.expand_extract_parsed(i));
			quote! {
				Self::#screaming_name => {
					let parsed = <(#(#arg_type,)*) as ::evm_coder::abi::AbiDecode>::abi_decode(reader)?;
					return Ok(Some(Self::#pascal_name {
						#(
							#parsers,
						)*
					}))
				}
			}
		} else {
			quote! { Self::#screaming_name => return Ok(Some(Self::#pascal_name)) }
		}
	}

	fn expand_variant_call(
		&self,
		result_macro_name: &Path,
		call_name: &proc_macro2::Ident,
	) -> proc_macro2::TokenStream {
		let pascal_name = &self.pascal_name;
		let name = &self.name;

		let matcher = if self.has_normal_args {
			let names = self
				.args
				.iter()
				.filter(|a| !a.is_special())
				.map(|a| &a.name);

			quote! {{
				#(
					#names,
				)*
			}}
		} else {
			quote! {}
		};

		let receiver = match self.mutability {
			Mutability::Mutable | Mutability::View => quote! {self.},
			Mutability::Pure => quote! {Self::},
		};
		let args = self.args.iter().map(MethodArg::expand_call_arg);

		quote! {
			#call_name::#pascal_name #matcher => {
				#[allow(deprecated)]
				let result = #receiver #name(
					#(
						#args,
					)*
				);
				let result = #result_macro_name!(result);
				result.map(|post| {
					<Self as ::evm_coder::Contract>::map_post(post, |res| {
						(res,).abi_encode()
					})
				})
			}
		}
	}

	fn expand_custom_signature(&self) -> proc_macro2::TokenStream {
		let mut args = TokenStream::new();

		let mut has_params = false;
		for arg in self.args.iter().filter(|a| !a.is_special()) {
			has_params = true;
			let ty = &arg.ty;
			args.extend(quote! {nameof(<#ty>::SIGNATURE)});
			args.extend(quote! {fixed(",")});
		}

		// Remove trailing comma
		if has_params {
			args.extend(quote! {shift_left(1)});
		}

		let func_name = self.camel_name.clone();
		quote! { ::evm_coder::make_signature!(new fixed(#func_name) fixed("(") #args fixed(")")) }
	}

	fn expand_solidity_function(&self) -> proc_macro2::TokenStream {
		let camel_name = &self.camel_name;
		let mutability = match self.mutability {
			Mutability::Mutable => quote! {SolidityMutability::Mutable},
			Mutability::View => quote! { SolidityMutability::View },
			Mutability::Pure => quote! {SolidityMutability::Pure},
		};
		let result = &self.result;

		let args = self
			.args
			.iter()
			.filter(|a| !a.is_special())
			.map(MethodArg::expand_solidity_argument);
		let docs = &self.docs;
		let screaming_name = &self.screaming_name;
		let hide = self.hide;
		let custom_signature = self.expand_custom_signature();
		let is_payable = self.has_value_args;

		quote! {
			SolidityFunction {
				docs: &[#(#docs),*],
				hide: #hide,
				selector: u32::from_be_bytes(Self::#screaming_name.0),
				custom_signature: #custom_signature,
				name: #camel_name,
				mutability: #mutability,
				is_payable: #is_payable,
				args: (
					#(
						#args,
					)*
				),
				result: <UnnamedArgument<#result>>::default(),
			}
		}
	}
}

fn generics_list(gen: &Generics) -> proc_macro2::TokenStream {
	if gen.params.is_empty() {
		return quote! {};
	}
	let params = gen.params.iter().map(|p| match p {
		syn::GenericParam::Type(id) => {
			let v = &id.ident;
			quote! {#v}
		}
		syn::GenericParam::Lifetime(lt) => {
			let v = &lt.lifetime;
			quote! {#v}
		}
		syn::GenericParam::Const(c) => {
			let i = &c.ident;
			quote! {#i}
		}
	});
	quote! { #(#params),* }
}
fn generics_reference(gen: &Generics) -> proc_macro2::TokenStream {
	if gen.params.is_empty() {
		return quote! {};
	}
	let list = generics_list(gen);
	quote! { <#list> }
}
fn generics_stub(gen: &Generics) -> proc_macro2::TokenStream {
	if gen.params.is_empty() {
		return quote! {};
	}
	let params = (0..gen.params.len()).map(|_| quote! {()});
	quote! {<#(#params,)*>}
}
fn generics_data(gen: &Generics) -> proc_macro2::TokenStream {
	let list = generics_list(gen);
	if gen.params.len() == 1 {
		quote! {#list}
	} else {
		quote! { (#list) }
	}
}

pub struct SolidityInterface {
	generics: Generics,
	name: Box<syn::Type>,
	result_macro_name: Path,
	info: InterfaceInfo,
	methods: Vec<Method>,
	docs: Vec<String>,
}
impl SolidityInterface {
	pub fn try_from(info: InterfaceInfo, value: &mut ItemImpl) -> syn::Result<Self> {
		let mut methods = Vec::new();

		for item in &mut value.items {
			if let ImplItem::Method(method) = item {
				methods.push(Method::try_from(method, &info.enum_variant_attrs)?);
			}
		}
		let mut docs = vec![];
		for attr in &value.attrs {
			let ident = parse_ident_from_path(&attr.path, false)?;
			if ident == "doc" {
				let args = attr.parse_meta().unwrap();
				let value = match args {
					Meta::NameValue(MetaNameValue {
						lit: Lit::Str(str), ..
					}) => str.value(),
					_ => unreachable!(),
				};
				docs.push(value);
			}
		}
		let mut result_macro_name = parse_path(&value.self_ty)?.clone();
		if let Some(last) = result_macro_name.segments.iter_mut().last() {
			last.ident = format_ident!("{}_result", &last.ident);
			last.arguments = PathArguments::None;
		}

		Ok(Self {
			generics: value.generics.clone(),
			name: value.self_ty.clone(),
			result_macro_name,
			info,
			methods,
			docs,
		})
	}

	#[allow(clippy::too_many_lines)]
	pub fn expand(self) -> proc_macro2::TokenStream {
		let name = self.name;

		let solidity_name = self.info.name.to_string();
		let call_name = pascal_ident_to_call(&self.info.name);
		let generics = self.generics;
		let gen_ref = generics_reference(&generics);
		let gen_data = generics_data(&generics);
		let gen_stub = generics_stub(&generics);
		let gen_where = &generics.where_clause;

		let call_sub = self
			.info
			.inline_is
			.0
			.iter()
			.chain(self.info.is.0.iter())
			.map(|c| Is::expand_call_def(c, &gen_ref));
		let call_parse = self
			.info
			.inline_is
			.0
			.iter()
			.chain(self.info.is.0.iter())
			.map(|is| Is::expand_parse(is, &gen_ref));
		let call_variants = self
			.info
			.inline_is
			.0
			.iter()
			.chain(self.info.is.0.iter())
			.map(|c| Is::expand_variant_call(c, &call_name, &gen_ref));

		let inline_interface_id = self.info.inline_is.0.iter().map(Is::expand_interface_id);
		let supports_interface = self
			.info
			.is
			.0
			.iter()
			.map(|is| Is::expand_supports_interface(is, &gen_ref));

		let calls = self.methods.iter().map(Method::expand_call_def);
		let consts = self.methods.iter().map(Method::expand_const);
		let interface_id = self.methods.iter().map(Method::expand_interface_id);
		let parsers = self.methods.iter().map(Method::expand_parse);
		let call_variants_this = self
			.methods
			.iter()
			.map(|m| Method::expand_variant_call(m, &self.result_macro_name, &call_name));
		let solidity_functions = self.methods.iter().map(Method::expand_solidity_function);

		// TODO: Inline inline_is
		let solidity_is = self
			.info
			.is
			.0
			.iter()
			.chain(self.info.inline_is.0.iter())
			.map(|is| is.name.to_string());
		let solidity_events_is = self.info.events.0.iter().map(|is| is.name.to_string());
		let solidity_generators = self
			.info
			.is
			.0
			.iter()
			.chain(self.info.inline_is.0.iter())
			.map(|is| Is::expand_generator(is, &gen_ref));
		let solidity_event_generators = self.info.events.0.iter().map(Is::expand_event_generator);
		let solidity_events_idents = self.info.events.0.iter().map(|is| is.name.clone());
		let docs = &self.docs;
		let enum_attrs = &self.info.enum_attrs;

		let expect_selector = self.info.expect_selector.map(|s| {
            quote! {
                const _: () = assert!(#s == u32::from_be_bytes(<#call_name #gen_stub>::interface_id().0), "selector mismatch, review contained function selectors");
            }
        });

		quote! {
			#(
				const _: ::core::marker::PhantomData<#solidity_events_idents> = ::core::marker::PhantomData;
			)*
			#[derive(Debug)]
			#(#[doc = #docs])*
			#(#[#enum_attrs])*
			pub enum #call_name #gen_ref {
				/// Inherited method
				ERC165Call(::evm_coder::ERC165Call, ::core::marker::PhantomData<#gen_data>),
				#(
					#calls,
				)*
				#(
					#call_sub,
				)*
			}

			#expect_selector

			impl #gen_ref #call_name #gen_ref {
				#(
					#consts
				)*
				/// Return this call ERC165 selector
				pub const fn interface_id() -> ::evm_coder::types::Bytes4 {
					let mut interface_id = 0;
					#(#interface_id)*
					#(#inline_interface_id)*
					::evm_coder::types::BytesFixed(u32::to_be_bytes(interface_id))
				}
				/// Generate solidity definitions for methods described in this interface
				#[cfg(feature = "stubgen")]
				pub fn generate_solidity_interface(tc: &evm_coder::solidity::TypeCollector, is_impl: bool) {
					use evm_coder::solidity::*;
					use core::fmt::Write;
					let interface = SolidityInterface {
						docs: &[#(#docs),*],
						name: #solidity_name,
						selector: Self::interface_id(),
						is: &["Dummy", "ERC165", #(
							#solidity_is,
						)* #(
							#solidity_events_is,
						)* ],
						functions: (#(
							#solidity_functions,
						)*),
					};

					let mut out = ::evm_coder::types::String::new();
					if #solidity_name.starts_with("Inline") {
						out.push_str("/// @dev inlined interface\n");
					}
					let _ = interface.format(is_impl, &mut out, tc);
					tc.collect(out);
					#(
						#solidity_event_generators
					)*
					#(
						#solidity_generators
					)*
					if is_impl {
						tc.collect("/// @dev common stubs holder\ncontract Dummy {\n\tuint8 dummy;\n\tstring stub_error = \"this contract is implemented in native\";\n}\ncontract ERC165 is Dummy {\n\tfunction supportsInterface(bytes4 interfaceID) external view returns (bool) {\n\t\trequire(false, stub_error);\n\t\tinterfaceID;\n\t\treturn true;\n\t}\n}\n".into());
					} else {
						tc.collect("/// @dev common stubs holder\ninterface Dummy {\n}\ninterface ERC165 is Dummy {\n\tfunction supportsInterface(bytes4 interfaceID) external view returns (bool);\n}\n".into());
					}
				}
			}
			impl #gen_ref ::evm_coder::Call for #call_name #gen_ref {
				fn parse(method_id: ::evm_coder::types::Bytes4, reader: &[u8]) -> ::evm_coder::abi::Result<Option<Self>> {
					match method_id {
						::evm_coder::ERC165Call::INTERFACE_ID => return Ok(
							::evm_coder::ERC165Call::parse(method_id, reader)?
							.map(|c| Self::ERC165Call(c, ::core::marker::PhantomData))
						),
						#(
							#parsers,
						)*
						_ => {},
					}
					#(
						#call_parse
					)else*
					return Ok(None);
				}
			}
			impl #generics #call_name #gen_ref
			#gen_where
			{
				/// Is this contract implements specified ERC165 selector
				pub fn supports_interface(this: &#name, interface_id: ::evm_coder::types::Bytes4) -> bool {
					interface_id.0 != u32::to_be_bytes(0xffffff) && (
						interface_id == ::evm_coder::ERC165Call::INTERFACE_ID ||
						interface_id == Self::interface_id()
						#(
							|| #supports_interface
						)*
					)
				}
			}
			impl #generics ::evm_coder::Callable<#call_name #gen_ref> for #name
			#gen_where
			{
				#[allow(unreachable_code)] // In case of no inner calls
				fn call(&mut self, c: ::evm_coder::types::Msg<#call_name #gen_ref>) -> ::evm_coder::ResultWithPostInfoOf<Self, ::evm_coder::types::Vec<u8>> {
					use ::evm_coder::abi::AbiEncode;
					match c.call {
						#(
							#call_variants,
						)*
						#call_name::ERC165Call(::evm_coder::ERC165Call::SupportsInterface {interface_id}, _) => {
							let data = <#call_name #gen_ref>::supports_interface(self, interface_id).abi_encode();
							return Ok(<Self as ::evm_coder::Contract>::with_default_post(data));
						}
						_ => {},
					}
					match c.call {
						#(
							#call_variants_this,
						)*
						_ => Err(<Self as ::evm_coder::Contract>::with_default_post("method is not available".into())),
					}
				}
			}
		}
	}
}
