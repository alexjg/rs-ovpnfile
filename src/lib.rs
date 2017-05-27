//! A tiny library for parsing openvpn config files
//!
//! openvpn config files have a very simple format. Arguments which would be
//! passed to the openvpn command line as `--<option> <arg1> <arg2>` are placed
//! on a line in the config file which looks like this `option arg1 arg2`.
//! Additionally some options can use a pseudo XML syntax to include the contents
//! of a file in the config file, for example
//!
//! ```text
//! <cert>
//! -----BEGIN CERTIFICATE-----
//! [...]
//! -----END CERTIFICATE-----
//! </cert>
//! ```
//!
//! All of these options are documented
//! [here](https://community.openvpn.net/openvpn/wiki/Openvpn24ManPage)
//!
//! This library represents each possible option as a separate variant  of the
//! `ConfigDirective` enum. Required arguments are
//! are represented as `String`s whilst optional arguments are represented as
//! `Option<String>`. There are a few exceptions which I will mention shortly.
//!
//! ## Usage
//! The entry point is the `ovpnfile::parse` function, which takes a `Read`
//! containing the config file and returns a `Result` which if successful will
//! be a `ParsedConfigFile`. Lines from the config file are represented as
//! `ConfigLine<T>` entries, where `T` is the parse result for that line.
//! For example
//!
//! ```
//! use std::io::{BufReader};
//! use ovpnfile::{ConfigDirective, ConfigLine, ParseWarning};
//! use ovpnfile;
//!
//! let contents = r"
//! resolv-retry 10
//! remote somehost someport
//! unknown-command
//! ".as_bytes();
//!
//! let reader = BufReader::new(contents);
//! let result = ovpnfile::parse(reader).unwrap();
//! assert!(result.success_lines == vec![
//!     ConfigLine{number: 1, result: ConfigDirective::ResolvRetry{n: "10".to_string()}},
//!     ConfigLine{number: 2, result: ConfigDirective::Remote{
//!         host: "somehost".to_string(),
//!         port: Some("someport".to_string()),
//!         proto: None,
//!     }},
//! ]);
//! assert!(result.warning_lines == vec![ConfigLine{number: 3, result: ParseWarning::NoMatchingCommand}]);
//! ```
//!
//! Lines which fail to parse either because the command is not recognized or
//! there are missing required arguments for the command result in warning, as
//! you can see from the above example.
//!
//! # Inline File Contents
//! As mentioned earlier some commands can include file contents inline in the
//! config file. These commands are:
//!
//! ```text
//! --ca
//! --ca
//! --cert
//! --extra-certs
//! --dh
//! --key
//! --pkcs12
//! --crl-verify
//! --http-proxy-user-pass
//! --tls-auth
//! --tls-crypt
//! --secret
//! ```
//!
//! The corresponding enum variants have a `file` record attribute which is an
//! instance of `File`. `File` is either an `InlineFileContents(String)` or a
//! `FilePath(String)`. So for example
//!
//! ```
//! use std::io::{BufReader};
//! use ovpnfile::{ConfigDirective, File, ConfigLine};
//! use ovpnfile;
//!
//! let contents = r"
//! tls-auth somefile somedirection
//! <tls-auth>
//! line1
//! line2
//! </tls-auth>
//! ".as_bytes();
//!
//! let reader = BufReader::new(contents);
//! let result = ovpnfile::parse(reader).unwrap();
//! assert!(result.success_lines == vec![
//!     ConfigLine{number: 1, result: ConfigDirective::TlsAuth{
//!         file: File::FilePath("somefile".to_string()),
//!         direction: Some("somedirection".to_string()),
//!     }},
//!     ConfigLine{number: 2, result: ConfigDirective::TlsAuth{
//!         file: File::InlineFileContents("line1\nline2".to_string()),
//!         direction: None,
//!     }},
//! ]);
//! ```
//!
//! # Server Bridge
//! The `--server-bridge` argument is special, it can take two forms
//!
//! ```rust
//! server-bridge gateway netmask pool-start-IP pool-end-IP
//! server-bridge nogw
//! ```
//!
//! This is represented in this library as the `ServerBridgeArg` enum variant, it
//! can either be a `NoGateway` or `GatewayConfig{gateway: String, netmask: String,
//! pool_start_ip: String, pool_end_ip: String}`.
//!
//!
//!
#![recursion_limit="1024"]
#[macro_use]
extern crate error_chain;
extern crate regex;
#[macro_use]
extern crate lazy_static;


use std::io::{BufRead, BufReader, Read};
use std::collections::HashSet;
use regex::Regex;

pub use self::config_directive::{ConfigDirective, ServerBridgeArg, File};
mod config_directive;

mod errors {
    error_chain!{}
}
use errors::ResultExt;

lazy_static! {
    static ref COMMENT_REGEX: Regex = Regex::new(r"#.*$").unwrap();
    static ref INLINE_START_REGEX: Regex = Regex::new(r"^<(\S+)>").unwrap();
    static ref INLINE_END_REGEX: Regex = Regex::new(r"^</(\S+)>").unwrap();
    static ref INLINE_FILE_OPTIONS: HashSet<&'static str> = {
        let mut s = HashSet::new();
        s.insert("ca");
        s.insert("ca");
        s.insert("cert");
        s.insert("extra-certs");
        s.insert("dh");
        s.insert("key");
        s.insert("pkcs12");
        s.insert("crl-verify");
        s.insert("http-proxy-user-pass");
        s.insert("tls-auth");
        s.insert("tls-crypt");
        s.insert("secret");
        s
    };
}

