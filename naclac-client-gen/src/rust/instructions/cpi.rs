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
        cpi_content.push_str(&format!("pub struct {}CpiAccounts<'a> {{\n", ix_camel));
        for acc in &ix.accounts {
            let acc_snake = AsSnakeCase(&acc.name).to_string();
            let is_optional = acc.optional.unwrap_or(false);
            let handle_type = if acc.writable {
                "CpiHandleMut<'a>"
            } else {
                "CpiHandle<'a>"
            };
            if is_optional {
                cpi_content.push_str(&format!(
                    "    pub {}: Option<crate::sdk_core::{}>,\n",
                    acc_snake, handle_type
                ));
            } else {
                cpi_content.push_str(&format!(
                    "    pub {}: crate::sdk_core::{},\n",
                    acc_snake, handle_type
                ));
            }
        }
        cpi_content.push_str("}\n\n");
    }

    // 2.2 Extension Trait
    cpi_content.push_str("#[cfg(feature = \"cpi\")]\n");
    cpi_content.push_str(&format!("pub trait {}Cpi<'info> {{\n", ix_camel));

    let mut params = Vec::new();
    let mut params_signed = Vec::new();
    let mut params_remaining = Vec::new();

    let mut call_args = Vec::new();
    let mut call_args_signed = Vec::new();

    if !ix.accounts.is_empty() {
        let acc_type = format!("{}CpiAccounts<'a>", ix_camel);
        params.push(format!("accounts: {}", acc_type));
        params_signed.push(format!("accounts: {}", acc_type));
        params_remaining.push(format!("accounts: {}", acc_type));

        call_args.push("accounts".to_string());
        call_args_signed.push("accounts".to_string());
    }

    for arg in &ix.args {
        let arg_snake = heck::AsSnakeCase(&arg.name).to_string();
        let arg_ty =
            crate::rust::map_type_to_rust_with_prefix(&arg.ty, idl.is_zero_copy, "crate::types::");
        params.push(format!("{}: {}", arg_snake, arg_ty));
        params_signed.push(format!("{}: {}", arg_snake, arg_ty));
        params_remaining.push(format!("{}: {}", arg_snake, arg_ty));

        call_args.push(arg_snake.clone());
        call_args_signed.push(arg_snake);
    }

    // Add signer_seeds for signed call
    let call_args_signed_str = if call_args_signed.is_empty() {
        "&[], &[]".to_string()
    } else {
        format!("{}, signer_seeds, &[]", call_args_signed.join(", "))
    };

    let call_args_str = if call_args.is_empty() {
        "&[], &[]".to_string()
    } else {
        format!("{}, &[], &[]", call_args.join(", "))
    };

    cpi_content.push_str(&format!(
        "    fn {}<'a>(\n\
         \x20       &self,\n\
         \x20       {}\n\
         \x20   ) -> crate::sdk_core::Result<()> {{\n\
         \x20       self.{}_with_remaining_accounts({})\n\
         \x20   }}\n\n",
        ix_snake,
        params.join(",\n        "),
        ix_snake,
        call_args_str
    ));

    params_signed.push("signer_seeds: &[&[&[u8]]]".to_string());
    cpi_content.push_str(&format!(
        "    fn {}_signed<'a>(\n\
         \x20       &self,\n\
         \x20       {}\n\
         \x20   ) -> crate::sdk_core::Result<()> {{\n\
         \x20       self.{}_with_remaining_accounts({})\n\
         \x20   }}\n\n",
        ix_snake,
        params_signed.join(",\n        "),
        ix_snake,
        call_args_signed_str
    ));

    params_remaining.push("signer_seeds: &[&[&[u8]]]".to_string());
    params_remaining
        .push("remaining_accounts: &[(crate::sdk_core::AccountInfo, bool, bool)]".to_string());
    cpi_content.push_str(&format!(
        "    fn {}_with_remaining_accounts<'a>(\n\
         \x20       &self,\n\
         \x20       {}\n\
         \x20   ) -> crate::sdk_core::Result<()>;\n\n",
        ix_snake,
        params_remaining.join(",\n        ")
    ));

    cpi_content.push_str("}\n\n");

    // 2.3 Implementation Block
    cpi_content.push_str("#[cfg(feature = \"cpi\")]\n");
    cpi_content.push_str(&format!(
        "impl<'info> {}Cpi<'info> for crate::sdk_core::Program<crate::{}> {{\n",
        ix_camel, program_camel
    ));

    let mut params_remaining_impl = Vec::new();
    if !ix.accounts.is_empty() {
        params_remaining_impl.push(format!("accounts: {}CpiAccounts<'a>", ix_camel));
    }
    for arg in &ix.args {
        let arg_snake = heck::AsSnakeCase(&arg.name).to_string();
        let arg_ty =
            crate::rust::map_type_to_rust_with_prefix(&arg.ty, idl.is_zero_copy, "crate::types::");
        params_remaining_impl.push(format!("{}: {}", arg_snake, arg_ty));
    }
    params_remaining_impl.push("signer_seeds: &[&[&[u8]]]".to_string());
    params_remaining_impl
        .push("remaining_accounts: &[(crate::sdk_core::AccountInfo, bool, bool)]".to_string());

    cpi_content.push_str(&format!(
        "    fn {}_with_remaining_accounts<'a>(\n\
         \x20       &self,\n\
         \x20       {}\n\
         \x20   ) -> crate::sdk_core::Result<()> {{\n",
        ix_snake,
        params_remaining_impl.join(",\n        ")
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
            "        let args = {}IxArgs {{\n\
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
            "        let mut ix_data = crate::sdk_core::Vec::new();\n\
             \x20       ix_data.extend_from_slice(&[{}]);\n",
            disc_str
        )
    } else {
        format!(
            "        let mut ix_data = crate::sdk_core::Vec::new();\n\
             \x20       ix_data.extend_from_slice(&[{}]);\n\
             \x20       #[cfg(not(feature = \"borsh\"))]\n\
             \x20       {{\n\
             \x20           ix_data.extend_from_slice(crate::sdk_core::bytemuck::bytes_of(&args));\n\
             \x20       }}\n\
             \x20       #[cfg(feature = \"borsh\")]\n\
             \x20       {{\n\
             \x20           crate::sdk_core::borsh::BorshSerialize::serialize(&args, &mut ix_data).unwrap();\n\
             \x20       }}\n",
            disc_str
        )
    };

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
            pinocchio_metas.push(format!(
                "            if let Some(ref info) = accounts.{} {{\n\
                 \x20               metas_arr[idx].write(crate::sdk_core::pinocchio::instruction::InstructionAccount::{}(info.info.view.address()));\n\
                 \x20               idx += 1;\n\
                 \x20           }}",
                acc_snake, method
            ));
        } else {
            pinocchio_metas.push(format!(
                "            metas_arr[idx].write(crate::sdk_core::pinocchio::instruction::InstructionAccount::{}(accounts.{}.info.view.address()));\n\
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
    impl_body.push_str("            let mut metas_arr = [const { core::mem::MaybeUninit::<crate::sdk_core::pinocchio::instruction::InstructionAccount>::uninit() }; 32];\n");
    impl_body.push_str("            let mut idx = 0;\n");
    for meta in &pinocchio_metas {
        impl_body.push_str(&format!("{}\n", meta));
    }
    impl_body.push_str(
        "            for (acc, is_writable, is_signer) in remaining_accounts {\n\
         \x20               if idx >= 32 { break; }\n\
         \x20               let meta = if *is_writable {\n\
         \x20                   if *is_signer {\n\
         \x20                       crate::sdk_core::pinocchio::instruction::InstructionAccount::writable_signer(acc.view.address())\n\
         \x20                   } else {\n\
         \x20                       crate::sdk_core::pinocchio::instruction::InstructionAccount::writable(acc.view.address())\n\
         \x20                   }\n\
         \x20               } else {\n\
         \x20                   if *is_signer {\n\
         \x20                       crate::sdk_core::pinocchio::instruction::InstructionAccount::readonly_signer(acc.view.address())\n\
         \x20                   } else {\n\
         \x20                       crate::sdk_core::pinocchio::instruction::InstructionAccount::readonly(acc.view.address())\n\
         \x20                   }\n\
         \x20               };\n\
         \x20               metas_arr[idx].write(meta);\n\
         \x20               idx += 1;\n\
         \x20           }\n"
    );
    impl_body.push_str(
        "            let account_metas = unsafe { core::slice::from_raw_parts(metas_arr.as_ptr() as *const crate::sdk_core::pinocchio::instruction::InstructionAccount, idx) };\n\
         \x20           let program_id_address = self.address();\n\
         \x20           let instruction = crate::sdk_core::pinocchio::instruction::InstructionView {\n\
         \x20               program_id: program_id_address.as_address(),\n\
         \x20               accounts: account_metas,\n\
         \x20               data: &ix_data,\n\
         \x20           };\n"
    );
    impl_body.push_str("            let mut handles_arr = [const { core::mem::MaybeUninit::<crate::sdk_core::CpiHandle>::uninit() }; 32];\n");
    impl_body.push_str("            let mut h_idx = 0;\n");
    for acc in &ix.accounts {
        let acc_snake = heck::AsSnakeCase(&acc.name).to_string();
        let is_writable = acc.writable;
        if acc.optional.unwrap_or(false) {
            impl_body.push_str(&format!(
                "            if let Some(info) = accounts.{} {{\n",
                acc_snake
            ));
            if is_writable {
                impl_body.push_str("                handles_arr[h_idx].write(crate::sdk_core::CpiHandle::from(info));\n");
            } else {
                impl_body.push_str("                handles_arr[h_idx].write(info);\n");
            }
            impl_body.push_str("                h_idx += 1;\n            }\n");
        } else {
            if is_writable {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(crate::sdk_core::CpiHandle::from(accounts.{}));\n",
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
         \x20               handles_arr[h_idx].write(crate::sdk_core::ToCpiHandle::to_cpi_handle(acc));\n\
         \x20               h_idx += 1;\n\
         \x20           }\n"
    );
    impl_body.push_str("            let account_handles = unsafe { core::slice::from_raw_parts(handles_arr.as_ptr() as *const crate::sdk_core::CpiHandle, h_idx) };\n");
    impl_body.push_str(
        "            crate::sdk_core::cpi::invoke_signed_pinocchio_handles(&instruction, account_handles, signer_seeds)\n\
         \x20       }\n"
    );

    // Solana CPI
    impl_body.push_str("        #[cfg(not(feature = \"pinocchio\"))]\n        {\n");
    impl_body.push_str("            use crate::sdk_core::ToAddress;\n");
    let mut initial_metas = Vec::new();
    for acc in &ix.accounts {
        if !acc.optional.unwrap_or(false) {
            let acc_snake = heck::AsSnakeCase(&acc.name).to_string();
            let is_signer = acc.signer;
            let is_writable = acc.writable;
            let method = if is_writable { "new" } else { "new_readonly" };
            initial_metas.push(format!(
                "::naclac_lang::solana_program::instruction::AccountMeta::{}(accounts.{}.address(), {})",
                method, acc_snake, is_signer
            ));
        }
    }
    if initial_metas.is_empty() {
        impl_body.push_str("            let mut account_metas = crate::sdk_core::Vec::new();\n");
    } else {
        impl_body.push_str(&format!(
            "            let mut account_metas = crate::sdk_core::vec![\n                {},\n            ];\n",
            initial_metas.join(",\n                ")
        ));
    }
    for acc in &ix.accounts {
        if acc.optional.unwrap_or(false) {
            let acc_snake = heck::AsSnakeCase(&acc.name).to_string();
            let is_signer = acc.signer;
            let is_writable = acc.writable;
            let method = if is_writable { "new" } else { "new_readonly" };
            impl_body.push_str(&format!(
                "            if let Some(ref info) = accounts.{} {{\n\
                 \x20               account_metas.push(::naclac_lang::solana_program::instruction::AccountMeta::{}(info.address(), {}));\n\
                 \x20           }}\n",
                acc_snake, method, is_signer
            ));
        }
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
    impl_body.push_str("            let mut handles_arr = [const { core::mem::MaybeUninit::<crate::sdk_core::CpiHandle>::uninit() }; 32];\n");
    impl_body.push_str("            let mut h_idx = 0;\n");
    for acc in &ix.accounts {
        let acc_snake = heck::AsSnakeCase(&acc.name).to_string();
        let is_writable = acc.writable;
        if acc.optional.unwrap_or(false) {
            impl_body.push_str(&format!(
                "            if let Some(info) = accounts.{} {{\n",
                acc_snake
            ));
            if is_writable {
                impl_body.push_str("                handles_arr[h_idx].write(crate::sdk_core::CpiHandle::from(info));\n");
            } else {
                impl_body.push_str("                handles_arr[h_idx].write(info);\n");
            }
            impl_body.push_str("                h_idx += 1;\n            }\n");
        } else {
            if is_writable {
                impl_body.push_str(&format!(
                    "            handles_arr[h_idx].write(crate::sdk_core::CpiHandle::from(accounts.{}));\n",
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
         \x20               handles_arr[h_idx].write(crate::sdk_core::ToCpiHandle::to_cpi_handle(acc));\n\
         \x20               h_idx += 1;\n\
         \x20           }\n"
    );
    impl_body.push_str("            let account_handles = unsafe { core::slice::from_raw_parts(handles_arr.as_ptr() as *const crate::sdk_core::CpiHandle, h_idx) };\n");
    impl_body.push_str(
        "            crate::sdk_core::cpi::invoke_signed(&instruction, account_handles, signer_seeds)\n\
         \x20       }\n"
    );

    cpi_content.push_str(&impl_body);
    cpi_content.push_str("    }\n}\n\n");

    cpi_content
}
