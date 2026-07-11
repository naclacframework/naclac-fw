mod helper;

use naclac_client::utils::TOKEN_2022_PROGRAM_ID;

#[test]
fn test_token_2022_vault_lifecycle() {
    helper::run_lifecycle_test(TOKEN_2022_PROGRAM_ID);
}
