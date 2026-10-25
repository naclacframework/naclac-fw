//! The order in which `#[derive(Accounts)]` loads and validates its fields.
//! A field whose load-time constraints name another field of the struct is
//! loaded after that field, so the generated code can refer to its already
//! validated local whatever the declaration order. Account slots, the IDL and
//! error codes keep using declaration order.

use {
    crate::instruction::parser::{ParsedField, PdaBump},
    std::collections::BTreeSet,
    syn::visit::Visit,
};

struct PathIdents<'a> {
    names: &'a mut BTreeSet<String>,
}

impl<'ast> Visit<'ast> for PathIdents<'_> {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.qself.is_none() {
            if let Some(ident) = node.path.get_ident() {
                self.names.insert(ident.to_string());
            }
        }
        syn::visit::visit_expr_path(self, node);
    }
}

/// The constraint expressions evaluated while `field` itself is being loaded.
/// Relations (`field = target`), `close` and realloc are evaluated after
/// every field has loaded, so they never constrain the load order.
fn load_time_exprs(field: &ParsedField) -> Vec<&syn::Expr> {
    let mut exprs: Vec<&syn::Expr> = Vec::new();
    if let Some(seeds) = &field.pda_seed {
        exprs.extend(seeds.elems.iter());
    }
    if let Some(PdaBump::Explicit(bump)) = &field.pda_bump {
        exprs.push(bump);
    }
    if let Some(init) = &field.init_config {
        exprs.push(&init.payer);
        exprs.extend(init.space.iter());
    }
    if let Some(extra) = &field.extra_accounts {
        exprs.extend(extra.elems.iter());
    }
    exprs.extend(field.plugin_constraints.iter().map(|c| &c.value));
    exprs.extend(
        [
            &field.pda_program,
            &field.owner,
            &field.address,
            &field.token_mint,
            &field.token_authority,
            &field.token_program,
            &field.mint_decimals,
            &field.mint_authority,
            &field.mint_freeze_authority,
            &field.associated_token_mint,
            &field.associated_token_authority,
            &field.associated_token_bump,
        ]
        .into_iter()
        .flatten(),
    );
    exprs
}

