mod helper;

use naclac_client::utils::TOKEN_PROGRAM_ID;

#[test]
fn test_token_vault_lifecycle() {
    helper::run_lifecycle_test(TOKEN_PROGRAM_ID);
}
