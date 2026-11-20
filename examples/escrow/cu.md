# Escrow: compute units and binary sizes

Compute units are from `naclac profile`. The tests use ids whose escrow PDA bump is 255 (one
search attempt), so `make` is identical in both flows.

Sizes are of the deployed (stripped) `.so`, built with
`cargo build-sbf --tools-version v1.58 --arch v3`, the flags the naclac CLI passes. The exact
size in KB is the byte count divided by 1024. File Explorer rounds up to the next whole KB, so
it shows slightly more.

## escrow

Pinocchio runtime, bytemuck layout.

- make: 3704 CU
- cancel: 3270 CU
- take: 4738 CU
- Size: 25,120 bytes (24.53 KB). File Explorer shows 25 KB.

## escrow_zc

Solana runtime, bytemuck layout.

- make: 5294 CU
- cancel: 5135 CU
- take: 7541 CU
- Size: 31,664 bytes (30.92 KB). File Explorer shows 31 KB.

## escrow_borsh

Solana runtime, borsh layout.

- make: 6029 CU
- cancel: 5826 CU
- take: 8441 CU
- Size: 60,448 bytes (59.03 KB). File Explorer shows 60 KB.

## escrow_wincode

Solana runtime, wincode layout.

- make: 5918 CU
- cancel: 5683 CU
- take: 8345 CU
- Size: 39,480 bytes (38.55 KB). File Explorer shows 39 KB.

## escrow_wincode_pinocchio

Pinocchio runtime, wincode layout.

- make: 4181 CU
- cancel: 3690 CU
- take: 5428 CU
- Size: 32,816 bytes (32.05 KB). File Explorer shows 33 KB.
