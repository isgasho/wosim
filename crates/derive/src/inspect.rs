use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, Data, DeriveInput, GenericParam, Generics,
};

pub fn inspect_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = add_trait_bounds(input.generics);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let inspect = inspect(&input.data);

    let inspect_mut = inspect_mut(&input.data);

    let expanded = quote! {
        impl #impl_generics util::inspect::Inspect for #name #ty_generics #where_clause {
            fn inspect(&self, name: &str, inspector: &mut impl util::inspect::Inspector) {
                inspector.inspect(name, |inspector| {
                    #inspect
                })
            }

            fn inspect_mut(&mut self, name: &str, inspector: &mut impl util::inspect::Inspector) {
                inspector.inspect(name, |inspector| {
                    #inspect_mut
                })
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}

fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(util::inspect::Inspect))
        }
    }
    generics
}

fn inspect(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    let name_text = format!("{}", f.ident.as_ref().unwrap());
                    quote_spanned! {f.span() =>
                        util::inspect::Inspect::inspect(&self.#name, #name_text, inspector);
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

fn inspect_mut(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    let readonly = f.attrs.iter().any(|attr| attr.path.is_ident("inspect_readonly"));
                    if readonly {
                        quote_spanned! {f.span() =>
                            util::inspect::Inspect::inspect(&self.#name, "#name", inspector);
                        }
                    } else {
                        quote_spanned! {f.span() =>
                            util::inspect::Inspect::inspect_mut(&mut self.#name, "#name", inspector);
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
