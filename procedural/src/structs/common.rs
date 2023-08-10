use std::ops::Range;

use proc_macro2::Span;
use quote::quote;
use syn::{parse::Error, DeriveInput, Ident, Lit, Meta, NestedMeta, Type};

use crate::{
	abi_derive::extract_docs,
	structs::parse::{
		FieldAttrBuilder, FieldAttrBuilderType, FieldBuilderRange, TryFromAttrBuilderError,
	},
};

pub struct BitMath {
	pub amount_of_bits: usize,
	pub zeros_on_left: usize,
	pub available_bits_in_first_byte: usize,
	pub starting_inject_byte: usize,
}

impl BitMath {
	pub fn from_field(field: &FieldInfo) -> Result<Self, syn::Error> {
		// get the total number of bits the field uses.
		let amount_of_bits = field.attrs.bit_length();
		// amount of zeros to have for the right mask. (right mask meaning a mask to keep data on the
		// left)
		let zeros_on_left = field.attrs.bit_range.start % 8;
		// NOTE endianness is only for determining how to get the bytes we will apply to the output.
		// calculate how many of the bits will be inside the most significant byte we are adding to.
		if 7 < zeros_on_left {
			return Err(syn::Error::new(
				field.ident.span(),
				"ne 8 - zeros_on_left = underflow",
			));
		}
		let available_bits_in_first_byte = 8 - zeros_on_left;
		// calculate the starting byte index in the outgoing buffer
		let starting_inject_byte: usize = field.attrs.bit_range.start / 8;
		Ok(Self {
			amount_of_bits,
			zeros_on_left,
			available_bits_in_first_byte,
			starting_inject_byte,
		})
	}

	/// (amount_of_bits, zeros_on_left, available_bits_in_first_byte, starting_inject_byte)
	pub fn into_tuple(self) -> (usize, usize, usize, usize) {
		(
			self.amount_of_bits,
			self.zeros_on_left,
			self.available_bits_in_first_byte,
			self.starting_inject_byte,
		)
	}
}

#[derive(Clone, Debug)]
pub enum Endianness {
	Little,
	Big,
	None,
}

impl Endianness {
	fn has_endianness(&self) -> bool {
		!matches!(self, Self::None)
	}
	fn perhaps_endianness(&mut self, size: usize) -> bool {
		if let Self::None = self {
			if size == 1 {
				let mut swap = Self::Big;
				std::mem::swap(&mut swap, self);
				true
			} else {
				false
			}
		} else {
			true
		}
	}
}

#[derive(Clone, Debug)]
pub enum NumberSignage {
	Signed,
	Unsigned,
}

#[derive(Clone, Debug)]
pub enum FieldDataType {
	Boolean,
	/// first field is byte size for number
	Number(usize, NumberSignage, proc_macro2::TokenStream),
	Float(usize, proc_macro2::TokenStream),
	/// first value is primitive type byte size of enum value in bytes.
	Enum(proc_macro2::TokenStream, usize, proc_macro2::TokenStream),
	/// first field is size in BYTES of the entire struct
	Struct(usize, proc_macro2::TokenStream),
	Char(usize, proc_macro2::TokenStream),
	// array types are Subfield info, array length, ident
	ElementArray(Box<SubFieldInfo>, usize, proc_macro2::TokenStream),
	BlockArray(Box<SubFieldInfo>, usize, proc_macro2::TokenStream),
}

impl FieldDataType {
	/// byte size of actual rust type .
	pub fn size(&self) -> usize {
		match self {
			Self::Number(ref size, _, _) => *size,
			Self::Float(ref size, _) => *size,
			Self::Enum(_, ref size, _) => *size,
			Self::Struct(ref size, _) => *size,
			Self::Char(ref size, _) => *size,
			Self::ElementArray(ref fields, ref length, _) => fields.ty.size() * length,
			Self::BlockArray(ref fields, size, _) => fields.ty.size() * size,
			Self::Boolean => 1,
		}
	}

