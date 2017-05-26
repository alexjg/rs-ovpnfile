#![recursion_limit="1024"]
#[macro_use]
extern crate error_chain;
extern crate regex;
#[macro_use]
extern crate lazy_static;

use std::io::{BufRead, BufReader, Read};
use std::collections::HashSet;
use regex::Regex;

pub use self::config_directive::{ConfigDirective, ServerGatewayArg, File};
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

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ConfigLine<T> {
    pub number: i32,
    pub result: T,
}

#[derive(Debug)]
pub enum ParseWarning {
    NotEnoughArguments,
    NoMatchingCommand,
}

pub struct ParsedConfigFile {
    pub directives: Vec<ConfigLine<ConfigDirective>>,
    pub warnings: Vec<ConfigLine<ParseWarning>>,
}

struct InlineFileParseState {
    start_line_no: i32,
    identifier: String,
    lines: Vec<String>,
}

impl InlineFileParseState {
    fn new(line_no: usize, identifier: String) -> InlineFileParseState {
        return InlineFileParseState{
            start_line_no: line_no as i32,
            identifier: identifier,
            lines: Vec::new(),
        }
    }
    fn is_completed_by_line(&self, line: &str) -> bool {
        if let Some(end_identifier_captures) = INLINE_END_REGEX.captures(&line) {
            return end_identifier_captures[1] == self.identifier
        }
        return false
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
        return ConfigLine{result: directive, number: self.start_line_no as i32};
    }
}

pub fn parse<R>(input: R) -> errors::Result<ParsedConfigFile> where R: Read {
    let buf_reader = BufReader::new(input);
    let mut directives = Vec::new();
    let mut warnings = Vec::new();
    let mut inline_file_parse_state: Option<InlineFileParseState> = None;
    for (line_index, line_result) in buf_reader.lines().enumerate() {
        let line_no = line_index + 1;
        let line = line_result.chain_err(|| "Error reading input")?;

        let mut reset_inline_state = false;
        if let Some(ref mut parse_state) = inline_file_parse_state {
            if parse_state.is_completed_by_line(&line) {
                directives.push(parse_state.to_config_line());
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

        if line.trim().starts_with("#") {
            continue
        }
        if line.trim().len() == 0 {
            continue
        }
        let line_without_comments = COMMENT_REGEX.replace(&line, "");
        let command_and_args: Vec<&str> = line_without_comments.split_whitespace().collect();
        let command = command_and_args[0];
        let args = &command_and_args[1..];
        match config_directive::parse_line(command, args) {
            config_directive::LineParseResult::NoMatchingCommand => {
                warnings.push(ConfigLine{number: line_no as i32, result: ParseWarning::NoMatchingCommand})
            },
            config_directive::LineParseResult::NotEnoughArguments => {
                warnings.push(ConfigLine{number: line_no as i32, result: ParseWarning::NotEnoughArguments})
            },
            config_directive::LineParseResult::Success(directive) => {
                directives.push(ConfigLine{ number: line_no as i32, result: directive })
            }
        }
    }
    Ok(ParsedConfigFile{
        directives: directives,
        warnings: warnings,
    })
}

