//! `#[constraint_args]`: the setters a constraint plugin's `Args` struct needs
//! so `#[derive(Accounts)]` can build it from the keys written in an
//! `#[account(namespace::key = value)]` attribute.

use {
    proc_macro2::TokenStream,
    quote::quote,
    syn::{GenericArgument, ItemStruct, PathArguments, Type},
};

/// Keeps the struct unchanged and adds, for each field `key: Option<T>`, an
/// `#[inline(always)] fn key(self, value: T) -> Self` that sets it to
/// `Some(value)`. Every field must be a named `Option<..>`, since a key that
/// is not written must have an empty default.
pub fn expand(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "Naclac Error: #[constraint_args] does not take any arguments.",
        )
        .to_compile_error()
        .into();
    }
    let input = syn::parse_macro_input!(item as ItemStruct);
    match setters(&input) {
        Ok(setters) => quote! { #input #setters }.into(),
        Err(err) => {
            let err = err.to_compile_error();
            quote! { #input #err }.into()
        }
    }
}

fn setters(input: &ItemStruct) -> syn::Result<TokenStream> {
    let syn::Fields::Named(fields) = &input.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Naclac Error: #[constraint_args] needs a struct with named fields, one per key.",
        ));
    };
    let mut methods = Vec::new();
    for field in &fields.named {
        let Some(key) = &field.ident else {
            continue;
        };
        let Some(value_ty) = option_inner(&field.ty) else {
            return Err(syn::Error::new_spanned(
                &field.ty,
                format!(
                    "Naclac Error: constraint key `{key}` must have type `Option<..>`, so it can \
                     be left out of the attribute."
                ),
            ));
        };
        methods.push(quote! {
            #[inline(always)]
            pub fn #key(mut self, value: #value_ty) -> Self {
                self.#key = ::core::option::Option::Some(value);
                self
            }
        });
    }
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #(#methods)*
        }
    })
}

/// The `T` of a field written `Option<T>` (or `core::option::Option<T>`).
fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    if last.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    match args.args.first()? {
        GenericArgument::Type(inner) if args.args.len() == 1 => Some(inner),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::setters;

    fn expand(source: &str) -> syn::Result<String> {
        let input: syn::ItemStruct = syn::parse_str(source).unwrap();
        setters(&input).map(|tokens| tokens.to_string().replace(' ', ""))
    }

    #[test]
    fn each_option_field_gets_a_setter_taking_its_inner_type() {
        let source = "pub struct Args { pub mint: Option<Address>, pub decimals: Option<u8> }";
        let generated = expand(source).unwrap();
        let mint = "pubfnmint(mutself,value:Address)->Self";
        let decimals = "pubfndecimals(mutself,value:u8)->Self";
        let body = "self.mint=::core::option::Option::Some(value);self";

        assert!(generated.contains(mint), "{generated}");
        assert!(generated.contains(decimals), "{generated}");
        assert!(generated.contains(body), "{generated}");
        assert_eq!(generated.matches("#[inline(always)]").count(), 2);
    }

    #[test]
    fn generics_and_lifetimes_are_kept_on_the_impl() {
        let source = "pub struct Args<'a, T: Copy> { pub view: Option<&'a T> }";
        let generated = expand(source).unwrap();
        let header = "impl<'a,T:Copy>Args<'a,T>";

        assert!(generated.contains(header), "{generated}");
        assert!(generated.contains("value:&'aT"), "{generated}");
    }

    #[test]
    fn a_qualified_option_is_accepted() {
        let source = "pub struct Args { pub min: core::option::Option<u64> }";
        let generated = expand(source).unwrap();
        assert!(generated.contains("value:u64"), "{generated}");
    }

    #[test]
    fn a_field_that_is_not_an_option_is_rejected() {
        let error = expand("pub struct Args { pub min: u64 }").unwrap_err();
        let message = error.to_string();
        let expected = "`min` must have type `Option<..>`";
        assert!(message.contains(expected), "{message}");
    }

    #[test]
    fn a_tuple_struct_is_rejected() {
        let error = expand("pub struct Args(Option<u64>);").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("named fields"), "{message}");
    }
}
