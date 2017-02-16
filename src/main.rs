#![recursion_limit = "1024"]
#[macro_use] extern crate clap;
#[macro_use] extern crate error_chain;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate lazy_static;
extern crate regex;
extern crate toml;
extern crate url;

mod errors {
    error_chain! {
        errors {
            HandlerNotFound
        }
        foreign_links {
            Url(::url::ParseError);
            Io(::std::io::Error);
            Toml(::toml::de::Error);
            Env(::std::env::VarError);
            Regex(::regex::Error);
        }
    }
}

use errors::*;
use std::env;
use std::fs;
use std::io::Read;
use std::process::Command;
use url::Url;
use regex::Regex;

const CONFIG_FILE_NAME: &'static str = "url-handler.toml";

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

fn load_config(cfg: &str) -> Result<Config> {
    let mut config_file = fs::File::open(cfg)?;
    let mut contents = String::new();
    config_file.read_to_string(&mut contents)?;
    toml::from_str(&*contents).chain_err(|| "Could not load config file.")
}

fn run_command(cmd: &str, args: Vec<String>) -> Result<()> {
    //println!("Command: {} {}", cmd, args.join(" "));
    Command::new(cmd).args(args).output()?;
    Ok(())
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
        let expanded = env::var(clear).chain_err(|| format!("{}", var))?;
        str_expanded = str_expanded.replace(var, &*expanded);
    }

    Ok(str_expanded)
}

// Expand arguments %0 %1 %2 ...
fn expand_args(str: &str, argv: Vec<&str>) -> Result<Vec<String>> {
    let mut args = String::from(str);
    for(i, &item) in argv.iter().enumerate() {
        let re_arg = Regex::new(&*format!("%{}", i))?;
        args = re_arg.replace_all(&*args, &*item).into_owned();
    }
    let s = &*expand_env(&*args)?;
    Ok(s.split(' ').map(|it| String::from(it)).collect())
}

fn run(arg: &str, cfg: &str) -> Result<()> {
    let config = load_config(cfg)?;

    let url = Url::parse(arg)?;
    let scheme = url.scheme();

    let handler = config.handler.into_iter().find(|ref it| it.scheme == scheme)
        .ok_or(ErrorKind::HandlerNotFound)?;

    let mut input : Vec<&str> = if url.cannot_be_a_base() {
        vec![url.host_str().unwrap_or("").into(), url.path().into()]
    } else {
        vec![url.host_str().unwrap_or("").into()]
    };

    let mut paths = url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap_or(vec![]);
    input.append(&mut paths);
    input.retain(|ref e| !e.is_empty());

    let args_expanded = match handler.args {
        Some(args) => expand_args(&*args, input)?,
        None => vec![]
    };
    let cmd_expanded = &*expand_env(&*handler.command)?;

    run_command(&*cmd_expanded, args_expanded)
}

fn main() {
    let matches = clap_app!(urlhandler =>
        (version: "0.1")
        (author: "Danny Angelo Carminati Grein <danny.cabelo@gmail.com>")
        (about: "Convert custom URL strings to command and command line arguments and execute them")
        (@arg URL: +required +takes_value "URL to handle, convert and execute")
        (@arg CONFIG: -c --config +takes_value "CONFIG file with handlers settings")
        (@arg install: -i --install "Install custom handles to the system")
    ).get_matches();

    let url_arg = matches.value_of("URL").unwrap(); // URL is required an will always be Some
    let config_file = matches.value_of("CONFIG").unwrap_or(CONFIG_FILE_NAME);

    if let Err(ref e) = run(url_arg, config_file) {
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

        ::std::process::exit(-1);
    }

    ::std::process::exit(0);
}
