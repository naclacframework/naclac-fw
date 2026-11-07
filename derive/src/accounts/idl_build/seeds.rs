//! Turns the seed expressions of a `#[account(seeds = [...])]` PDA into the
//! `idl-build` tokens that describe them: a literal, an instruction argument,
//! an account field, or a spliced constant.

use {crate::accounts::field::parser, quote::quote};

/// Tokens constructing, at the calling crate's own `idl-build` runtime, the
/// `IdlSeedBuild::Const` of an unnamed literal seed whose bytes are known at
/// macro-expansion time.
fn literal_seed_tokens(value: &[u8]) -> proc_macro2::TokenStream {
    quote! {
        naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Const {
            value: vec![#(#value),*],
            name: None,
        }
    }
}

/// A seed expression this macro cannot prove is an instruction arg or an
/// account field of this struct is spliced into the generated `idl-build` code
/// as real Rust source, so the calling crate's compiler resolves it and its
/// real value lands in the JSON. The seed carries `name` as its constant name
/// only when it names a constant of this crate; a framework constant such as
/// `SYSTEM_PROGRAM_ID` is reported unnamed.
pub(super) fn splice_const_fallback(
    expr: &syn::Expr,
    name: Option<&str>,
) -> proc_macro2::TokenStream {
    let is_crate_constant = |candidate: &str| {
        crate::common::crate_source::crate_constants()
            .iter()
            .any(|constant| constant.name == candidate)
    };
    let name_tokens = match name.filter(|candidate| is_crate_constant(candidate)) {
        Some(n) => quote! { Some(#n.into()) },
        None => quote! { None },
    };
    quote! {
        naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Const {
            value: AsRef::<[u8]>::as_ref(&(#expr)).to_vec(),
            name: #name_tokens,
        }
    }
}

/// Classifies a named (non-literal) seed as an instruction-arg reference, an
/// account-field reference, or (falling through) a spliced constant, using
/// only this struct's own parsed fields. An account seed's `account_type` is
/// filled in when the path is a dotted subfield access and the field's own
/// `Account<'info, T>` parameter is known; `#[program]`'s assembler resolves
/// the subfield's primitive type from that name later.
fn classify_named_seed(
    path: &str,
    root: &str,
    original_expr: &syn::Expr,
    account_names: &[String],
    instr_names: &[syn::Ident],
    parsed_fields: &[parser::ParsedField],
) -> proc_macro2::TokenStream {
    if instr_names.iter().any(|n| n == root) {
        return quote! { naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Arg { path: #path.into() } };
    }
    if account_names.iter().any(|n| n == root) || root.ends_with("_account") {
        let account_type_tokens = if path.contains('.') {
            let ty_name = parsed_fields
                .iter()
                .find(|f| f.ident == root)
                .and_then(|f| crate::type_classify::first_generic_type(&f.ty))
                .and_then(crate::type_classify::base_ident)
                .map(|id| id.to_string());
            match ty_name {
                Some(name) => quote! { Some(#name.into()) },
                None => quote! { None },
            }
        } else {
            quote! { None }
        };
        return quote! {
            naclac_lang::naclac_idl::idl_build_accounts::IdlSeedBuild::Account {
                path: #path.into(),
                account_type: #account_type_tokens,
            }
        };
    }
    splice_const_fallback(original_expr, Some(root))
}

/// Builds the `idl-build` tokens for one seed expression: literal, path,
/// method-call, field-access, reference and single-element array, reusing
/// `naclac_syn::pda`'s literal-classification and path-extraction helpers.
pub(super) fn build_idl_seed_expr(
    expr: &syn::Expr,
    account_names: &[String],
    instr_names: &[syn::Ident],
    parsed_fields: &[parser::ParsedField],
) -> proc_macro2::TokenStream {
    match expr {
        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::ByteStr(b) => literal_seed_tokens(&b.value()),
            syn::Lit::Str(s) => literal_seed_tokens(s.value().as_bytes()),
            syn::Lit::Int(int_lit) => {
                literal_seed_tokens(&naclac_syn::pda::int_literal_seed_byte(int_lit))
            }
            _ => splice_const_fallback(expr, None),
        },
        syn::Expr::Path(expr_path) => {
            let Some(segment) = expr_path.path.segments.last() else {
                return syn::Error::new_spanned(
                    expr_path,
                    "Naclac Error: a seed path must have at least one segment.",
                )
                .to_compile_error();
            };
            let ident = segment.ident.to_string();
            classify_named_seed(
                &ident,
                &ident,
                expr,
                account_names,
                instr_names,
                parsed_fields,
            )
        }
        syn::Expr::MethodCall(expr_method) => {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(int_lit),
                ..
            }) = &*expr_method.receiver
            {
                let method = expr_method.method.to_string();
                if method == "to_le_bytes" || method == "to_be_bytes" {
                    return literal_seed_tokens(&naclac_syn::pda::int_literal_seed_bytes(
                        int_lit,
                        method == "to_be_bytes",
                    ));
                }
            }
            if let Some(path) = naclac_syn::pda::extract_field_path(&expr_method.receiver) {
                let root = path
                    .split_once('.')
                    .map_or(path.as_str(), |(root, _)| root)
                    .to_string();
                classify_named_seed(
                    &path,
                    &root,
                    expr,
                    account_names,
                    instr_names,
                    parsed_fields,
                )
            } else {
                let root = naclac_syn::pda::extract_root_ident(&expr_method.receiver);
                classify_named_seed(
                    &root,
                    &root,
                    expr,
                    account_names,
                    instr_names,
                    parsed_fields,
                )
            }
        }
        syn::Expr::Field(expr_field) => {
            if let Some(path) = naclac_syn::pda::extract_field_path(expr) {
                let root = path
                    .split_once('.')
                    .map_or(path.as_str(), |(root, _)| root)
                    .to_string();
                classify_named_seed(
                    &path,
                    &root,
                    expr,
                    account_names,
                    instr_names,
                    parsed_fields,
                )
            } else {
                let field_name = quote::ToTokens::to_token_stream(&expr_field.member).to_string();
                let root = naclac_syn::pda::extract_root_ident(&expr_field.base);
                classify_named_seed(
                    &field_name,
                    &root,
                    expr,
                    account_names,
                    instr_names,
                    parsed_fields,
                )
            }
        }
        syn::Expr::Reference(expr_ref) => {
            build_idl_seed_expr(&expr_ref.expr, account_names, instr_names, parsed_fields)
        }
        syn::Expr::Array(expr_array) if expr_array.elems.len() == 1 => build_idl_seed_expr(
            &expr_array.elems[0],
            account_names,
            instr_names,
            parsed_fields,
        ),
        _ => splice_const_fallback(expr, None),
    }
}

/// True if `expr` names any of `names` as a bare identifier. An account field
/// or an instruction argument has no value when the IDL is built, so an
/// expression that refers to one cannot be evaluated into a fixed address.
pub(super) fn mentions_any_name(expr: &syn::Expr, names: &[String]) -> bool {
    struct Finder<'a> {
        names: &'a [String],
        found: bool,
    }
    impl<'ast> syn::visit::Visit<'ast> for Finder<'_> {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            if node
                .path
                .get_ident()
                .is_some_and(|ident| self.names.iter().any(|name| ident == name))
            {
                self.found = true;
            }
            syn::visit::visit_expr_path(self, node);
        }
    }
    let mut finder = Finder {
        names,
        found: false,
    };
    syn::visit::Visit::visit_expr(&mut finder, expr);
    finder.found
}