/// Represents a line of the config file, the type `T` will be either
/// a `ConfigDirective` or a `ParseWarning`.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ConfigLine<T> {
    pub number: i32,
    pub result: T,
}

/// Possible warnings
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ParseWarning {
    NotEnoughArguments,
    NoMatchingCommand,
}

/// The result of the `parse` function
pub struct ParsedConfigFile {
    pub success_lines: Vec<ConfigLine<ConfigDirective>>,
    pub warning_lines: Vec<ConfigLine<ParseWarning>>,
}

impl ParsedConfigFile {
    /// Get the succesfully parsed ConfigDirectives.
    pub fn directives(&self) -> Vec<ConfigDirective> {
        self.success_lines.iter().map(|l| l.result.clone()).collect()
    }
}

struct InlineFileParseState {
    start_line_no: i32,
    identifier: String,
    lines: Vec<String>,
}

impl InlineFileParseState {
    fn new(line_no: usize, identifier: String) -> InlineFileParseState {
        InlineFileParseState{
            start_line_no: line_no as i32,
            identifier: identifier,
            lines: Vec::new(),
        }
    }
    fn is_completed_by_line(&self, line: &str) -> bool {
        if let Some(end_identifier_captures) = INLINE_END_REGEX.captures(line) {
            return end_identifier_captures[1] == self.identifier
        }
        false
    }
    fn add_line(&mut self, line: String) {
        self.lines.push(line);
    }
    fn to_config_line(&self) -> ConfigLine<ConfigDirective> {
        let file = File::InlineFileContents(self.lines.join("\n"));
        let directive = match self.identifier.as_ref() {
            "ca" => ConfigDirective::Ca{file: file},
            "cert" => ConfigDirective::Cert{file: file},
            "extra-certs" => ConfigDirective::ExtraCerts{file: file},
            "dh" => ConfigDirective::Dh{file: file},
            "key" => ConfigDirective::Key{file: file},
            "pkcs12" => ConfigDirective::Pkcs12{file: file},
            "crl-verify" => ConfigDirective::CrlVerify{file: file, direction: None},
            "http-proxy-user-pass" => ConfigDirective::HttpProxyUserPass{file: file},
            "tls-auth" => ConfigDirective::TlsAuth{file: file, direction: None},
            "tls-crypt" => ConfigDirective::TlsCrypt{file: file},
            "secret" => ConfigDirective::Secret{file: file, direction: None},
            _ => unreachable!()
        };
        ConfigLine{result: directive, number: self.start_line_no as i32}
    }
}

/// The entry point for this library. Pass a `Read` containing the config file
/// and get back a `ParsedConfigFile`.
pub fn parse<R>(input: R) -> errors::Result<ParsedConfigFile> where R: Read {
    let buf_reader = BufReader::new(input);
    let mut success_lines = Vec::new();
    let mut warning_lines = Vec::new();
    let mut inline_file_parse_state: Option<InlineFileParseState> = None;
    for (line_index, line_result) in buf_reader.lines().enumerate() {
        let line_no = line_index;
        let line = line_result.chain_err(|| "Error reading input")?;

        let mut reset_inline_state = false;
        if let Some(ref mut parse_state) = inline_file_parse_state {
            if parse_state.is_completed_by_line(&line) {
                success_lines.push(parse_state.to_config_line());
                reset_inline_state = true
            } else {
                parse_state.add_line(line.clone());
                continue;
            }
        }
        if reset_inline_state {
            inline_file_parse_state = None;
            continue;
        }

        if let Some(captures) = INLINE_START_REGEX.captures(&line) {
            let option = &captures[1];
            if INLINE_FILE_OPTIONS.contains(option) {
                inline_file_parse_state = Some(InlineFileParseState::new(line_no, option.to_string()));
                continue;
            }
        }

        if line.trim().starts_with('#') || line.trim().is_empty() {
            continue
        }

        let line_without_comments = COMMENT_REGEX.replace(&line, "");
        let command_and_args: Vec<&str> = line_without_comments.split_whitespace().collect();
        let command = command_and_args[0];
        let args = &command_and_args[1..];
        match config_directive::parse_line(command, args) {
            config_directive::LineParseResult::NoMatchingCommand => {
                warning_lines.push(ConfigLine{number: line_no as i32, result: ParseWarning::NoMatchingCommand})
            },
            config_directive::LineParseResult::NotEnoughArguments => {
                warning_lines.push(ConfigLine{number: line_no as i32, result: ParseWarning::NotEnoughArguments})
            },
            config_directive::LineParseResult::Success(directive) => {
                success_lines.push(ConfigLine{ number: line_no as i32, result: directive })
            }
        }
    }
    Ok(ParsedConfigFile{
        success_lines: success_lines,
        warning_lines: warning_lines,
    })
}

