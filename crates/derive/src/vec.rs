use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput};

pub fn vec_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let vec_name = Ident::new(&format!("{}Vec", name), Span::call_site());
    let members = members(&input.data);
    let new = new(&input.data);
    let update = update(&input.data);
    let insert = insert(&input.data);
    let swap_remove = swap_remove(&input.data, &name);

    let expanded = quote! {
        pub struct #vec_name {
            #members
            pub id: std::vec::Vec<usize>,
            pub index: std::collections::HashMap<usize, usize>,
        }

        impl #vec_name {

            pub fn new() -> Self {
                Self {
                    #new
                    id: std::vec::Vec::new(),
                    index: std::collections::HashMap::new()
                }
            }

            pub fn insert(&mut self, id: usize, mut value: #name) -> Option<#name> {
                use std::collections::hash_map::Entry;
                use std::option::Option;
                match self.index.entry(id) {
                    Entry::Occupied(entry) => {
                        let index = *entry.get();
                        self.id[index] = id;
                        #update
                        Option::Some(value)
                    },
                    Entry::Vacant(entry) => {
                        let index = self.id.len();
                        entry.insert(index);
                        self.id.push(id);
                        #insert
                        Option::None
                    }
                }
            }

            pub fn range(&self) -> std::ops::Range<usize> {
                0..self.id.len()
            }

            pub fn remove_by_index(&mut self, index: usize) -> (usize, #name) {
                let id = self.id.swap_remove(index);
                self.index.remove(&id).unwrap();
                if index != self.id.len() {
                    self.index.insert(self.id[index], index);
                }
                (id, #swap_remove)
            }

            pub fn remove_by_id(&mut self, id: usize) -> Option<#name> {
                if let Some(index) = self.index.remove(&id) {
                    self.id.swap_remove(index);
                    if index != self.id.len() {
                        self.index.insert(self.id[index], index);
                    }
                    Some(#swap_remove)
                } else {
                    None
                }
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
                    quote_spanned! {f.span() =>
                        pub #name: std::vec::Vec<#ty>,
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
                        #name: std::vec::Vec::new(),
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

fn update(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span() =>
                        std::mem::swap(&mut self.#name[index], &mut value.#name);
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

fn insert(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span() =>
                        self.#name.push(value.#name);
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

fn swap_remove(data: &Data, name: &Ident) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote_spanned! {f.span() =>
                        #name: self.#name.swap_remove(index),
                    }
                });
                quote! {
                    #name {
                        #(#recurse)*
                    }
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}
