#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]
#![recursion_limit = "1024"]
//#![feature(windows_subsystem)]
//#![windows_subsystem = "windows"]
#[cfg(windows)] extern crate winreg;
#[macro_use] extern crate error_chain;
#[macro_use] extern crate clap;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate lazy_static;
extern crate regex;
extern crate toml;
extern crate url;

mod install;
mod errors;

use errors::*;
use std::env;
use std::fs;
use std::io::Read;
use std::process::Command;
use url::Url;
use regex::Regex;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &'static str = "./url-handler.toml";
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct Config {
    handler: Vec<Handler>
}

#[derive(Debug, Deserialize)]
struct Handler {
    scheme: String,
    command: String,
    args: Option<String>
}

struct Options {
    uninstall: bool,
    install: bool,
    list_all: bool,
}

fn load_config(cfg: &PathBuf) -> Result<Config> {
    let mut config_file = fs::File::open(cfg)
        .chain_err(|| format!("couldn't find {}", cfg.to_string_lossy()))?;
    let mut contents = String::new();
    config_file.read_to_string(&mut contents)?;
    toml::from_str(&*contents).chain_err(|| "Could not load config file.")
}

fn run_command(cmd: &str, args: &[String]) -> Result<i32> {
    Command::new(cmd).args(args).spawn()?;
    Ok(0)
}

// Expand system environment variables, ie. %USERNAME%
fn expand_env(str: &str) -> Result<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"%([_\(\)\{\}\[\]\$\\;!\?0-9A-Za-z]+)%").unwrap();
    }
    let mut str_expanded = String::from(str);
    for cap in RE.captures_iter(&*str) {
        let var = &cap[0];
        let clear = var.trim_matches('%');
        let expanded = env::var(clear).chain_err(|| var)?;
        str_expanded = str_expanded.replace(var, &*expanded);
    }

    Ok(str_expanded)
}

#[test]
fn expand_env_basic() {
    std::env::set_var("1", "1");
    std::env::set_var("2", "2");
    std::env::set_var("3", "3");
    assert_eq!(expand_env("test %1%").unwrap(), String::from("test 1"));
    assert_eq!(expand_env("test %1% %3% %2%").unwrap(), String::from("test 1 3 2"));
    assert_eq!(expand_env("test %3% %2% %1%").unwrap(), String::from("test 3 2 1"));
    assert_eq!(expand_env("repeat %1% %2% %1%").unwrap(), String::from("repeat 1 2 1"));
    assert_eq!(expand_env("concat %1%%1%%1% %2%x").unwrap(), String::from("concat 111 2x"));
}

// Expand named parameters: {key} -> value
fn expand_named(str: &str, url: &Url) -> String {
    let params = url.query_pairs();
    let mut args = String::from(str);
    for (k, v) in params {
        args = str::replace(&*args, &*format!("{{{}}}", k), &*v);
    }
    args
}

#[test]
fn expand_named_basic() {
    assert_eq!(expand_named("test {arg}", &Url::parse("x://?arg=replaced").unwrap()), String::from("test replaced"));
    assert_eq!(expand_named("test {1} {3} {2}", &Url::parse("x://?1=1&2=2&3=3").unwrap()), String::from("test 1 3 2"));
    assert_eq!(expand_named("test {3} {2} {1}", &Url::parse("x://?2=2&1=1&3=3").unwrap()), String::from("test 3 2 1"));
    assert_eq!(expand_named("repeat {1} {2} {1}", &Url::parse("x://?1=1&2=2").unwrap()), String::from("repeat 1 2 1"));
    assert_eq!(expand_named("concat {1}{1}{1} {2}x", &Url::parse("x://?1=1&2=2").unwrap()), String::from("concat 111 2x"));
    assert_eq!(expand_named("missing {1} {2} {3}", &Url::parse("x://?1=1&2=2").unwrap()), String::from("missing 1 2 {3}"));
    assert_eq!(expand_named("extra {1} {3}", &Url::parse("x://?1=1&2=2&3=3").unwrap()), String::from("extra 1 3"));
}