	pub fn type_quote(&self) -> proc_macro2::TokenStream {
		match self {
			Self::Number(_, _, ref ident) => ident.clone(),
			Self::Float(_, ref ident) => ident.clone(),
			Self::Enum(_, _, ref ident) => ident.clone(),
			Self::Struct(_, ref ident) => ident.clone(),
			Self::Char(_, ref ident) => ident.clone(),
			Self::ElementArray(_, _, ref ident) => ident.clone(),
			Self::BlockArray(_, _, ident) => ident.clone(),
			Self::Boolean => quote! {bool},
		}
	}
	pub fn is_number(&self) -> bool {
		// TODO put Arrays in here
		match self {
			Self::Enum(_, _, _) | Self::Number(_, _, _) | Self::Float(_, _) | Self::Char(_, _) => {
				true
			}
			Self::Boolean | Self::Struct(_, _) => false,
			Self::ElementArray(ref ty, _, _) | Self::BlockArray(ref ty, _, _) => {
				ty.as_ref().ty.is_number()
			}
		}
	}
	fn get_element_bit_length(&self) -> usize {
		match self {
			Self::Boolean => 1,
			Self::Char(_, _) => 32,
			Self::Number(ref size, _, _) => size * 8,
			Self::Enum(_, ref size, _) => size * 8,
			Self::Float(ref size, _) => size * 8,
			Self::Struct(ref size, _) => size * 8,
			Self::BlockArray(sub, _, _) => sub.as_ref().ty.get_element_bit_length(),
			Self::ElementArray(sub, _, _) => sub.as_ref().ty.get_element_bit_length(),
		}
	}

