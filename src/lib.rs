#![recursion_limit="1024"]
#[macro_use]
extern crate error_chain;
extern crate regex;
#[macro_use]
extern crate lazy_static;

use std::io::{BufRead, BufReader, Read};
use regex::Regex;

pub use self::config_directive::{ConfigDirective, ServerGatewayArg};
mod config_directive;

mod errors {
    error_chain!{}
}
use errors::ResultExt;

lazy_static! {
    static ref COMMENT_REGEX: Regex = Regex::new(r"#.*$").unwrap();
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

pub fn parse<R>(input: R) -> errors::Result<ParsedConfigFile> where R: Read {
    let buf_reader = BufReader::new(input);
    let mut directives = Vec::new();
    let mut warnings = Vec::new();
    for (line_index, line_result) in buf_reader.lines().enumerate() {
        let line_no = line_index + 1;
        let line = line_result.chain_err(|| "Error reading input")?;
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


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
