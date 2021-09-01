mod db_vec;
mod inspect;
mod vec;

#[proc_macro_derive(Inspect)]
pub fn inspect_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    inspect::inspect_macro(input)
}

#[proc_macro_derive(Vec)]
pub fn vec_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    vec::vec_macro(input)
}

#[proc_macro_derive(DbVec)]
pub fn db_vec_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    db_vec::db_vec_macro(input)
}
