# Meow Coin

> The built-in system coin: `transfer`, `split`, `merge`, and `burn`.

Meow Coin is the native coin of MEOW. It is defined in the built-in `meow_coin` module, published at genesis at the fixed address `0x10`. Every address that appears in `allocations.csv` starts with one `MeowCoin` object whose `balance` equals the allocated amount.

See [Contracts](contracts.md) for the language reference and call argument format.

## The module

Source: [`crates/meow-framework/modules/meow_coin.meow`](../crates/meow-framework/modules/meow_coin.meow)

`mint` is called only at genesis and cannot be called by users. All other functions are available via `meow transaction meow-call`.

## Find your coins

```bash
meow client get-objects <YOUR_ADDRESS>
```

Your coin objects are of type `MeowCoin`. Note their addresses — you'll need them for all operations below.

## Transfer a whole coin

Moves the entire coin to another address.

```bash
meow transaction meow-call \
  --module 0x10 \
  --function transfer \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <COIN_ADDRESS> @<RECIPIENT_ADDRESS>

meow transaction sign <BASE64_TRANSACTION>
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

> `<COIN_ADDRESS>` and `<GAS_COIN_ADDRESS>` must be different objects. If you only have one coin, use `split` first to create a second one for gas.

## Send an amount to another address

Splits `amount` out of the coin and sends the new coin to the recipient. The original coin stays with you, reduced by `amount`.

```bash
meow transaction meow-call \
  --module 0x10 \
  --function split_and_transfer \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <COIN_ADDRESS> 500 @<RECIPIENT_ADDRESS>

meow transaction sign <BASE64_TRANSACTION>
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

## Split off an amount for yourself

Creates a new coin with `amount` balance and sends it to the transaction sender. Useful for creating a separate gas coin.

```bash
meow transaction meow-call \
  --module 0x10 \
  --function split \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <COIN_ADDRESS> 100

meow transaction sign <BASE64_TRANSACTION>
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

## Merge two coins

Adds the balance of `from` into `to` and destroys `from`. Both coins must be owned by the sender.

```bash
meow transaction meow-call \
  --module 0x10 \
  --function merge \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <FROM_COIN_ADDRESS> <TO_COIN_ADDRESS>

meow transaction sign <BASE64_TRANSACTION>
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

## Burn a coin

Destroys the coin permanently.

```bash
meow transaction meow-call \
  --module 0x10 \
  --function burn \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <COIN_ADDRESS>

meow transaction sign <BASE64_TRANSACTION>
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```
