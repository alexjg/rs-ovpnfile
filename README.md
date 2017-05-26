# ovpnfile [![Build Status](https://travis-ci.org/alexjg/rs-ovpnfile.svg?branch=master)](https://travis-ci.org/alexjg/rs-ovpnfile) [![Crates.io](https://img.shields.io/crates/v/ovpnfile.svg)](https://crates.io/crates/ovpnfile) [![Docs](https://docs.rs/ovpnfile/badge.svg)](https://docs.rs/ovpnfile/0.1.0/ovpnfile/)

This is a tiny library for parsing openvpn config files as documented [here](https://community.openvpn.net/openvpn/wiki/Openvpn24ManPage).

## Usage

Install the thing

    cargo install ovpnfile

Use the thing

```rust
use ovpnfile;
let mut file = File::open("myovpnfile.ovpn").unwrap();
let parsed_file = ovpnfile::parse(file).unwrap();
let first_line = parsed_file.success_lines[0];
assert!(first_line.line_no == 5) //or whatever line the first succesfully parsed directive was on
assert!(first_line.directive == ovpnfile::ConfigDirective::Nobind)
```

See the documentation for more details.


## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

