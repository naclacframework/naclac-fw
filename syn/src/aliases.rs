//! Crate-local type aliases (`type Amount = u64;`) for the macros' IDL
//! assembly and type-name checks.
//!
//! Both read source syntax only, so without this a field of type `Amount` is
//! indistinguishable from a user-defined struct named `Amount`. [`Aliases`]
//! holds every non-generic alias of the crate and replaces a use of one by its
//! target, recursively. An alias is left alone when its name is ambiguous: two
//! aliases of that name with different targets, or a struct, enum or union of
//! the same name elsewhere in the crate.

use {
    crate::modules::CrateModule,
    std::collections::{HashMap, HashSet},
    syn::{visit_mut::VisitMut, Item, Type},
};

const MAX_ALIAS_DEPTH: usize = 16;

/// The resolvable type aliases of a crate.
#[derive(Default)]
pub struct Aliases {
    targets: HashMap<String, Type>,
}

impl Aliases {
    /// The aliases declared in `modules`.
    pub fn collect(modules: &[CrateModule]) -> Self {
        let mut declared_types: HashSet<String> = HashSet::new();
        let mut candidates: HashMap<String, Vec<Type>> = HashMap::new();
        for item in modules.iter().flat_map(|module| module.items.iter()) {
            match item {
                Item::Struct(item) => {
                    declared_types.insert(item.ident.to_string());
                }
                Item::Enum(item) => {
                    declared_types.insert(item.ident.to_string());
                }
                Item::Union(item) => {
                    declared_types.insert(item.ident.to_string());
                }
                Item::Type(item) if item.generics.params.is_empty() => {
                    candidates
                        .entry(item.ident.to_string())
                        .or_default()
                        .push((*item.ty).clone());
                }
                _ => {}
            }
        }
        let targets = candidates
            .into_iter()
            .filter(|(name, _)| !declared_types.contains(name))
            .filter_map(|(name, mut targets)| {
                let first = targets.pop()?;
                let agree = targets.iter().all(|other| *other == first);
                agree.then_some((name, first))
            })
            .collect();
        Self { targets }
    }

    /// `ty` with every use of an alias replaced by its target.
    pub fn resolve(&self, ty: &Type) -> Type {
        let mut resolved = ty.clone();
        self.resolver().visit_type_mut(&mut resolved);
        resolved
    }

    /// Replaces the uses of aliases in the items of `modules` in place.
    pub fn resolve_modules(&self, modules: &mut [CrateModule]) {
        if self.targets.is_empty() {
            return;
        }
        let mut resolver = self.resolver();
        for item in modules
            .iter_mut()
            .flat_map(|module| module.items.iter_mut())
        {
            resolver.visit_item_mut(item);
        }
    }

    fn resolver(&self) -> Resolver<'_> {
        Resolver {
            aliases: self,
            depth: 0,
        }
    }
}

struct Resolver<'a> {
    aliases: &'a Aliases,
    depth: usize,
}

impl VisitMut for Resolver<'_> {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(path) = ty {
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].arguments.is_empty()
                && self.depth < MAX_ALIAS_DEPTH
            {
                if let Some(target) = self
                    .aliases
                    .targets
                    .get(&path.path.segments[0].ident.to_string())
                {
                    let mut resolved = target.clone();
                    self.depth += 1;
                    self.visit_type_mut(&mut resolved);
                    self.depth -= 1;
                    *ty = resolved;
                    return;
                }
            }
        }
        syn::visit_mut::visit_type_mut(self, ty);
    }
}
