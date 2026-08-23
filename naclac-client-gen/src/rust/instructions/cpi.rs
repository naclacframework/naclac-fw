use crate::{Idl, IdlInstruction};
use heck::{AsSnakeCase, ToUpperCamelCase};

pub fn generate_cpi_ix(ix: &IdlInstruction, idl: &Idl) -> String {
    let mut cpi_content = String::new();
    let ix_camel = ix.name.to_upper_camel_case();
    let ix_snake = AsSnakeCase(&ix.name).to_string();
    let program_camel = idl.metadata.name.to_upper_camel_case();

    // 2.1 CPI Accounts Struct (lifetime-free, Copy/Clone under Pinocchio)
    if !ix.accounts.is_empty() {
        cpi_content.push_str("#[cfg(feature = \"cpi\")]\n");
        cpi_content.push_str(&crate::rust::render_docs(&ix.docs, ""));
        cpi_content.push_str(&format!("pub struct {}CpiAccounts<'a> {{\n", ix_camel));
        for acc in &ix.accounts {
            let acc_snake = AsSnakeCase(&acc.name).to_string();
            let is_optional = acc.optional.unwrap_or(false);
            let handle_type = if acc.writable {
                "CpiHandleMut<'a>"
            } else {
                "CpiHandle<'a>"
            };
            cpi_content.push_str(&crate::rust::render_docs(&acc.docs, "    "));
            if is_optional {
                cpi_content.push_str(&format!(
                    "    pub {}: Option<crate::sdk_core_cpi::{}>,\n",
                    acc_snake, handle_type
                ));
            } else {
                cpi_content.push_str(&format!(
                    "    pub {}: crate::sdk_core_cpi::{},\n",
                    acc_snake, handle_type
                ));
            }
        }
        cpi_content.push_str("}\n\n");
    }

    // 2.2 Extension Trait
    cpi_content.push_str("#[cfg(feature = \"cpi\")]\n");
    cpi_content.push_str(&format!("pub struct {}CpiCall<'a> {{\n", ix_camel));
    if !ix.accounts.is_empty() {
        cpi_content.push_str(&format!("    pub accounts: {}CpiAccounts<'a>,\n", ix_camel));
    }
    for arg in &ix.args {
        let arg_snake = heck::AsSnakeCase(&arg.name).to_string();
        let arg_ty = crate::rust::map_type_to_rust_cpi(&arg.ty, idl.is_zero_copy, "crate::types::");
        cpi_content.push_str(&format!("    pub {}: {},\n", arg_snake, arg_ty));
    }
    cpi_content.push_str("    pub signer_seeds: &'a [&'a [&'a [u8]]],\n");
    cpi_content.push_str(
        "    pub remaining_accounts: &'a [(crate::sdk_core_cpi::AccountInfo, bool, bool)],\n",
    );
    cpi_content.push_str("}\n\n");

    cpi_content.push_str("#[cfg(feature = \"cpi\")]\n");
    cpi_content.push_str(&format!("pub trait {}Cpi<'info> {{\n", ix_camel));

    let mut params = Vec::new();
    let mut params_signed = Vec::new();

    if !ix.accounts.is_empty() {
        let acc_type = format!("{}CpiAccounts<'a>", ix_camel);
        params.push(format!("accounts: {}", acc_type));
        params_signed.push(format!("accounts: {}", acc_type));
    }

    for arg in &ix.args {
        let arg_snake = heck::AsSnakeCase(&arg.name).to_string();
        let arg_ty = crate::rust::map_type_to_rust_cpi(&arg.ty, idl.is_zero_copy, "crate::types::");
        params.push(format!("{}: {}", arg_snake, arg_ty));
        params_signed.push(format!("{}: {}", arg_snake, arg_ty));
    }

    let mut call_fields = Vec::new();
    if !ix.accounts.is_empty() {
        call_fields.push("accounts".to_string());
    }
    for arg in &ix.args {
        call_fields.push(heck::AsSnakeCase(&arg.name).to_string());
    }

    let init_call_literal = {
        let field_lines = call_fields
            .iter()
            .map(|field| format!("        {},", field))
            .collect::<Vec<_>>()
            .join("\n");
        if field_lines.is_empty() {
            format!("{}CpiCall {{\n        signer_seeds: &[],\n        remaining_accounts: &[],\n    }}", ix_camel)
        } else {
            format!(
                "{}CpiCall {{\n{}\n        signer_seeds: &[],\n        remaining_accounts: &[],\n    }}",
                ix_camel,
                field_lines
            )
        }
    };

    let signed_call_literal = {
        let field_lines = call_fields
            .iter()
            .map(|field| format!("        {},", field))
            .collect::<Vec<_>>()
            .join("\n");
        if field_lines.is_empty() {
            format!(
                "{}CpiCall {{\n        signer_seeds,\n        remaining_accounts: &[],\n    }}",
                ix_camel
            )
        } else {
            format!(
                "{}CpiCall {{\n{}\n        signer_seeds,\n        remaining_accounts: &[],\n    }}",
                ix_camel, field_lines
            )
        }
    };

    cpi_content.push_str(&crate::rust::render_docs(&ix.docs, "    "));
    cpi_content.push_str(&format!(
        "    fn {}<'a>(\n\
         \x20       &self,\n\
         \x20       {}\n\
         \x20   ) -> crate::sdk_core_cpi::Result<()> {{\n\
         \x20       self.{}_with_remaining_accounts({})\n\
         \x20   }}\n\n",
        ix_snake,
        params.join(",\n        "),
        ix_snake,
        init_call_literal
    ));

    params_signed.push("signer_seeds: &[&[&[u8]]]".to_string());
    cpi_content.push_str(&format!(
        "    fn {}_signed<'a>(\n\
         \x20       &self,\n\
         \x20       {}\n\
         \x20   ) -> crate::sdk_core_cpi::Result<()> {{\n\
         \x20       self.{}_with_remaining_accounts({})\n\
         \x20   }}\n\n",
        ix_snake,
        params_signed.join(",\n        "),
        ix_snake,
        signed_call_literal
    ));

    cpi_content.push_str(&format!(
        "    fn {}_with_remaining_accounts<'a>(\n\
         \x20       &self,\n\
         \x20       call: {}CpiCall<'a>\n\
         \x20   ) -> crate::sdk_core_cpi::Result<()>;\n\n",
        ix_snake, ix_camel
    ));

    cpi_content.push_str("}\n\n");

    // 2.3 Implementation Block
    cpi_content.push_str("#[cfg(feature = \"cpi\")]\n");
    cpi_content.push_str(&format!(
        "impl<'info> {}Cpi<'info> for crate::sdk_core_cpi::Program<crate::{}> {{\n",
        ix_camel, program_camel
    ));

    cpi_content.push_str(&format!(
        "    fn {}_with_remaining_accounts<'a>(\n\
         \x20       &self,\n\
         \x20       call: {}CpiCall<'a>\n\
         \x20   ) -> crate::sdk_core_cpi::Result<()> {{\n",
        ix_snake, ix_camel
    ));

    let mut destructure_fields = Vec::new();
    if !ix.accounts.is_empty() {
        destructure_fields.push("accounts".to_string());
    }
    for arg in &ix.args {
        destructure_fields.push(heck::AsSnakeCase(&arg.name).to_string());
    }
    destructure_fields.push("signer_seeds".to_string());
    destructure_fields.push("remaining_accounts".to_string());
    cpi_content.push_str(&format!(
        "        let {}CpiCall {{ {} }} = call;\n",
        ix_camel,
        destructure_fields.join(", ")
    ));

    // internal body
    let args_struct_init = if ix.args.is_empty() {
        "".to_string()
    } else {
        let mut arg_fields = Vec::new();
        for arg in &ix.args {
            let arg_snake = heck::AsSnakeCase(&arg.name).to_string();
            arg_fields.push(format!("            {},", arg_snake));
        }
        format!(
            "        let args = {}CpiIxArgs {{\n\
             {}\n\
             \x20       }};\n",
            ix_camel,
            arg_fields.join("\n")
        )
    };

    let disc_str = ix
        .discriminator
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let serializing_code = if ix.args.is_empty() {
        format!(
            "        let mut ix_data = crate::sdk_core_cpi::Vec::new();\n\
             \x20       ix_data.extend_from_slice(&[{}]);\n",
            disc_str
        )
    } else {
        let zc_writes = crate::rust::generate_zero_copy_arg_bytes(
            ix,
            true,
            idl.is_zero_copy,
            &idl.defined_types,
        );
        format!(
            "        let mut ix_data = crate::sdk_core_cpi::Vec::new();\n\
             \x20       ix_data.extend_from_slice(&[{}]);\n\
             \x20       #[cfg(not(feature = \"borsh\"))]\n\
             \x20       {{\n\
             {}\
             \x20       }}\n\
             \x20       #[cfg(feature = \"borsh\")]\n\
             \x20       {{\n\
             \x20           crate::sdk_core_cpi::borsh::BorshSerialize::serialize(&args, &mut ix_data).unwrap();\n\
             \x20       }}\n",
            disc_str, zc_writes
        )
    };

    let has_optional_accounts = ix.accounts.iter().any(|a| a.optional.unwrap_or(false));

    let mut pinocchio_metas = Vec::new();
    for acc in &ix.accounts {
        let acc_snake = heck::AsSnakeCase(&acc.name).to_string();
        let is_signer = acc.signer;
        let is_writable = acc.writable;

        let method = if is_writable {
            if is_signer {
                "writable_signer"
            } else {
                "writable"
            }
        } else {
            if is_signer {
                "readonly_signer"
            } else {
                "readonly"
            }
        };

        if acc.optional.unwrap_or(false) {
            // Sentinel scheme: the slot is always present. When the caller
            // passed `None`, fill it with the target program's own address
            // via `__cpi_self_handle` (a `CpiHandle` derived from `self`,
            // computed once above) — same field-access shape as the `Some`
            // case, and avoids returning a reference borrowed from a
            // temporary out of a closure. `self.address()` isn't used here
            // directly: on the pinocchio backend it returns naclac's
            // `Address` newtype, not the native pinocchio address type this
            // call site expects.
            pinocchio_metas.push(format!(
                "            metas_arr[idx].write(crate::sdk_core_cpi::pinocchio::instruction::InstructionAccount::{}(accounts.{}.as_ref().map(|info| info.info.view.address()).unwrap_or_else(|| __cpi_self_handle.info.view.address())));\n\
                 \x20           idx += 1;",
                method, acc_snake
            ));
        } else {
            pinocchio_metas.push(format!(
                "            metas_arr[idx].write(crate::sdk_core_cpi::pinocchio::instruction::InstructionAccount::{}(accounts.{}.info.view.address()));\n\
                 \x20           idx += 1;",
                method, acc_snake
            ));
        }
    }

    let mut impl_body = String::new();
    impl_body.push_str(&args_struct_init);
    impl_body.push_str(&serializing_code);

    // Pinocchio CPI
    impl_body.push_str("        #[cfg(feature = \"pinocchio\")]\n        {\n");
    impl_body.push_str("            let mut metas_arr = [const { core::mem::MaybeUninit::<crate::sdk_core_cpi::pinocchio::instruction::InstructionAccount>::uninit() }; 32];\n");
    impl_body.push_str("            let mut idx = 0;\n");
    if has_optional_accounts {
        impl_body.push_str("            let __cpi_self_handle: crate::sdk_core_cpi::CpiHandle = crate::sdk_core_cpi::ToCpiHandle::to_cpi_handle(self);\n");
    }
    for meta in &pinocchio_metas {
        impl_body.push_str(&format!("{}\n", meta));
    }
    impl_body.push_str(
        "            for (acc, is_writable, is_signer) in remaining_accounts {\n\
         \x20               if idx >= 32 { break; }\n\
         \x20               let meta = if *is_writable {\n\
         \x20                   if *is_signer {\n\
         \x20                       crate::sdk_core_cpi::pinocchio::instruction::InstructionAccount::writable_signer(acc.view.address())\n\
         \x20                   } else {\n\
         \x20                       crate::sdk_core_cpi::pinocchio::instruction::InstructionAccount::writable(acc.view.address())\n\
         \x20                   }\n\
         \x20               } else {\n\
         \x20                   if *is_signer {\n\
         \x20                       crate::sdk_core_cpi::pinocchio::instruction::InstructionAccount::readonly_signer(acc.view.address())\n\
         \x20                   } else {\n\
         \x20                       crate::sdk_core_cpi::pinocchio::instruction::InstructionAccount::readonly(acc.view.address())\n\
         \x20                   }\n\
         \x20               };\n\
         \x20               metas_arr[idx].write(meta);\n\
         \x20               idx += 1;\n\
         \x20           }\n"
    );
    impl_body.push_str(
        "            let account_metas = unsafe { core::slice::from_raw_parts(metas_arr.as_ptr() as *const crate::sdk_core_cpi::pinocchio::instruction::InstructionAccount, idx) };\n\
         \x20           let program_id_address = self.address();\n\
         \x20           let instruction = crate::sdk_core_cpi::pinocchio::instruction::InstructionView {\n\
         \x20               program_id: program_id_address.as_address(),\n\
         \x20               accounts: account_metas,\n\
         \x20               data: &ix_data,\n\
         \x20           };\n"
    );
    impl_body.push_str("            let mut handles_arr = [const { core::mem::MaybeUninit::<crate::sdk_core_cpi::CpiHandle>::uninit() }; 32];\n");
    impl_body.push_str("            let mut h_idx = 0;\n");
    for acc in &ix.accounts {
        let acc_snake = heck::AsSnakeCase(&acc.name).to_string();
        let is_writable = acc.writable;
        if acc.optional.unwrap_or(false) {
            // Must stay positionally aligned with the metas array above: a
            // slot written there (real or sentinel) always needs a matching
            // handle here, so a `None` still writes one — the target
            // program's own handle (`self`), matching the sentinel address.
            if is_writable {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(accounts.{}.map(crate::sdk_core_cpi::CpiHandle::from).unwrap_or_else(|| crate::sdk_core_cpi::ToCpiHandle::to_cpi_handle(self)));\n",
                    acc_snake
                ));
            } else {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(accounts.{}.unwrap_or_else(|| crate::sdk_core_cpi::ToCpiHandle::to_cpi_handle(self)));\n",
                    acc_snake
                ));
            }
            impl_body.push_str("            h_idx += 1;\n");
        } else {
            if is_writable {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(crate::sdk_core_cpi::CpiHandle::from(accounts.{}));\n",
                    acc_snake
                ));
            } else {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(accounts.{});\n",
                    acc_snake
                ));
            }
            impl_body.push_str("            h_idx += 1;\n");
        }
    }
    impl_body.push_str(
        "            for (acc, _, _) in remaining_accounts {\n\
         \x20               if h_idx >= 32 { break; }\n\
         \x20               handles_arr[h_idx].write(crate::sdk_core_cpi::ToCpiHandle::to_cpi_handle(acc));\n\
         \x20               h_idx += 1;\n\
         \x20           }\n"
    );
    impl_body.push_str("            let account_handles = unsafe { core::slice::from_raw_parts(handles_arr.as_ptr() as *const crate::sdk_core_cpi::CpiHandle, h_idx) };\n");
    impl_body.push_str(
        "            crate::sdk_core_cpi::cpi::invoke_signed_pinocchio_handles(&instruction, account_handles, signer_seeds)\n\
         \x20       }\n"
    );

    // Solana CPI
    impl_body.push_str("        #[cfg(not(feature = \"pinocchio\"))]\n        {\n");
    impl_body.push_str("            use crate::sdk_core_cpi::ToAddress;\n");
    // Every declared account gets exactly one meta, in declared order — same
    // sentinel-scheme requirement as the offchain builder above: an absent
    // optional account is filled with the target program's own address
    // (`self`), never omitted, since omitting it would shift every
    // subsequent account's position.
    let mut all_metas = Vec::new();
    for acc in &ix.accounts {
        let acc_snake = heck::AsSnakeCase(&acc.name).to_string();
        let is_signer = acc.signer;
        let is_writable = acc.writable;
        let method = if is_writable { "new" } else { "new_readonly" };
        let address_expr = if acc.optional.unwrap_or(false) {
            format!(
                "accounts.{}.as_ref().map(|info| info.address()).unwrap_or_else(|| self.address())",
                acc_snake
            )
        } else {
            format!("accounts.{}.address()", acc_snake)
        };
        all_metas.push(format!(
            "::naclac_lang::solana_program::instruction::AccountMeta::{}({}, {})",
            method, address_expr, is_signer
        ));
    }
    if all_metas.is_empty() {
        impl_body
            .push_str("            let mut account_metas = crate::sdk_core_cpi::Vec::new();\n");
    } else {
        impl_body.push_str(&format!(
            "            let mut account_metas = crate::sdk_core_cpi::vec![\n                {},\n            ];\n",
            all_metas.join(",\n                ")
        ));
    }
    impl_body.push_str(
        "            for (acc, is_writable, is_signer) in remaining_accounts {\n\
         \x20               let meta = if *is_writable {\n\
         \x20                   ::naclac_lang::solana_program::instruction::AccountMeta::new(acc.address(), *is_signer)\n\
         \x20               } else {\n\
         \x20                   ::naclac_lang::solana_program::instruction::AccountMeta::new_readonly(acc.address(), *is_signer)\n\
         \x20               };\n\
         \x20               account_metas.push(meta);\n\
         \x20           }\n"
    );
    impl_body.push_str(
        "            let instruction = ::naclac_lang::solana_program::instruction::Instruction {\n\
         \x20               program_id: self.address(),\n\
         \x20               accounts: account_metas,\n\
         \x20               data: ix_data,\n\
         \x20           };\n",
    );
    impl_body.push_str("            let mut handles_arr = [const { core::mem::MaybeUninit::<crate::sdk_core_cpi::CpiHandle>::uninit() }; 32];\n");
    impl_body.push_str("            let mut h_idx = 0;\n");
    for acc in &ix.accounts {
        let acc_snake = heck::AsSnakeCase(&acc.name).to_string();
        let is_writable = acc.writable;
        if acc.optional.unwrap_or(false) {
            // Must stay positionally aligned with the metas array above: a
            // slot written there (real or sentinel) always needs a matching
            // handle here, so a `None` still writes one — the target
            // program's own handle (`self`), matching the sentinel address.
            if is_writable {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(accounts.{}.map(crate::sdk_core_cpi::CpiHandle::from).unwrap_or_else(|| crate::sdk_core_cpi::ToCpiHandle::to_cpi_handle(self)));\n",
                    acc_snake
                ));
            } else {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(accounts.{}.unwrap_or_else(|| crate::sdk_core_cpi::ToCpiHandle::to_cpi_handle(self)));\n",
                    acc_snake
                ));
            }
            impl_body.push_str("            h_idx += 1;\n");
        } else {
            if is_writable {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(crate::sdk_core_cpi::CpiHandle::from(accounts.{}));\n",
                    acc_snake
                ));
            } else {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(accounts.{});\n",
                    acc_snake
                ));
            }
            impl_body.push_str("            h_idx += 1;\n");
        }
    }
    impl_body.push_str(
        "            for (acc, _, _) in remaining_accounts {\n\
         \x20               if h_idx >= 32 { break; }\n\
         \x20               handles_arr[h_idx].write(crate::sdk_core_cpi::ToCpiHandle::to_cpi_handle(acc));\n\
         \x20               h_idx += 1;\n\
         \x20           }\n"
    );
    impl_body.push_str("            let account_handles = unsafe { core::slice::from_raw_parts(handles_arr.as_ptr() as *const crate::sdk_core_cpi::CpiHandle, h_idx) };\n");
    impl_body.push_str(
        "            crate::sdk_core_cpi::cpi::invoke_signed(&instruction, account_handles, signer_seeds)\n\
         \x20       }\n"
    );

    cpi_content.push_str(&impl_body);
    cpi_content.push_str("    }\n}\n\n");

    cpi_content
}
