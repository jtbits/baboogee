extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DataEnum, DeriveInput, Fields, Type};

#[proc_macro_derive(Serialize)]
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let name = &ast.ident;

    let expanded = match ast.data {
        syn::Data::Struct(s) => match &s.fields {
            Fields::Named(fields) => {
                let field_quotes = fields.named.iter().map(|f| {
                    let field_name = &f.ident;
                    quote! {
                        offset += self.#field_name.serialize(&mut buf[offset..])?;
                    }
                });

                quote! {
                    impl Serialize for #name {
                        fn serialize(&self, buf: &mut [u8]) -> Result<usize, SerializeError> {
                            let mut offset = 0;

                            #(#field_quotes)*

                            Ok(offset)
                        }
                    }
                }
            }
            _ => panic!("Serialize only works with structs with named fields"),
        },
        syn::Data::Enum(DataEnum { variants, .. }) => {
            let variant_arms = variants.iter().enumerate().map(|(index, variant)| {
                let variant_name = &variant.ident;
                let (enum_field_names, enum_fields) = match &variant.fields {
                    Fields::Named(_fields) => {
                        todo!("named")
                    }
                    Fields::Unnamed(fields) => {
                        let count = fields.unnamed.len();
                        let field_names = (0..count)
                            .map(|i| format_ident!("a{i}"))
                            .collect::<Vec<_>>();

                        let quote_field_names = (0..count)
                            .map(|i| {
                                let enum_var_name = format_ident!("a{}", i);
                                quote! {
                                    #enum_var_name
                                }
                            })
                            .collect();

                        let field_calculations = fields
                            .unnamed
                            .iter()
                            .zip(field_names)
                            .map(|(field, field_name)| match &field.ty {
                                Type::Path(tp) => {
                                    if let Some(_) = tp.path.get_ident() {
                                        return quote! {
                                            offset += #field_name.serialize(&mut buf[offset..])?;
                                        };
                                    }
                                    todo!("no ident")
                                }
                                _ => todo!("unknown type"),
                            })
                            .collect::<Vec<_>>();

                        (quote_field_names, field_calculations)
                    }
                    Fields::Unit => (Vec::default(), Vec::default()),
                };

                let variant_pattern = if enum_field_names.is_empty() {
                    quote! { #name::#variant_name }
                } else {
                    quote! { #name::#variant_name(#(#enum_field_names,)*) }
                };

                quote! {
                    #variant_pattern => {
                        #(#enum_fields)*
                        #index as u8
                    }
                }
            });

            quote! {
                impl Serialize for #name {
                    fn serialize(&self, buf: &mut [u8]) -> Result<usize, SerializeError> {
                        if buf.len() < 1 {
                            return Err(SerializeError::BufferOverflow);
                        }

                        let mut offset = 1;

                        buf[0] = match self {
                            #(#variant_arms,)*
                        };

                        Ok(offset)
                    }
                }
            }
        }
        _ => panic!("Serialize only works with structs and enums"),
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(Deserialize)]
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let name = &ast.ident;

    let expanded = match ast.data {
        syn::Data::Struct(s) => match &s.fields {
            Fields::Named(fields) => {
                let field_deserialize_quotes = fields.named.iter().map(|f| {
                    let field_name = &f.ident;
                    match &f.ty {
                        Type::Path(field_type) => {
                            let field_type_quote = quote! { #field_type };
                            // TODO is this a hack??
                            let field_type = if field_type_quote.to_string().starts_with("Vec") {
                                quote! { Vec }
                            } else {
                                field_type_quote
                            };
                            return quote! {
                                let (#field_name, size) = #field_type::deserialize(&buf[offset..])?;
                                offset += size;
                            }
                        }
                        _ => todo!("unknown type"),
                    }

                })
                .collect::<Vec<_>>();

                let field_name_quotes = fields
                    .named
                    .iter()
                    .map(|f| {
                        let field_name = &f.ident;

                        quote! {
                            #field_name,
                        }
                    })
                    .collect::<Vec<_>>();

                quote! {
                    impl Deserialize for #name {
                        fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
                            let mut offset = 0;

                            #(#field_deserialize_quotes)*

                            Ok((Self { #(#field_name_quotes)* }, offset))
                        }
                    }
                }
            }
            _ => panic!("Serialize only works with structs with named fields"),
        },
        syn::Data::Enum(DataEnum { variants, .. }) => {
            let variant_arms = variants.iter().enumerate().map(|(index, variant)| {
                let variant_name = &variant.ident;
                let (enum_field_names, enum_fields) = match &variant.fields {
                    Fields::Named(_fields) => {
                        todo!("named")
                    }
                    Fields::Unnamed(fields) => {
                        let count = fields.unnamed.len();
                        let field_names = (0..count)
                            .map(|i| format_ident!("a{i}"))
                            .collect::<Vec<_>>();

                        let quote_field_names = (0..count)
                            .map(|i| {
                                let enum_var_name = format_ident!("a{}", i);
                                quote! {
                                    #enum_var_name
                                }
                            })
                            .collect();

                        let field_calculations = fields
                            .unnamed
                            .iter()
                            .zip(field_names)
                            .map(|(field, field_name)| match &field.ty {
                                Type::Path(tp) => {
                                    if let Some(_) = tp.path.get_ident() {
                                        return quote! {
                                            let (#field_name, size) = #tp::deserialize(&buf[offset..])?;
                                            offset += size;
                                        };
                                    }
                                    todo!("no ident")
                                }
                                _ => todo!("unknown type"),
                            })
                            .collect::<Vec<_>>();

                        (quote_field_names, field_calculations)
                    }
                    Fields::Unit => (Vec::default(), Vec::default()),
                };

                let variant_pattern = if enum_field_names.is_empty() {
                    quote! { #name::#variant_name }
                } else {
                    quote! { #name::#variant_name(#(#enum_field_names,)*) }
                };

                let index_u8 = index as u8;

                quote! {
                    #index_u8 => {
                        #(#enum_fields)*
                        #variant_pattern
                    }
                }
            });

            quote! {
                impl Deserialize for #name {
                    fn deserialize(buf: &[u8]) -> Result<(Self, usize), DeserializeError> {
                        if buf.len() < 1 {
                            return Err(DeserializeError::Invalid);
                        }

                        let mut offset = 1;

                        Ok((match buf[0] {
                            #(#variant_arms,)*
                            _ => return Err(DeserializeError::Invalid),
                        }, offset))
                    }
                }
            }
        }
        _ => panic!("Deserialize only works with structs and enums"),
    };

    TokenStream::from(expanded)
}
