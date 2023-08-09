use std::ops::Range;

use proc_macro2::Span;
use quote::format_ident;
use syn::{parse::Error, Ident, Lit, Meta, NestedMeta};

use super::common::OverlapOptions;
use crate::structs::common::{Endianness, FieldAttrs, FieldInfo, ReserveFieldOption};

pub struct TryFromAttrBuilderError {
	pub endianness: Box<Endianness>,
	pub reserve: ReserveFieldOption,
	pub overlap: OverlapOptions,
}

impl TryFromAttrBuilderError {
	pub fn fix(self, bit_range: Range<usize>) -> FieldAttrs {
		FieldAttrs {
			endianness: self.endianness,
			bit_range,
			reserve: self.reserve,
			overlap: self.overlap,
		}
	}
}

impl std::fmt::Display for TryFromAttrBuilderError {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		write!(
			fmt,
			"Did not provide enough information to determine bit_length"
		)
	}
}

#[derive(Clone, Debug)]
pub enum FieldAttrBuilderType {
	None,
	Struct(usize),
	Enum(usize, Ident),
	// amount of bits for each element.
	ElementArray(usize, Box<Option<FieldAttrBuilderType>>),
	BlockArray(Box<Option<FieldAttrBuilderType>>),
}

#[derive(Clone, Debug)]
pub enum FieldBuilderRange {
	// a range of bits to use.
	Range(std::ops::Range<usize>),
	// used to pass on the last starting location to another part to figure out.
	LastEnd(usize),
	None,
}

impl Default for FieldBuilderRange {
	fn default() -> Self {
		Self::None
	}
}

#[derive(Clone, Debug)]
pub struct FieldAttrBuilder {
	/// name is just so we can give better errors
	name: Box<Ident>,
	pub endianness: Box<Endianness>,
	pub bit_range: FieldBuilderRange,
	pub ty: FieldAttrBuilderType,
	pub reserve: ReserveFieldOption,
	pub overlap: OverlapOptions,
}

impl FieldAttrBuilder {
	fn new(name: Box<Ident>) -> Self {
		Self {
			name,
			endianness: Box::new(Endianness::None),
			bit_range: FieldBuilderRange::None,
			ty: FieldAttrBuilderType::None,
			reserve: ReserveFieldOption::NotReserve,
			overlap: OverlapOptions::None,
		}
	}

	fn span(&self) -> Span {
		self.name.span()
	}

	pub fn parse(
		field: &syn::Field,
		last_field: Option<&FieldInfo>,
		name: Box<Ident>,
	) -> syn::Result<FieldAttrBuilder> {
		let mut builder = FieldAttrBuilder::new(name);
		// TODO make this more compact. use match or something.
		// we are just looking for attrs that can fill in the details in the builder variable above
		// sometimes having the last field is useful for example the bit range the builder wants could be
		// filled in using the end of the previous field as the start, add the length in bits you get the
		// end ( this only works if a all bit fields are in order, ex. if a bit_range attribute defines a
		// complete range which occupies the same space as this field and that field is not the "last_field"
		// you will get a conflicting fields error returned to the user... hopefully )
		for attr in field.attrs.iter() {
			let meta = attr.parse_meta()?;
			Self::parse_meta(meta, &last_field, &mut builder)?;
		}
		if let FieldBuilderRange::None = builder.bit_range {
			builder.bit_range = FieldBuilderRange::LastEnd(if let Some(last_value) = last_field {
				last_value.attrs.bit_range.end
			} else {
				0
			})
		}

		Ok(builder)
	}