/// Indexes into `fields` in the order their loaders must run: each field
/// after every other field its load-time constraint expressions name, and
/// otherwise in declaration order. A name that merely matches a field adds an
/// ordering constraint and never changes what is checked. Fails when those
/// references form a cycle, which no load order can satisfy.
pub fn load_order(fields: &[ParsedField]) -> Result<Vec<usize>, syn::Error> {
    let names: Vec<String> = fields.iter().map(|f| f.ident.to_string()).collect();
    let deps: Vec<Vec<usize>> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let mut used = BTreeSet::new();
            let mut visitor = PathIdents { names: &mut used };
            for expr in load_time_exprs(field) {
                visitor.visit_expr(expr);
            }
            names
                .iter()
                .enumerate()
                .filter(|(j, name)| *j != i && used.contains(*name))
                .map(|(j, _)| j)
                .collect()
        })
        .collect();

    let mut placed = vec![false; fields.len()];
    let mut order = Vec::with_capacity(fields.len());
    while order.len() < fields.len() {
        let ready = (0..fields.len()).find(|&i| !placed[i] && deps[i].iter().all(|&d| placed[d]));
        if let Some(i) = ready {
            placed[i] = true;
            order.push(i);
            continue;
        }

        let Some(start) = (0..fields.len()).find(|&i| !placed[i]) else {
            break;
        };
        let mut path = vec![start];
        let mut current = start;
        let cycle = loop {
            let Some(next) = deps[current].iter().copied().find(|&d| !placed[d]) else {
                break vec![current];
            };
            if let Some(pos) = path.iter().position(|&p| p == next) {
                break path[pos..].to_vec();
            }
            path.push(next);
            current = next;
        };
        let listed: Vec<String> = cycle.iter().map(|&i| format!("`{}`", names[i])).collect();
        return Err(syn::Error::new_spanned(
            &fields[cycle[0]].ident,
            format!(
                "Naclac Error: the account constraints of {} refer to each other in a cycle, \
                 so none of them can be loaded first. Remove one of these references.",
                listed.join(", ")
            ),
        ));
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use {super::load_order, crate::instruction::parser::parse_struct_fields};

    fn order_of(fields: &str) -> Result<Vec<usize>, String> {
        let item: syn::ItemStruct = syn::parse_str(&format!("struct S {{ {fields} }}")).unwrap();
        let syn::Fields::Named(named) = &item.fields else {
            panic!("a struct with named fields");
        };
        let parsed = parse_struct_fields(named).unwrap();
        assert!(
            parsed.iter().all(|f| f.parse_error.is_none()),
            "every test attribute must parse"
        );
        load_order(&parsed).map_err(|e| e.to_string())
    }

    #[test]
    fn unrelated_fields_keep_declaration_order() {
        assert_eq!(
            order_of("a: Account<A>, b: Account<B>, c: Signer").unwrap(),
            [0, 1, 2]
        );
    }

    #[test]
    fn a_seed_reading_a_later_fields_data_loads_that_field_first() {
        let order = order_of(
            "#[account(init, payer = payer, seeds = [SEED, registry.bump.to_le_bytes().as_ref()], bump)]
             child: Account<Child>,
             #[account(mut)] payer: Signer,
             registry: Account<Registry>",
        )
        .unwrap();
        assert_eq!(order, [1, 2, 0]);
    }

    #[test]
    fn a_seed_using_a_later_fields_address_loads_that_field_first() {
        let order = order_of(
            "#[account(seeds = [SEED, registry.address().as_ref()], bump)] entry: Account<Entry>,
             registry: Account<Registry>",
        )
        .unwrap();
        assert_eq!(order, [1, 0]);
    }

    #[test]
    fn references_to_earlier_fields_change_nothing() {
        let order = order_of(
            "#[account(mut)] payer: Signer,
             registry: Account<Registry>,
             #[account(init, payer = payer, seeds = [SEED, registry.address().as_ref()], bump)]
             entry: Account<Entry>",
        )
        .unwrap();
        assert_eq!(order, [0, 1, 2]);
    }

    #[test]
    fn a_token_constraint_loads_the_account_it_names_first() {
        let order = order_of(
            "#[account(mut, token::mint = mint)] vault: Account<TokenAccount>,
             mint: Account<Mint>",
        )
        .unwrap();
        assert_eq!(order, [1, 0]);
    }

    #[test]
    fn a_plugin_constraint_loads_the_account_it_names_first() {
        let order = order_of(
            "#[account(my_vault::admin = admin)] vault: Account<Vault>,
             admin: Signer",
        )
        .unwrap();
        assert_eq!(order, [1, 0]);
    }

    #[test]
    fn relations_are_checked_after_loading_and_never_order_fields() {
        let order = order_of(
            "#[account(maker = maker)] escrow: Account<Escrow>,
             maker: Signer",
        )
        .unwrap();
        assert_eq!(order, [0, 1]);
    }

    #[test]
    fn a_field_naming_itself_is_not_a_cycle() {
        let order = order_of(
            "#[account(mut, seeds = [SEED, a.bump.to_le_bytes().as_ref()], bump = a.bump)]
             a: Account<A>",
        )
        .unwrap();
        assert_eq!(order, [0]);
    }

    #[test]
    fn a_chain_of_later_fields_is_fully_ordered() {
        let order = order_of(
            "#[account(seeds = [SEED, b.address().as_ref()], bump)] a: Account<A>,
             #[account(seeds = [SEED, c.address().as_ref()], bump)] b: Account<B>,
             c: Account<C>",
        )
        .unwrap();
        assert_eq!(order, [2, 1, 0]);
    }

    #[test]
    fn a_cycle_is_a_clear_error_naming_the_fields() {
        let message = order_of(
            "#[account(seeds = [SEED, b.address().as_ref()], bump)] a: Account<A>,
             #[account(seeds = [SEED, a.address().as_ref()], bump)] b: Account<B>",
        )
        .unwrap_err();
        assert!(message.contains("cycle"), "{message}");
        assert!(
            message.contains("`a`") && message.contains("`b`"),
            "{message}"
        );
    }
}
