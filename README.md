# modified_utf8

Modified UTF-8 encoding and decoding utilities.
Full documentation is available at [docs.rs/modified_utf8](https://docs.rs/modified_utf8).

## Example

```rust
use modified_utf8::{encode, decode};

let s = "Hello, world!";
let encoded = encode(s);
let decoded = decode(&encoded).unwrap();
assert_eq!(s, decoded);
```

The `decode` function returns a `Result<String, Error>`.
The `Error` type provides `valid_up_to()` and `error_len()` to help locate and diagnose invalid byte sequences.

```rust
use modified_utf8::decode;

let bytes = &[0b1000_0000, 0b0101_0010];
if let Err(e) = decode(bytes) {
    assert_eq!(e.valid_up_to(), 0);
    assert_eq!(e.error_len(), Some(1));
}
```

Use `decode_lossy` to replace invalid byte sequences with `�` (U+FFFD) instead of failing.

```rust
use modified_utf8::decode_lossy;

let bytes = &[0b1100_0000, 0b1000_0000, 0b1111_1111, 0b0101_0010];
let s = decode_lossy(bytes);
assert_eq!(s, "\0�R");
```

## License

This project is licensed under the MIT License.