#[test]
fn expand_named_real_world() {
    assert_eq!(
        expand_named(r#"cargo run --bin {app} -- --input="{input}" -f"#, &Url::parse("x://?app=url-handler&input=Some%20input").unwrap()),
        String::from(r#"cargo run --bin url-handler -- --input="Some input" -f"#)
    );
}

#[test]
fn expand_named_quoted() {
    assert_eq!(
        expand_named(r#"{cmd} "lorem ipsum "{msg}" sit amet""#, &Url::parse("x://?cmd=echo&msg='dolor'").unwrap()),
        String::from(r#"echo "lorem ipsum "'dolor'" sit amet""#)
    );
}

// Expand arguments %1 %2 %3 ...
fn expand_args(str: &str, argv: &[&str]) -> String {
    let mut args = String::from(str);
    for (i, &item) in argv.iter().enumerate() {
        args = str::replace(&*args, &*format!("%{}", i + 1), &*item);
    }
    args
}

#[test]
fn expand_args_basic() {
    assert_eq!(expand_args("test %1", &["arg"]), String::from("test arg"));
    assert_eq!(expand_args("test %1 %3 %2", &["1", "2", "3"]), String::from("test 1 3 2"));
    assert_eq!(expand_args("test %3 %2 %1", &["1", "2", "3"]), String::from("test 3 2 1"));
    assert_eq!(expand_args("repeat %1 %2 %1", &["1", "2"]), String::from("repeat 1 2 1"));
    assert_eq!(expand_args("concat %1%1%1 %2x", &["1", "2"]), String::from("concat 111 2x"));
    assert_eq!(expand_args("missing %1 %2 %3", &["1", "2"]), String::from("missing 1 2 %3"));
    assert_eq!(expand_args("extra %1 %3", &["1", "2", "3"]), String::from("extra 1 3"));
}

#[test]
fn expand_args_real_world() {
    assert_eq!(
        expand_args(r#"cargo run --bin %1 -- --input="%2" -f"#, &["url-handler", "Some input"]),
        String::from(r#"cargo run --bin url-handler -- --input="Some input" -f"#)
    );
}

#[test]
fn expand_args_nested_quotes() {
    assert_eq!(
        expand_args(r#"%2 "lorem ipsum '%1' sit amet""#, &["dolor", "echo"]),
        String::from(r#"echo "lorem ipsum 'dolor' sit amet""#)
    );

    assert_eq!(
        expand_args(r#"%1 "lorem ipsum ('%2 "%2is" %3') sit amet""#, &["echo", "dolor", "septetur"]),
        String::from(r#"echo "lorem ipsum ('dolor "doloris" septetur') sit amet""#)
    );
}

// Split a string containing space separated args into a vector respecting quoted strings
fn split_args(args: &str) -> Vec<String> {
    let mut res = Vec::new();
    let mut current_arg = String::new();
    let mut in_quote = Vec::new();
    for c in args.chars() {
        if in_quote.is_empty() && c.is_whitespace() {
            res.push(current_arg);
            current_arg = String::new();
            continue;
        }
        current_arg.push(c);
        if c == '"' || c == '\'' {
            if in_quote.last() == Some(&c) {
                in_quote.pop();
            } else {
                in_quote.push(c);
            }
        }
    }
    if !current_arg.is_empty() {
        res.push(current_arg);
    }
    res
}

#[test]
fn split_args_basic() {
    assert_eq!(split_args("test arg"), vec!["test", "arg"]);
    assert_eq!(split_args(r#"test "arg""#), vec!["test", "\"arg\""]);
    assert_eq!(split_args(r#"test "arg 1 2""#), vec!["test", "\"arg 1 2\""]);
    assert_eq!(split_args(r#"test 'arg'"#), vec!["test", "\'arg\'"]);
    assert_eq!(split_args(r#"test 'arg 1 2'"#), vec!["test", "\'arg 1 2\'"]);
}

#[test]
fn split_args_real_world() {
    assert_eq!(
        split_args(r#"cargo run --bin url-handler -- --input="Some input" -f"#),
        vec!["cargo", "run", "--bin", "url-handler", "--", "--input=\"Some input\"", "-f"]
    );
}

#[test]
fn split_args_nested_quotes() {
    assert_eq!(
        split_args(r#"echo "lorem ipsum 'dolor' sit amet""#),
        vec!["echo", "\"lorem ipsum 'dolor' sit amet\""]
    );

    assert_eq!(
        split_args(r#"echo "lorem ipsum ('dolor "doloris" septetur') sit amet""#),
        vec!["echo", "\"lorem ipsum ('dolor \"doloris\" septetur') sit amet\""]
    );
}

// Anything after the scheme and before parameters "?" is considered a numbered argument
fn get_args(url: &Url) -> Vec<&str> {
    if url.cannot_be_a_base() {
        vec![url.host_str().unwrap_or("").into(), url.path().into()]
    } else {
        vec![url.host_str().unwrap_or("").into()]
    }
}

fn run(arg: Option<&str>, cfg: &str, opt: &Options) -> Result<i32> {
    let cfg = fs::canonicalize(&cfg)?;
    if opt.list_all {
        let v = install::list_all();
        if v.is_empty() {
            println!("No handlers found.");
        } else {
            println!("Listing installed handlers:");
            for i in v {
                println!("\t{}", i);
            }
        }
        return Ok(0)
    }

    if opt.uninstall {
        install::uninstall_all()?;
    }

    let config = load_config(&cfg)?;

    if opt.install {
        let cmd = std::env::current_exe()?;
        let cmd = cmd.to_str().ok_or(ErrorKind::UnknownError)?;
        for it in &config.handler {
            println!("Installing {}...", &*it.scheme);
            install::install_handler(&*it.scheme, cmd, &cfg)?;
        }
        return Ok(0)
    }

    match arg {
        Some(arg) => {
            let url = Url::parse(arg)?;
            let scheme = url.scheme();
            let handler = config.handler.into_iter()
                .find(|it| it.scheme == scheme)
                .ok_or(ErrorKind::HandlerNotFound)?;

            let mut args = get_args(&url);
            let mut paths = url.path_segments()
                .map(|c| c.collect::<Vec<_>>()).unwrap_or_else(|| vec![]);
            args.append(&mut paths);
            args.retain(|e| !e.is_empty());

            let handler_args = handler.args.unwrap_or_default();
            let args_expanded = expand_named(&handler_args, &url);
            let args_expanded = expand_args(&*args_expanded, &args);
            let args_expanded = expand_env(&*args_expanded)?;
            let cmd_expanded = &*expand_env(&*handler.command)?;

            //println!("{} {:?}", cmd_expanded, args_expanded);
            run_command(&*cmd_expanded, &split_args(&*args_expanded))
        },
        None => Ok(0)
    }
}

fn main() {
    let matches = clap_app!(urlhandler =>
        (version: VERSION.unwrap_or("unknown"))
        (author: "Danny Angelo Carminati Grein <danny.cabelo@gmail.com>")
        (about: "URL-to-command line conversion\nLicense MIT\n")
        (@arg URL: +takes_value "URL to handle, convert and execute")
        (@arg CONFIG: -c --config +takes_value "CONFIG file with handlers settings")
        (@arg list: -l --list "List all installed handlers")
        (@arg install: -i --install "Install all custom handles to the system")
        (@arg uninstall: -u --uninstall "Uninstall all existing custom handles from the system")
    ).get_matches();

    let url_arg = matches.value_of("URL");
    let config_file = matches.value_of("CONFIG").unwrap_or(CONFIG_FILE_NAME);
    let options = Options {
        uninstall: matches.is_present("uninstall"),
        install: matches.is_present("install"),
        list_all: matches.is_present("list")
    };

    let code = match run(url_arg, config_file, &options) {
        Err(ref e) => {
            use ::std::io::Write;
            let stderr = &mut ::std::io::stderr();
            let err_msg = "Error writing to stderr";
            writeln!(stderr, "error: {}", e).expect(err_msg);

            for e in e.iter().skip(1) {
                writeln!(stderr, "caused by: {}", e).expect(err_msg);
            }

            // To enable backtrace run with `RUST_BACKTRACE=1`.
            if let Some(backtrace) = e.backtrace() {
                writeln!(stderr, "backtrace: {:?}", backtrace).expect(err_msg);
            }
            -1
        }
        Ok(c) => c
    };

    ::std::process::exit(code);
}
