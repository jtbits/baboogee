extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Fields, DataEnum};

#[proc_macro_derive(Serialize)]
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let name = &ast.ident;

    let expanded = match ast.data {
        syn::Data::Struct(s) => {
            match &s.fields {
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
                },
                _ => panic!("MyMacro only works with structs with named fields"),
            }
        },
        syn::Data::Enum(DataEnum { variants, .. }) => {
            let variant_arms = variants.iter().enumerate().map(|(index, variant)| {
                let variant_name = &variant.ident;
                quote! { #name::#variant_name => #index as u8 }
            });

            quote! {
                impl Serialize for #name {
                    fn serialize(&self, buf: &mut [u8]) -> Result<usize, SerializeError> {
                        buf[0] = match self {
                            #(#variant_arms,)*
                        };

                        Ok(1)
                    }
                }
            }
        },
        _ => panic!("MyMacro only works with structs and enums"),
    };

    println!("generated: {}", expanded.to_string());
    TokenStream::from(expanded)
}