	pub fn parse(
		ty: &syn::Type,
		attrs: &mut FieldAttrBuilder,
		ident: &Ident,
		default_endianess: &Endianness,
	) -> syn::Result<FieldDataType> {
		let data_type = match ty {
			Type::Path(ref path) => match attrs.ty {
				FieldAttrBuilderType::Struct(ref size) => FieldDataType::Struct(
					*size,
					if let Some(last_segment) = path.path.segments.last() {
						let asdf = &last_segment.ident;
						quote! {#asdf}
					} else {
						return Err(syn::Error::new(ident.span(), "field has no Type?"));
					},
				),
				FieldAttrBuilderType::Enum(ref size, ref prim) => FieldDataType::Enum(
					quote! {#prim},
					*size,
					if let Some(last_segment) = path.path.segments.last() {
						let asdf = &last_segment.ident;
						quote! {#asdf}
					} else {
						return Err(syn::Error::new(ident.span(), "field has no Type?"));
					},
				),
				_ => Self::parse_path(&path.path, attrs, ident.span())?,
			},
			Type::Array(ref array_path) => {
				// arrays must use a literal for length, because its would be hard any other way.
				if let syn::Expr::Lit(ref lit_expr) = array_path.len {
					if let syn::Lit::Int(ref lit_int) = lit_expr.lit {
						if let Ok(array_length) = lit_int.base10_parse::<usize>() {
							match attrs.ty {
								FieldAttrBuilderType::ElementArray(
									ref element_bit_size,
									ref sub,
								) => {
									attrs.bit_range = match std::mem::take(&mut attrs.bit_range) {
										FieldBuilderRange::Range(ref range) => {
											if range.end < range.start {
												return Err(syn::Error::new(
													ident.span(),
													"range end is less than range start",
												));
											}
											if range.end - range.start
												!= *element_bit_size * array_length
											{
												return Err(
                                                    syn::Error::new(
                                                        ident.span(),
                                                        "Element arrays bit range didn't match (element bit size * array length)"
                                                    )
                                                );
											}
											FieldBuilderRange::Range(range.clone())
										}
										FieldBuilderRange::LastEnd(ref last_end) => {
											FieldBuilderRange::Range(
												*last_end
													..last_end + (array_length * *element_bit_size),
											)
										}
										_ => {
											return Err(syn::Error::new(
												ident.span(),
												"failed getting Range for element array",
											));
										}
									};
									let mut sub_attrs = attrs.clone();
									if let Type::Array(_) = array_path.elem.as_ref() {
									} else if let Some(ref ty) = sub.as_ref() {
										sub_attrs.ty = ty.clone();
									} else {
										sub_attrs.ty = FieldAttrBuilderType::None;
									}
									let sub_ty = Self::parse(
										&array_path.elem,
										&mut sub_attrs,
										ident,
										default_endianess,
									)?;

									let type_ident = &sub_ty.type_quote();
									FieldDataType::ElementArray(
										Box::new(SubFieldInfo { ty: sub_ty }),
										array_length,
										quote! {[#type_ident;#array_length]},
									)
								}
								FieldAttrBuilderType::BlockArray(_) => {
									let mut sub_attrs = attrs.clone();
									if let Type::Array(_) = array_path.elem.as_ref() {
									} else {
										sub_attrs.ty = FieldAttrBuilderType::None;
									}

									let sub_ty = Self::parse(
										&array_path.elem,
										&mut sub_attrs,
										ident,
										default_endianess,
									)?;
									attrs.endianness = sub_attrs.endianness;
									let type_ident = &sub_ty.type_quote();
									FieldDataType::BlockArray(
										Box::new(SubFieldInfo { ty: sub_ty }),
										array_length,
										quote! {[#type_ident;#array_length]},
									)
								}
								FieldAttrBuilderType::Enum(_, _)
								| FieldAttrBuilderType::Struct(_) => {
									let mut sub_attrs = attrs.clone();
									if let Type::Array(_) = array_path.elem.as_ref() {
									} else {
										sub_attrs.ty = attrs.ty.clone();
									}

									let sub_ty = Self::parse(
										&array_path.elem,
										&mut sub_attrs,
										ident,
										default_endianess,
									)?;
									attrs.endianness = sub_attrs.endianness;
									let type_ident = &sub_ty.type_quote();
									FieldDataType::BlockArray(
										Box::new(SubFieldInfo { ty: sub_ty }),
										array_length,
										quote! {[#type_ident;#array_length]},
									)
								}
								FieldAttrBuilderType::None => {
									let mut sub_attrs = attrs.clone();
									if let Type::Array(_) = array_path.elem.as_ref() {
									} else {
										sub_attrs.ty = FieldAttrBuilderType::None;
									}
									let sub_ty = Self::parse(
										&array_path.elem,
										&mut sub_attrs,
										ident,
										default_endianess,
									)?;
									attrs.bit_range = match std::mem::take(&mut attrs.bit_range) {
										FieldBuilderRange::Range(ref range) => {
											if range.end < range.start {
												return Err(syn::Error::new(
													ident.span(),
													"range end is less than range start",
												));
											}
											if range.end - range.start % array_length != 0 {
												return Err(
                                                    syn::Error::new(
                                                        ident.span(),
                                                        "Array Inference failed because given total bit_length does not split up evenly between elements"
                                                    )
                                                );
											}
											FieldBuilderRange::Range(range.clone())
										}
										FieldBuilderRange::LastEnd(ref last_end) => {
											let element_bit_length =
												sub_ty.get_element_bit_length();
											FieldBuilderRange::Range(
												*last_end
													..last_end
														+ (array_length * element_bit_length),
											)
										}
										_ => {
											return Err(syn::Error::new(
												ident.span(),
												"failed getting Range for element array",
											));
										}
									};
									let type_ident = &sub_ty.type_quote();
									FieldDataType::ElementArray(
										Box::new(SubFieldInfo { ty: sub_ty }),
										array_length,
										quote! {[#type_ident;#array_length]},
									)
								}
							}
						} else {
							return Err(Error::new(
								array_path.bracket_token.span,
								"failed parsing array length as literal integer",
							));
						}
					} else {
						return Err(Error::new(array_path.bracket_token.span, "Couldn't determine Array length, literal array lengths must be an integer"));
					}
				} else {
					return Err(Error::new(
						array_path.bracket_token.span,
						"Couldn't determine Array length, must be literal",
					));
				}
			}
			_ => {
				return Err(Error::new(ident.span(), "Unsupported field type"));
			}
		};
		// if the type is a number and its endianess is None (numbers should have endianess) then we
		// apply the structs default (which might also be None)
		if data_type.is_number() && !attrs.endianness.perhaps_endianness(data_type.size()) {
			if default_endianess.has_endianness() {
				attrs.endianness = Box::new(default_endianess.clone());
			} else if data_type.size() == 1 {
				let mut big = Endianness::Big;
				std::mem::swap(attrs.endianness.as_mut(), &mut big);
			} else {
				return Err(Error::new(ident.span(), "field without defined endianess found, please set endianess of struct or fields"));
			}
		}

		Ok(data_type)
	}

	fn parse_path(
		path: &syn::Path,
		attrs: &mut FieldAttrBuilder,
		field_span: Span,
	) -> syn::Result<FieldDataType> {
		// TODO added attribute consideration for recognizing structs and enums.
		// TODO impl enum logic.
		// TODO impl struct logic
		match attrs.ty {
			FieldAttrBuilderType::None => {
				if let Some(last_segment) = path.segments.last() {
					let type_quote = &last_segment.ident;
					let field_type_name = last_segment.ident.to_string();
					match field_type_name.as_str() {
						"bool" => match attrs.bit_range {
							FieldBuilderRange::LastEnd(start) => {
								attrs.bit_range = FieldBuilderRange::Range(start..start + 1);
								Ok(FieldDataType::Boolean)
							}
							_ => Ok(FieldDataType::Boolean),
						},
						"u8" => Ok(FieldDataType::Number(
							1,
							NumberSignage::Unsigned,
							quote! {#type_quote},
						)),
						"i8" => Ok(FieldDataType::Number(
							1,
							NumberSignage::Signed,
							quote! {#type_quote},
						)),
						"u16" => Ok(FieldDataType::Number(
							2,
							NumberSignage::Unsigned,
							quote! {#type_quote},
						)),
						"i16" => Ok(FieldDataType::Number(
							2,
							NumberSignage::Signed,
							quote! {#type_quote},
						)),
						"f32" => {
							if let FieldBuilderRange::Range(ref span) = attrs.bit_range {
								if 32 != span.end - span.start {
									return Err(syn::Error::new(field_span, format!("f32 must be full sized, if this is a problem for you open an issue.. provided bit length = {}.", span.end - span.start)));
								}
							}
							Ok(FieldDataType::Float(4, quote! {#type_quote}))
						}
						"u32" => Ok(FieldDataType::Number(
							4,
							NumberSignage::Unsigned,
							quote! {#type_quote},
						)),
						"i32" => Ok(FieldDataType::Number(
							4,
							NumberSignage::Signed,
							quote! {#type_quote},
						)),
						"char" => Ok(FieldDataType::Char(4, quote! {#type_quote})),
						"f64" => {
							if let FieldBuilderRange::Range(ref span) = attrs.bit_range {
								if 64 != span.end - span.start {
									return Err(syn::Error::new(field_span, format!("f64 must be full sized, if this is a problem for you open an issue. provided bit length = {}.", span.end - span.start)));
								}
							}
							Ok(FieldDataType::Float(8, quote! {#type_quote}))
						}
						"u64" => Ok(FieldDataType::Number(
							8,
							NumberSignage::Unsigned,
							quote! {#type_quote},
						)),
						"i64" => Ok(FieldDataType::Number(
							8,
							NumberSignage::Signed,
							quote! {#type_quote},
						)),
						"u128" => Ok(FieldDataType::Number(
							16,
							NumberSignage::Unsigned,
							quote! {#type_quote},
						)),
						"i128" => Ok(FieldDataType::Number(
							16,
							NumberSignage::Signed,
							quote! {#type_quote},
						)),
						"usize" | "isize" => Err(Error::new(
							field_span,
							"usize and isize are not supported due to ambiguous sizing".to_string(),
						)),
						_ => Err(Error::new(
							field_span,
							format!("unknown primitive type [{field_type_name}]"),
						)),
					}
				} else {
					Err(syn::Error::new(field_span, "field has no Type?"))
				}
			}
			FieldAttrBuilderType::Struct(size) => {
				if let Some(ident) = path.get_ident() {
					Ok(FieldDataType::Struct(size, quote! {#ident}))
				} else {
					Err(syn::Error::new(field_span, "field has no Type?"))
				}
			}
			FieldAttrBuilderType::Enum(size, ref type_ident) => {
				if let Some(ident) = path.get_ident() {
					Ok(FieldDataType::Enum(
						quote! {#type_ident},
						size,
						quote! {#ident},
					))
				} else {
					Err(syn::Error::new(field_span, "field has no Type?"))
				}
			}
			_ => Err(syn::Error::new(
				field_span,
				"Array did not get detected properly, found Path",
			)),
		}
	}
}

#[derive(Clone, Debug)]
pub enum ReserveFieldOption {
	NotReserve,
	ReserveField,
	FakeReserveField,
	ReadOnly,
}

#[derive(Clone, Debug)]
pub enum OverlapOptions {
	None,
	Allow(usize),
	Redundant,
}

impl OverlapOptions {
	pub fn enabled(&self) -> bool {
		!matches!(self, Self::None)
	}
	pub fn is_redundant(&self) -> bool {
		matches!(self, Self::Redundant)
	}
}

#[derive(Clone, Debug)]
pub struct FieldAttrs {
	pub endianness: Box<Endianness>,
	pub bit_range: Range<usize>,
	pub reserve: ReserveFieldOption,
	pub overlap: OverlapOptions,
}

impl FieldAttrs {
	pub fn bit_length(&self) -> usize {
		self.bit_range.end - self.bit_range.start
	}
}

#[derive(Clone, Debug)]
pub struct SubFieldInfo {
	pub ty: FieldDataType,
}

#[derive(Clone, Debug)]
pub struct FieldInfo {
	pub name: Ident,
	pub ident: Box<Ident>,
	pub ty: FieldDataType,
	pub attrs: FieldAttrs,
	pub docs: Vec<String>,
}

impl FieldInfo {
	fn overlapping(&self, other: &Self) -> bool {
		if self.attrs.overlap.enabled() || other.attrs.overlap.enabled() {
			return false;
		}
		// check that self's start is not within other's range
		if self.attrs.bit_range.start >= other.attrs.bit_range.start
			&& (self.attrs.bit_range.start == other.attrs.bit_range.start
				|| self.attrs.bit_range.start < other.attrs.bit_range.end)
		{
			return true;
		}
		// check that other's start is not within self's range
		if other.attrs.bit_range.start >= self.attrs.bit_range.start
			&& (other.attrs.bit_range.start == self.attrs.bit_range.start
				|| other.attrs.bit_range.start < self.attrs.bit_range.end)
		{
			return true;
		}
		if self.attrs.bit_range.end > other.attrs.bit_range.start
			&& self.attrs.bit_range.end <= other.attrs.bit_range.end
		{
			return true;
		}
		if other.attrs.bit_range.end > self.attrs.bit_range.start
			&& other.attrs.bit_range.end <= self.attrs.bit_range.end
		{
			return true;
		}
		false
	}

	#[inline]
	// this returns how many bits of the fields pertain to total structure bits.
	// where as attrs.bit_length() give you bits the fields actually needs.
	pub fn bit_size(&self) -> usize {
		if self.attrs.overlap.is_redundant() {
			0
		} else {
			let minus = if let OverlapOptions::Allow(skip) = self.attrs.overlap {
				skip
			} else {
				0
			};
			(self.attrs.bit_range.end - self.attrs.bit_range.start) - minus
		}
	}

	pub fn from_syn_field(field: &syn::Field, struct_info: &StructInfo) -> syn::Result<Self> {
		let ident: Box<Ident> = if let Some(ref name) = field.ident {
			Box::new(name.clone())
		} else {
			return Err(Error::new(Span::call_site(), "all fields must be named"));
		};
		// parse all attrs. which will also give us the bit locations
		// NOTE read only attribute assumes that the value should not effect the placement of the rest og
		let last_relevant_field = struct_info
			.fields
			.iter()
			.filter(|x| !x.attrs.overlap.is_redundant())
			.last();
		let mut attrs_builder = FieldAttrBuilder::parse(field, last_relevant_field, ident.clone())?;
		// check the field for supported types.
		let data_type = FieldDataType::parse(
			&field.ty,
			&mut attrs_builder,
			&ident,
			&struct_info.default_endianess,
		)?;

		let attr_result: std::result::Result<FieldAttrs, TryFromAttrBuilderError> =
			attrs_builder.try_into();

		let attrs = match attr_result {
			Ok(attr) => attr,
			Err(fix_me) => {
				let mut start = 0;
				if let Some(last_value) = last_relevant_field {
					start = last_value.attrs.bit_range.end;
				}
				fix_me.fix(start..start + (data_type.size() * 8))
			}
		};

		let docs = extract_docs(&field.attrs).unwrap_or_default();

		// construct the field we are parsed.
		let new_field = FieldInfo {
			name: ident.as_ref().clone(),
			ident: ident.clone(),
			ty: data_type,
			attrs,
			docs,
		};
		// check to verify there are no overlapping bit ranges from previously parsed fields.
		for (parsed_field, i) in struct_info.fields.iter().zip(0..struct_info.fields.len()) {
			if parsed_field.overlapping(&new_field) {
				return Err(Error::new(
					Span::call_site(),
					format!("fields {} and {} overlap", i, struct_info.fields.len()),
				));
			}
		}

		Ok(new_field)
	}
}

#[derive(Debug)]
pub enum StructEnforcement {
	/// there is no enforcement so if bits are unused then it will act like they are a reserve field
	NoRules,
	/// enforce the BIT_SIZE equals BYTE_SIZE * 8
	EnforceFullBytes,
	/// enforce an amount of bits total that need to be used.
	EnforceBitAmount(usize),
}

pub struct StructInfo {
	pub name: Ident,
	/// if false then bit 0 is the Most Significant Bit meaning the first values first bit will start there.
	/// if true then bit 0 is the Least Significant Bit (the last bit in the last byte).
	pub lsb_zero: bool,
	/// flip all the bytes, like .reverse() for vecs or arrays. but we do that here because we can do
	/// it with no runtime cost.
	pub flip: bool,
	pub enforcement: StructEnforcement,
	pub fields: Vec<FieldInfo>,
	pub default_endianess: Endianness,
	pub fill_bits: Option<usize>,
	pub vis: syn::Visibility,
}

impl StructInfo {
	pub fn total_bits(&self) -> usize {
		let mut total: usize = 0;
		for field in self.fields.iter() {
			total += field.bit_size();
		}
		total
	}

	pub fn total_bytes(&self) -> usize {
		(self.total_bits() as f64 / 8.0f64).ceil() as usize
	}
	fn parse_struct_attrs_meta(info: &mut StructInfo, meta: Meta) -> Result<(), syn::Error> {
		match meta {
			Meta::NameValue(value) => {
				if value.path.is_ident("read_from") {
					if let Lit::Str(val) = value.lit {
						match val.value().as_str() {
                            "lsb0" => info.lsb_zero = true,
                            "msb0" => info.lsb_zero = false,
                            _ => return Err(Error::new(
                                val.span(),
                                "Expected literal str \"lsb0\" or \"msb0\" for read_from attribute.",
                            )),
                        }
					}
				} else if value.path.is_ident("default_endianness") {
					if let Lit::Str(val) = value.lit {
						match val.value().as_str() {
							"le" | "lsb" | "little" | "lil" => {
								info.default_endianess = Endianness::Little
							}
							"be" | "msb" | "big" => info.default_endianess = Endianness::Big,
							"ne" | "native" => info.default_endianess = Endianness::None,
							_ => {}
						}
					}
				} else if value.path.is_ident("enforce_bytes") {
					if let Lit::Int(val) = value.lit {
						match val.base10_parse::<usize>() {
							Ok(value) => {
								info.enforcement = StructEnforcement::EnforceBitAmount(value * 8);
							}
							Err(err) => {
								return Err(syn::Error::new(
									info.name.span(),
									format!("failed parsing enforce_bytes value [{err}]"),
								))
							}
						}
					}
				} else if value.path.is_ident("enforce_bits") {
					if let Lit::Int(val) = value.lit {
						match val.base10_parse::<usize>() {
							Ok(value) => {
								info.enforcement = StructEnforcement::EnforceBitAmount(value);
							}
							Err(err) => {
								return Err(syn::Error::new(
									info.name.span(),
									format!("failed parsing enforce_bits value [{err}]"),
								))
							}
						}
					}
				} else if value.path.is_ident("fill_bytes") {
					if let Lit::Int(val) = value.lit {
						match val.base10_parse::<usize>() {
							Ok(value) => {
								if info.fill_bits.is_none() {
									info.fill_bits = Some(value * 8);
								} else {
									return Err(syn::Error::new(
										info.name.span(),
										"multiple fill_bits values".to_string(),
									));
								}
							}
							Err(err) => {
								return Err(syn::Error::new(
									info.name.span(),
									format!("failed parsing fill_bits value [{err}]"),
								))
							}
						}
					}
				}
			}
			Meta::Path(value) => {
				if let Some(ident) = value.get_ident() {
					match ident.to_string().as_str() {
						"reverse" => {
							info.flip = true;
						}
						"enforce_full_bytes" => {
							info.enforcement = StructEnforcement::EnforceFullBytes;
						}
						_ => {}
					}
				}
			}
			Meta::List(meta_list) => {
				if meta_list.path.is_ident("bondrewd") {
					for nested_meta in meta_list.nested {
						match nested_meta {
							NestedMeta::Meta(meta) => {
								Self::parse_struct_attrs_meta(info, meta)?;
							}
							NestedMeta::Lit(_) => {}
						}
					}
				}
			}
		}
		Ok(())
	}
	pub fn parse(input: &DeriveInput) -> syn::Result<StructInfo> {
		// get the struct, error out if not a struct
		let data = match input.data {
			syn::Data::Struct(ref data) => data,
			_ => {
				return Err(Error::new(Span::call_site(), "input must be a struct"));
			}
		};
		let mut info = StructInfo {
			name: input.ident.clone(),
			lsb_zero: false,
			flip: false,
			enforcement: StructEnforcement::NoRules,
			fields: Default::default(),
			default_endianess: Endianness::None,
			fill_bits: None,
			vis: input.vis.clone(),
		};
		for attr in input.attrs.iter() {
			let meta = attr.parse_meta()?;
			Self::parse_struct_attrs_meta(&mut info, meta)?;
		}
		// get the list of fields in syn form, error out if unit struct (because they have no data, and
		// data packing/analysis don't seem necessary)
		let fields = match data.fields {
            syn::Fields::Named(ref named_fields) => named_fields.named.iter().cloned().collect::<Vec<syn::Field>>(),
            syn::Fields::Unnamed(ref fields) => fields.unnamed.iter().cloned().collect::<Vec<syn::Field>>(),
            syn::Fields::Unit => return Err(Error::new(data.struct_token.span, "Packing a Unit Struct (Struct with no data) seems pointless to me, so i didn't write code for it.")),
        };

		// figure out what the field are and what/where they should be in byte form.
		let mut bit_size = 0;
		for ref field in fields {
			let parsed_field = FieldInfo::from_syn_field(field, &info)?;
			bit_size += parsed_field.bit_size();
			info.fields.push(parsed_field);
		}

		match info.enforcement {
			StructEnforcement::NoRules => {}
			StructEnforcement::EnforceFullBytes => {
				if bit_size % 8 != 0 {
					return Err(syn::Error::new(
						info.name.span(),
						"BIT_SIZE modulus 8 is not zero",
					));
				}
			}
			StructEnforcement::EnforceBitAmount(expected_total_bits) => {
				if bit_size != expected_total_bits {
					return Err(syn::Error::new(
                        info.name.span(),
                        format!(
                            "Bit Enforcement failed because bondrewd detected {bit_size} total bits used by defined fields, but the bit enforcement attribute is defined as {expected_total_bits} bits."
                        ),
                    ));
				}
			}
		}

		// add reserve for fill bytes. this happens after bit enforcement because bit_enforcement is for checking user code.
		if let Some(fill_bits) = info.fill_bits {
			let first_bit = if let Some(last_range) = info.fields.iter().last() {
				last_range.attrs.bit_range.end
			} else {
				0_usize
			};
			let fill_bytes_size = ((fill_bits - first_bit) as f64 / 8.0_f64).ceil() as usize;
			let ident = quote::format_ident!("bondrewd_fill_bits");
			info.fields.push(FieldInfo {
				name: ident.clone(),
				ident: Box::new(ident),
				attrs: FieldAttrs {
					bit_range: first_bit..fill_bits,
					endianness: Box::new(Endianness::Big),
					reserve: ReserveFieldOption::FakeReserveField,
					overlap: OverlapOptions::None,
				},
				ty: FieldDataType::BlockArray(
					Box::new(SubFieldInfo {
						ty: FieldDataType::Number(1, NumberSignage::Unsigned, quote! {u8}),
					}),
					fill_bytes_size,
					quote! {[u8;#fill_bytes_size]},
				),
				docs: Vec::default(),
			});
		}

		if info.lsb_zero {
			for ref mut field in info.fields.iter_mut() {
				field.attrs.bit_range = (bit_size - field.attrs.bit_range.end)
					..(bit_size - field.attrs.bit_range.start);
			}
			info.fields.reverse();
		}

		Ok(info)
	}
}
