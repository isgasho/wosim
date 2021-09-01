use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput};

pub fn db_vec_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let vec_name = Ident::new(&format!("{}Vec", name), Span::call_site());

    let members = members(&input.data);
    let new = new(&input.data);
    let serialize = serialize(&input.data);
    let deserialize = deserialize(&input.data);
    let push = push(&input.data);
    let write = write(&input.data);

    let expanded = quote! {
        pub struct #vec_name {
            #members
            free: database::Vec<usize>,
            used: database::Vec<u8>,
        }

        impl #vec_name {

            pub fn new(database: database::DatabaseRef) -> Self {
                Self {
                    #new
                    free: database::Vec::new(database.clone()),
                    used: database::Vec::new(database)
                }
            }

            pub fn serialize(&mut self, mut writer: impl std::io::Write) -> std::io::Result<()> {
                #serialize
                self.free.serialize(&mut writer)?;
                self.used.serialize(&mut writer)?;
                Ok(())
            }

            pub fn deserialize(mut reader: impl std::io::Read, database: database::DatabaseRef) -> std::io::Result<Self> {
                Ok(Self {
                    #deserialize
                    free: database::Vec::deserialize(&mut reader, database.clone())?,
                    used: database::Vec::deserialize(&mut reader, database.clone())?,
                })
            }

            pub fn add(&mut self, mut value: #name) -> usize {
                use database::Len;
                let mut free = self.free.write();
                let mut used = self.used.write();
                if free.is_empty() {
                    #push
                    let index = used.len();
                    used.push(1);
                    index
                } else {
                    let index = free.pop().unwrap();
                    #write
                    index
                }
            }

            pub fn free(&mut self, index: usize) {
                let mut free = self.free.write();
                let mut used = self.used.write();
                assert_eq!(used[index], 1);
                used[index] = 0;
                free.push(index);
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn members(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    let ty = &f.ty;
                    if f.ident.as_ref().unwrap() == "id" {
                        quote_spanned! {f.span() =>
                            pub free_ids: database::Vec<#ty>,
                            pub #name: database::Vec<#ty>,
                        }
                    } else {
                        quote_spanned! {f.span() =>
                            pub #name: database::Vec<#ty>,
                        }
                    }
                });
                quote! {
                    #(#recurse)*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

fn new(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span() =>
                        #name: database::Vec::new(database.clone()),
                    }
                });
                quote! {
                    #(#recurse)*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

fn serialize(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span() =>
                        self.#name.serialize(&mut writer)?;
                    }
                });
                quote! {
                    #(#recurse)*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

fn deserialize(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span() =>
                        #name: database::Vec::deserialize(&mut reader, database.clone())?,
                    }
                });
                quote! {
                    #(#recurse)*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

fn push(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span() =>
                        self.#name.write().push(value.#name);
                    }
                });
                quote! {
                    #(#recurse)*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

fn write(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span() =>
                        self.#name.write()[index] = value.#name;
                    }
                });
                quote! {
                    #(#recurse)*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}
