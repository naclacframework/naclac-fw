//! `emit!` and `emit_cpi!`: one event-building expansion shared by both, so
//! the two macros accept exactly the same event syntax and differ only in how
//! the built event is sent.

use {
    proc_macro::TokenStream,
    proc_macro2::{Ident, Span, TokenStream as TokenStream2},
    quote::quote,
    syn::parse_macro_input,
};

/// Binds the event written in a macro call to `var`. A struct literal
/// `Event { a, b: 2 }` starts from `Default` and assigns each field through
/// `.into()`, so the event's hidden padding fields never need to be written.
/// Any other expression, including a literal with `..base` or a qualified
/// path, is taken as the event value unchanged.
fn build_event(event: syn::Expr, var: &Ident) -> TokenStream2 {
    match event {
        syn::Expr::Struct(literal) if literal.qself.is_none() && literal.rest.is_none() => {
            let path = &literal.path;
            let assignments = literal.fields.iter().map(|field| {
                let member = &field.member;
                let value = &field.expr;
                quote! { #var.#member = (#value).into(); }
            });
            quote! {
                let mut #var = <#path as core::default::Default>::default();
                #(#assignments)*
            }
        }
        other => quote! { let #var = #other; },
    }
}

/// Expansion of `emit!(..)`: logs the event with `sol_log_data` through its
/// `emit` method and evaluates to the event.
pub fn expand_emit(input: TokenStream) -> TokenStream {
    let event = parse_macro_input!(input as syn::Expr);
    let var = Ident::new("__event", Span::mixed_site());
    let build = build_event(event, &var);
    quote! {{
        #build
        #var.emit();
        #var
    }}
    .into()
}

/// Expansion of `emit_cpi!(..)`: sends the event as a signed self-CPI through
/// `naclac_lang::event::emit_cpi`, reading the handler's context from the
/// variable named `ctx` at the call site, and evaluates to the event.
pub fn expand_emit_cpi(input: TokenStream) -> TokenStream {
    let event = parse_macro_input!(input as syn::Expr);
    let var = Ident::new("__event", Span::mixed_site());
    let ctx = Ident::new("ctx", Span::call_site());
    let build = build_event(event, &var);
    quote! {{
        #build
        naclac_lang::event::emit_cpi(&#ctx, &#var);
        #var
    }}
    .into()
}
