amuelhorjet@SamuelHorjet:/mnt/c/Users/hp/Documents/naclac-fw/examples/launchpad/launchpad$ naclac test
🧪 Running Naclac Test Suite...
   > cargo test --package amm -- --nocapture
   Compiling amm v0.1.0 (/mnt/c/Users/hp/Documents/naclac-fw/examples/launchpad/launchpad/programs/amm)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 39s
     Running unittests src/lib.rs (target/debug/deps/amm-bcb41a09bb94f74a)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/amm_test.rs (target/debug/deps/amm_test-0f85b14f36164038)

running 1 test

🔑 Loaded Payer Wallet: 28ArNHwi2Fg4WLfzdqWXrdaXg7hVZ1zam1bpd23hybWJ
   📦 Loaded AMM program binary.
   🪙 Created Token A Mint: Hi7stKxVNQJj4jJXoxWY7QgRp2xcqrRrRrUKTeJGqDj3
   🪙 Created Token B Mint: 3n9C15Gb4KJn3qVwZoXSnSRehfxwgRauvg6Tv7HEoK33
   💰 Minted initial tokens to user.
   🌊 Derived AMM Pool State PDA: FMqJUwmDrasQPQkNstmGugetznyGWvADKX6Je7YbwpFM
   🏦 Created AMM vault and LP mint accounts.
   🎟️ Created user LP ATA.
🔥 Sending initialize pool transaction...
   ✅ Pool initialized!
   📊 Compute Units (CUs) consumed: 11819
   📜 Program Logs:
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F invoke [1]
      Program 11111111111111111111111111111111 invoke [2]
      Program 11111111111111111111111111111111 success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 196551 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 195416 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 119 of 188754 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program data: ZHatVwzG/uU= +EOUiMHGcFncftIHmQIdkQJGaQO43NP62bF5Y27bvoYpR8U2NgYDu8ZmM7Px4fXmDJFnCospfamAyexsPlqV/nRflqgAAAAA
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F consumed 11819 of 200000 compute units
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F success
   📊 Initial balances verified. LP minted: 2828427124
🔥 Adding liquidity...
   ✅ Liquidity added successfully!
   📊 Compute Units (CUs) consumed: 4926
   📜 Program Logs:
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F invoke [1]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 198071 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 196936 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 119 of 195653 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program data: mhrdbO5A2aE= +EOUiMHGcFncftIHmQIdkQJGaQO43NP62bF5Y27bvoYpR8U2NgYDu8ZmM7Px4fXmDJFnCospfamAyexsPlqV/gBlzR0AAAAAAMqaOwAAAADdlyUqAAAAAA==
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F consumed 4926 of 200000 compute units
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F success
   📊 Post-add balances verified. LP count: 3535533905
🔥 Executing swap...
   ✅ Swap executed successfully!
   📊 Compute Units (CUs) consumed: 3871
   📜 Program Logs:
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F invoke [1]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 197927 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 196665 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program data: lqYa4RxZJk8= ELHkLhx1hR/WJEYxoo2GPF1I967AAj6n8LxuB01QZysA4fUFAAAAAPlTbwsAAAAA
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F consumed 3871 of 200000 compute units
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F success
   📊 Post-swap vault A balance: 2600000000
🔥 Removing liquidity...
   ✅ Liquidity removed successfully!
   📊 Compute Units (CUs) consumed: 4992
   📜 Program Logs:
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F invoke [1]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 123 of 198090 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 196780 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 195562 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program data: 4WnYJ3x0qb0= +EOUiMHGcFncftIHmQIdkQJGaQO43NP62bF5Y27bvoYpR8U2NgYDu8ZmM7Px4fXmDJFnCospfamAyexsPlqV/qaW6hUAAAAARZuHKAAAAAAAZc0dAAAAAA==
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F consumed 4992 of 200000 compute units
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F success
   📊 Final LP Balance: 3035533905

✨ AMM full lifecycle test completed successfully!

test test_amm_full_lifecycle ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.82s

   Doc-tests amm

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

🧪 Running Naclac Test Suite...
   > cargo test --package token_creator -- --nocapture
   Compiling token_creator v0.1.0 (/mnt/c/Users/hp/Documents/naclac-fw/examples/launchpad/launchpad/programs/token_creator)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 44s
     Running unittests src/lib.rs (target/debug/deps/token_creator-c1657c4f74e83a27)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/launchpad_test.rs (target/debug/deps/launchpad_test-66fa61a2ff7ce884)

running 1 test

🔑 Loaded Payer Wallet: 28ArNHwi2Fg4WLfzdqWXrdaXg7hVZ1zam1bpd23hybWJ
   📦 Loaded program binaries.
   🪙 Created Quote Mint: DTefsjUnFvNTjjfykUrZwZrZrxu4jPjTPRfccHJUsqqi
   💰 Minted quote tokens to payer.
   🪙 Derived Token A Mint PDA: 6E6bmz7nvSi3n4DrPwLuq6xAoRxrT7fnm11teQSraqYk
   📝 Derived Launch Record PDA: EXHyrFdj6LcgxYEFtx8bw6TK7djQKrw7fskHe8Zs6beK
   ✅ Mint created on-chain!
   🌊 Derived AMM Pool State PDA: HAArmVfsj21XwiSuVgM9RHyS3giDpG3XrDwPEKnUwkf6
   🏦 Created AMM vault and LP mint accounts.
   🚀 Created launcher vaults.
🔥 Sending launch_token transaction...
   📊 Compute Units (CUs) consumed: 19809
   📜 Program Logs:
      Program H1Hcpaf3iW9RHrc8bAYkrEcqfb3kTWRvgZceNBi8tWYX invoke [1]
      Program 11111111111111111111111111111111 invoke [2]
      Program 11111111111111111111111111111111 success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 196550 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 119 of 195284 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F invoke [2]
      Program 11111111111111111111111111111111 invoke [3]
      Program 11111111111111111111111111111111 success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 188971 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 76 of 187836 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [3]
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 119 of 181325 compute units
      Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success
      Program data: ZHatVwzG/uU= TaVRDaGDDmDVEQaLHT6MuRSyMEL7amuN/FCA3IeyMQm5H/AjngXzvPre+7PCT1jHQPIJMzXpm27iwXkiADowabovS1QAAAAA
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F consumed 11663 of 192397 compute units
      Program 6WCKBWWvuGYDHA14FXBbWMFeTQzHYmNR3p2oiLct4G1F success
      Program data: 4ei+k9XA3Kg= KgAAAAAAAABNpVENoYMOYNURBosdPoy5FLIwQvtqa438UIDch7IxCQDKmjsAAAAAAJQ1dwAAAAA=        
      Program H1Hcpaf3iW9RHrc8bAYkrEcqfb3kTWRvgZceNBi8tWYX consumed 19809 of 200000 compute units
      Program H1Hcpaf3iW9RHrc8bAYkrEcqfb3kTWRvgZceNBi8tWYX success
   ✅ State verified successfully!

✨ Launchpad integration test completed successfully!

test test_launchpad_integration ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s

   Doc-tests token_creator

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

✅ All tests passed successfully!