	fn parse_meta(
		meta: Meta,
		last_field: &Option<&FieldInfo>,
		builder: &mut Self,
	) -> syn::Result<()> {
		match meta {
			Meta::NameValue(value) => {
				if let Some(ident) = value.path.get_ident() {
					let ident_as_str = ident.to_string();
					match ident_as_str.as_str() {
						"endianness" => {
							if let Lit::Str(val) = value.lit {
								builder.endianness = Box::new(match val.value().as_str() {
									"le" | "lsb" | "little" | "lil" => Endianness::Little,
									"be" | "msb" | "big" => Endianness::Big,
									"ne" | "native" => Endianness::None,
									_ => {
										return Err(syn::Error::new(
											builder.span(),
											"{} is not a valid endianness use le or be",
										));
									}
								});
							}
						}
						"bit_length" => {
							if let FieldBuilderRange::None = builder.bit_range {
								if let Lit::Int(val) = value.lit {
									match val.base10_parse::<usize>() {
										Ok(bit_length) => {
											let mut start = 0;
											if let Some(last_value) = last_field {
												start = last_value.attrs.bit_range.end;
											}
											builder.bit_range = FieldBuilderRange::Range(
												start..start + (bit_length),
											);
										}
										Err(err) => {
											return Err(Error::new(
                                                builder.span(),
                                                format!("bit_length must be a number that can be parsed as a usize [{err}]"),
                                            ));
										}
									}
								} else {
									return Err(Error::new(
										builder.span(),
										"bit_length must use a literal usize",
									));
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"please don't double define bit_length",
								));
							}
						}
						"byte_length" => {
							if let FieldBuilderRange::None = builder.bit_range {
								if let Lit::Int(val) = value.lit {
									match val.base10_parse::<usize>() {
										Ok(byte_length) => {
											let mut start = 0;
											if let Some(last_value) = last_field {
												start = last_value.attrs.bit_range.end;
											}
											builder.bit_range = FieldBuilderRange::Range(
												start..start + (byte_length * 8),
											);
										}
										Err(err) => {
											return Err(Error::new(
                                                builder.span(),
                                                format!("bit length must be a number that can be parsed as a usize [{err}]"),
                                            ));
										}
									}
								} else {
									return Err(Error::new(
										builder.span(),
										"bit_length must use a literal usize",
									));
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"please don't double define bit width",
								));
							}
						}
						"enum_primitive" => {
							if let Lit::Str(val) = value.lit {
								let mut ty = Some(match val.value().as_str() {
									"u8" => FieldAttrBuilderType::Enum(1, format_ident!("u8")),
									"u16" => FieldAttrBuilderType::Enum(2, format_ident!("u16")),
									"u32" => FieldAttrBuilderType::Enum(4, format_ident!("u32")),
									"u64" => FieldAttrBuilderType::Enum(8, format_ident!("u64")),
									"u128" => FieldAttrBuilderType::Enum(16, format_ident!("u128")),
									_ => {
										return Err(syn::Error::new(
											builder.span(),
											"primitives for enums must be an unsigned integer",
										))
									}
								});
								match builder.ty {
									FieldAttrBuilderType::BlockArray(ref mut sub_ty) => {
										std::mem::swap(&mut ty, sub_ty)
									}
									FieldAttrBuilderType::ElementArray(_, ref mut sub_ty) => {
										std::mem::swap(&mut ty, sub_ty)
									}
									_ => {
										builder.ty = ty.unwrap();
									}
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"defining a struct_size requires a Int Literal".to_string(),
								));
							}
						}
						"struct_size" => {
							if let Lit::Int(val) = value.lit {
								let mut ty = Some(match val.base10_parse::<usize>() {
									Ok(byte_length) => FieldAttrBuilderType::Struct(byte_length),
									Err(err) => {
										return Err(Error::new(
                                            builder.span(),
                                            format!("struct_size must provided a number that can be parsed as a usize [{err}]"),
                                        ));
									}
								});
								match builder.ty {
									FieldAttrBuilderType::BlockArray(ref mut sub_ty) => {
										std::mem::swap(&mut ty, sub_ty.as_mut())
									}
									FieldAttrBuilderType::ElementArray(_, ref mut sub_ty) => {
										std::mem::swap(&mut ty, sub_ty.as_mut())
									}
									_ => {
										builder.ty = ty.unwrap();
									}
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"defining a struct_size requires a Int Literal".to_string(),
								));
							}
						}
						"bits" => {
							if let Lit::Str(val) = value.lit {
								let val_string = val.value();
								let split =
									val_string.split("..").into_iter().collect::<Vec<&str>>();
								if split.len() == 2 {
									match (split[0].parse::<usize>(), split[1].parse::<usize>()) {
										(Ok(start), Ok(end)) => match builder.bit_range {
											FieldBuilderRange::Range(ref range) => {
												if range.end - range.start == end - start {
													builder.bit_range =
														FieldBuilderRange::Range(start..end);
												} else {
													return Err(Error::new(
                                                        builder.span(),
                                                        "bits attribute didn't match bit range requirements",
                                                    ));
												}
											}
											_ => {
												builder.bit_range =
													FieldBuilderRange::Range(start..end);
											}
										},
										(Ok(_), Err(_)) => {
											return Err(Error::new(
												builder.span(),
												"failed paring ending index for range",
											));
										}
										(Err(_), Ok(_)) => {
											return Err(Error::new(
												builder.span(),
												"failed paring starting index for range",
											));
										}
										_ => {
											return Err(Error::new(
												builder.span(),
												"failed paring range",
											));
										}
									}
								} else {
									return Err(Error::new(
										builder.span(),
										"bits attribute should have data like \"0..8\"",
									));
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"bits must use a literal str value with range inside quotes",
								));
							}
						}
						"element_bit_length" => {
							if let Lit::Int(val) = value.lit {
								match val.base10_parse::<usize>() {
									Ok(bit_length) => {
										builder.bit_range = match std::mem::take(&mut builder.bit_range) {
                                            FieldBuilderRange::None => {
                                                builder.ty = match builder.ty {
                                                    FieldAttrBuilderType::Struct(_) |
                                                    FieldAttrBuilderType::Enum(_, _) => {
                                                        FieldAttrBuilderType::ElementArray(bit_length, Box::new(Some(builder.ty.clone())))
                                                    }
                                                    _ => FieldAttrBuilderType::ElementArray(bit_length, Box::new(None)),
                                                };
                                                if let Some(last_value) = last_field {
                                                    FieldBuilderRange::LastEnd(last_value.attrs.bit_range.end)
                                                }else{
                                                    FieldBuilderRange::LastEnd(0)
                                                }
                                            }
                                            FieldBuilderRange::Range(range) => {
                                                builder.ty = match builder.ty {
                                                    FieldAttrBuilderType::Struct(_) |
                                                    FieldAttrBuilderType::Enum(_, _) => {
                                                        FieldAttrBuilderType::ElementArray(bit_length, Box::new(Some(builder.ty.clone())))
                                                    }
                                                    _ => FieldAttrBuilderType::ElementArray(bit_length, Box::new(None)),
                                                };
                                                FieldBuilderRange::Range(range)
                                            }
                                            _ => return Err(Error::new(
                                                builder.span(),
                                                "found Field bit range no_end while element_bit_length attribute which should never happen",
                                            )),
                                        };
									}
									Err(err) => {
										return Err(Error::new(
                                            builder.span(),
                                            format!("bit_length must be a number that can be parsed as a usize [{err}]"),
                                        ));
									}
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"bit_length must use a literal usize",
								));
							}
						}
						"element_byte_length" => {
							if let Lit::Int(val) = value.lit {
								match val.base10_parse::<usize>() {
									Ok(byte_length) => {
										builder.bit_range = match std::mem::take(&mut builder.bit_range) {
                                            FieldBuilderRange::None => {
                                                builder.ty = match builder.ty {
                                                    FieldAttrBuilderType::Struct(_) |
                                                    FieldAttrBuilderType::Enum(_, _) => {
                                                        FieldAttrBuilderType::ElementArray(byte_length * 8, Box::new(Some(builder.ty.clone())))
                                                    }
                                                    _ => FieldAttrBuilderType::ElementArray(byte_length * 8, Box::new(None)),
                                                };
                                                if let Some(last_value) = last_field {
                                                    FieldBuilderRange::LastEnd(last_value.attrs.bit_range.end)
                                                }else{
                                                    FieldBuilderRange::LastEnd(0)
                                                }
                                            }
                                            FieldBuilderRange::Range(range) => {
                                                builder.ty = match builder.ty {
                                                    FieldAttrBuilderType::Struct(_) |
                                                    FieldAttrBuilderType::Enum(_, _) => {
                                                        FieldAttrBuilderType::ElementArray(byte_length * 8, Box::new(Some(builder.ty.clone())))
                                                    }
                                                    _ => FieldAttrBuilderType::ElementArray(byte_length * 8, Box::new(None)),
                                                };
                                                FieldBuilderRange::Range(range)
                                            }
                                                _ => return Err(Error::new(
                                                    builder.span(),
                                                    "found Field bit range no_end while element_byte_length attribute which should never happen",
                                                )),
                                        };
									}
									Err(err) => {
										return Err(Error::new(
                                            builder.span(),
                                            format!("bit_length must be a number that can be parsed as a usize [{err}]"),
                                        ));
									}
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"bit_length must use a literal usize",
								));
							}
						}
						"block_bit_length" => {
							if let Lit::Int(val) = value.lit {
								match val.base10_parse::<usize>() {
									Ok(bit_length) => {
										builder.bit_range = match std::mem::take(&mut builder.bit_range) {
                                            FieldBuilderRange::None => {
                                                builder.ty = match builder.ty {
                                                    FieldAttrBuilderType::Struct(_) |
                                                    FieldAttrBuilderType::Enum(_, _) => {
                                                        FieldAttrBuilderType::BlockArray(Box::new(Some(builder.ty.clone())))
                                                    }
                                                    _ => FieldAttrBuilderType::BlockArray(Box::new(None)),
                                                };
                                                if let Some(last_value) = last_field {
                                                    FieldBuilderRange::Range(last_value.attrs.bit_range.end..last_value.attrs.bit_range.end + (bit_length))
                                                }else{
                                                    FieldBuilderRange::Range(0..bit_length)
                                                }
                                            }
                                            FieldBuilderRange::Range(range) => {
                                                builder.ty = match builder.ty {
                                                    FieldAttrBuilderType::Struct(_) |
                                                    FieldAttrBuilderType::Enum(_, _) => {
                                                        FieldAttrBuilderType::BlockArray(Box::new(Some(builder.ty.clone())))
                                                    }
                                                    _ => FieldAttrBuilderType::BlockArray(Box::new(None)),
                                                };
                                                if range.end - range.start == bit_length{
                                                FieldBuilderRange::Range(range)
                                                }else{
                                                    return Err(Error::new(
                                                        builder.span(),
                                                        "size of bit-range provided by (bits, bit_length, or byte_length) does not match array_bit_length",
                                                    ));
                                                }
                                            }
                                            _ => return Err(Error::new(
                                                    builder.span(),
                                                    "found Field bit range no_end while array_bit_length attribute which should never happen",
                                                )),
                                        };
									}
									Err(err) => {
										return Err(Error::new(
                                            builder.span(),
                                            format!("array_bit_length must be a number that can be parsed as a usize [{err}]"),
                                        ));
									}
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"array_bit_length must use a literal usize",
								));
							}
						}
						"block_byte_length" => {
							if let Lit::Int(val) = value.lit {
								match val.base10_parse::<usize>() {
									Ok(byte_length) => {
										builder.bit_range = match std::mem::take(&mut builder.bit_range) {
                                            FieldBuilderRange::None => {
                                                builder.ty = match builder.ty {
                                                    FieldAttrBuilderType::Struct(_) |
                                                    FieldAttrBuilderType::Enum(_, _) => {
                                                        FieldAttrBuilderType::BlockArray(Box::new(Some(builder.ty.clone())))
                                                    }
                                                    _ => FieldAttrBuilderType::BlockArray(Box::new(None)),
                                                };
                                                if let Some(last_value) = last_field {
                                                    FieldBuilderRange::Range(last_value.attrs.bit_range.end..last_value.attrs.bit_range.end + (byte_length * 8))
                                                }else{
                                                    FieldBuilderRange::Range(0..byte_length*8)
                                                }
                                            }
                                            FieldBuilderRange::Range(range) => {
                                                builder.ty = match builder.ty {
                                                    FieldAttrBuilderType::Struct(_) |
                                                    FieldAttrBuilderType::Enum(_, _) => {
                                                        FieldAttrBuilderType::BlockArray(Box::new(Some(builder.ty.clone())))
                                                    }
                                                    _ => FieldAttrBuilderType::BlockArray(Box::new(None)),
                                                };
                                                if range.end - range.start == byte_length * 8{
                                                FieldBuilderRange::Range(range)
                                                }else{
                                                    return Err(Error::new(
                                                        builder.span(),
                                                        "size of bit-range provided by (bits, bit_length, or byte_length) does not match array_byte_length",
                                                    ));
                                                }
                                            }
                                            _ => return Err(Error::new(
                                                builder.span(),
                                                "found Field bit range no_end while array_byte_length attribute which should never happen",
                                            )),
                                        };
									}
									Err(err) => {
										return Err(Error::new(
                                            builder.span(),
                                            format!("array_byte_length must be a number that can be parsed as a usize [{err}]"),
                                        ));
									}
								}
							} else {
								return Err(Error::new(
									builder.span(),
									"array_byte_length must use a literal usize",
								));
							}
						}
						"overlapping_bits" => {
							if let Lit::Int(val) = value.lit {
								match val.base10_parse::<usize>() {
									Ok(bits) => builder.overlap = OverlapOptions::Allow(bits),
									Err(err) => {
										return Err(Error::new(
                                            builder.span(),
                                            format!("overlapping_bits must provided a number that can be parsed as a usize [{err}]"),
                                        ));
									}
								};
							} else {
								return Err(Error::new(
									builder.span(),
									"defining a overlapping_bits requires a Int Literal"
										.to_string(),
								));
							}
						}
						_ => {
							if ident_as_str.as_str() != "doc" {
								return Err(Error::new(
									builder.span(),
									format!("\"{ident_as_str}\" is not a valid attribute"),
								));
							}
						}
					}
				}
			}
			Meta::Path(path) => {
				if let Some(ident) = path.get_ident() {
					match ident.to_string().as_str() {
						"reserve" => {
							builder.reserve = ReserveFieldOption::ReserveField;
						}
						"read_only" => {
							builder.reserve = ReserveFieldOption::ReadOnly;
						}
						// TODO  can not enable this until i figure out a way to express exactly the amount
						// of overlapping bits.
						/*"allow_overlap" => {
							builder.overlap = OverlapOptions::Allow;
						}*/
						"redundant" => {
							builder.overlap = OverlapOptions::Redundant;
							builder.reserve = ReserveFieldOption::ReadOnly;
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
								Self::parse_meta(meta, last_field, builder)?;
							}
							NestedMeta::Lit(_) => {}
						}
					}
				}
			}
		}
		Ok(())
	}
}

impl TryInto<FieldAttrs> for FieldAttrBuilder {
	type Error = TryFromAttrBuilderError;
	fn try_into(self) -> std::result::Result<FieldAttrs, Self::Error> {
		if let FieldBuilderRange::Range(bit_range) = self.bit_range {
			Ok(FieldAttrs {
				endianness: self.endianness,
				bit_range,
				reserve: self.reserve,
				overlap: self.overlap,
			})
		} else {
			Err(TryFromAttrBuilderError {
				endianness: self.endianness,
				reserve: self.reserve,
				overlap: self.overlap,
			})
		}
	}
}
