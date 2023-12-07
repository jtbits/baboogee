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
                    },
                    Fields::Unnamed(fields) => {
                        let count = fields.unnamed.len();
                        let field_names = (0..count).map(|i| format_ident!("a{i}")).collect::<Vec<_>>();

                        let quote_field_names = (0..count)
                            .map(|i| {
                                let enum_var_name = format_ident!("a{}", i);
                                quote! {
                                    #enum_var_name
                                }
                            })
                        .collect();

                        let field_calculations = fields.unnamed.iter().zip(field_names)
                            .map(|(field, field_name)| {
                                match &field.ty {
                                    Type::Path(tp) => {
                                        if let Some(_) = tp.path.get_ident() {
                                            return quote! {
                                                offset += #field_name.serialize(&mut buf[offset..])?;
                                            }
                                        }
                                        todo!("no ident")
                                    },
                                    _ => todo!("unknown type"),
                                }
                            })
                        .collect::<Vec<_>>();

                        (quote_field_names, field_calculations)
                    },
                    Fields::Unit => {
                        (Vec::default(), Vec::default())
                    },
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
