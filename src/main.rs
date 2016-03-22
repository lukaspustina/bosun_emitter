//! emit_bosun -- Emits a single metric datum and corresponding metric metadata to [Bosun](http:////bosun.org).

extern crate clap;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate rustc_serialize;
extern crate toml;

extern crate bosun_emitter;

use clap::{Arg, ArgMatches, App};
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::io::Read;
use rustc_serialize::Decodable;

use bosun_emitter::{BosunClient, Metadata, Datum, Tags, EmitterError};

static VERSION: &'static str = env!("CARGO_PKG_VERSION");
static DEFAULT_CONFIG_FILE: &'static str = "/etc/bosun/scollector.conf";

#[derive(Debug)]
#[derive(RustcDecodable)]
#[allow(non_snake_case)]
struct Config {
    Host: String,
    Hostname: String,
    Metric: Option<String>,
    Value: Option<String>,
    Rate: Option<String>,
    Unit: Option<String>,
    Description: Option<String>,
    Tags: Tags,
}

impl Config {
    pub fn default() -> Config {
        Config {
            Host: "localhost:8070".to_string(),
            Hostname: "localhost".to_string(),
            Metric: None,
            Value: None,
            Rate: None,
            Unit: None,
            Description: None,
            Tags: Tags::new(),
        }
    }
}

fn main() {
    if env_logger::init().is_err() {
        exit_with_error("Could not initiliaze logger", -1);
    }

    let cli_args = App::new("Emit a Bosun")
                       .version(VERSION)
                       .after_help("Two modes are supported, i.e., sending a datum with meta \
                                    data or sending only meta data. The modes are controlled \
                                    whether a value `--value` is passed or not. Please mind that \
                                    in both cases the meta data is required.")
                       .arg(Arg::with_name("config")
                                .short("c")
                                .long("config")
                                .value_name("FILE")
                                .help("Sets a custom config file")
                                .takes_value(true))
                       .arg(Arg::with_name("host")
                                .long("host")
                                .value_name("HOST:PORT")
                                .help("Sets Bosun server connection parameters")
                                .takes_value(true))
                       .arg(Arg::with_name("hostname")
                                .long("hostname")
                                .value_name("HOSTNAME")
                                .help("Sets hostname")
                                .takes_value(true))
                       .arg(Arg::with_name("metric")
                                .short("m")
                                .long("metric")
                                .value_name("METRIC NAME")
                                .help("Sets metric name")
                                .takes_value(true))
                       .arg(Arg::with_name("value")
                                .requires("metric")
                                .short("v")
                                .long("value")
                                .value_name("VALUE")
                                .help("Sets metric value")
                                .takes_value(true))
                       .arg(Arg::with_name("rate")
                                .requires_all(&["metric", "rate", "unit", "description"])
                                .short("r")
                                .long("rate")
                                .value_name("RATE")
                                .possible_values(&["gauge", "counter", "rate"])
                                .help("Sets rate type")
                                .takes_value(true))
                       .arg(Arg::with_name("unit")
                                .requires_all(&["metric", "rate", "unit", "description"])
                                .short("u")
                                .long("unit")
                                .value_name("UNIT")
                                .help("Sets metric value unit")
                                .takes_value(true))
                       .arg(Arg::with_name("description")
                                .requires_all(&["metric", "rate", "unit", "description"])
                                .short("d")
                                .long("description")
                                .value_name("DESCRIPTION")
                                .help("Sets metric description")
                                .takes_value(true))
                       .arg(Arg::with_name("tags")
                                .use_delimiter(false)
                                .short("t")
                                .long("tags")
                                .value_name("KEY1=VALUE1,KEY2=VALUE2,...")
                                .help("Sets tags")
                                .takes_value(true))
                       .arg(Arg::with_name("show-config")
                                .long("show-config")
                                .help("Prints config"))
                       .arg(Arg::with_name("verbose")
                                .long("verbose")
                                .help("Enables verbose output"))
                       .arg(Arg::with_name("force")
                                .hidden(true)
                                .long("force")
                                .help("Forces metric datum to be send even without metadata"))
                       .get_matches();

    let force: bool = cli_args.is_present("force");
    let verbose: bool = cli_args.is_present("verbose");
    let config: Config = match parse_args(&cli_args) {
        Ok(config) => config,
        Err(err) => {
            exit_with_error(&format!("Failed to parse configuration, because {}.", err),
                            -2);
        }
    };
    if cli_args.is_present("show-config") {
        println!("config: {:?}", config);
    }

    let mode = match mode(&config, force) {
        Ok(mode) => mode,
        Err(ModeError::NoMetadata) => exit_with_error("Cannot send datum without metadata.", -11),
        Err(ModeError::NoValue) => exit_with_error("Cannot send datum without value.", -12),
        Err(ModeError::NoSuchMode) => {
            exit_with_error("Command line arguments combination does not make any sense.",
                            -13);
        }
    };

    let result = run(&config, mode, verbose);
    match result {
        Ok(_) => {}
        Err(EmitterError::JsonParseError(e)) => {
            exit_with_error(&format!("Failed to create JSON document, because {}.", e),
                            1)
        }
        Err(EmitterError::EmitError(e)) => {
            exit_with_error(&format!("Failed to send, because {}.", e), 2)
        }
        Err(EmitterError::ReceiveError(e)) => {
            exit_with_error(&format!("Failed to create resource, because {}.", e), 3)
        }
    }
}

fn parse_args(cli_args: &ArgMatches) -> Result<Config, Box<Error>> {
    let config_file_path = Path::new(cli_args.value_of("config").unwrap_or(DEFAULT_CONFIG_FILE));
    let mut config: Config = if config_file_path.exists() {
        try!(load_config_from_file(&config_file_path))
    } else {
        Config::default()
    };

    if cli_args.is_present("host") {
        config.Host = cli_args.value_of("host").unwrap().to_string();
    }

    if cli_args.is_present("hostname") {
        config.Hostname = cli_args.value_of("hostname").unwrap().to_string();
        config.Tags.insert("host".to_string(), config.Hostname.to_string());
    }

    if cli_args.is_present("metric") {
        config.Metric = Some(cli_args.value_of("metric").unwrap().to_string());
    }

    if cli_args.is_present("value") {
        config.Value = Some(cli_args.value_of("value").unwrap().to_string());
    }

    if cli_args.is_present("rate") {
        config.Rate = Some(cli_args.value_of("rate").unwrap().to_string());
    }

    if cli_args.is_present("unit") {
        config.Unit = Some(cli_args.value_of("unit").unwrap().to_string());
    }

    if cli_args.is_present("description") {
        config.Description = Some(cli_args.value_of("description").unwrap().to_string());
    }

    if cli_args.is_present("tags") {
        let tags_string = cli_args.value_of("tags").unwrap().to_string();
        parse_tags(&mut config, &tags_string);
    }

    Ok(config)
}

fn parse_tags(config: &mut Config, tags_string: &str) {
    tags_string.split(',')
               .map(|kv| kv.split('=').collect::<Vec<&str>>())
               .map(|vec| {
                   assert_eq!(vec.len(), 2);
                   (vec[0].to_string(), vec[1].to_string())
               })
               .fold((), |_, (k, v)| {
                   config.Tags.insert(k, v);
               });
}

fn load_config_from_file<T: Decodable>(file_path: &Path) -> Result<T, Box<Error>> {
    match load_toml(file_path) {
        Ok(toml) => {
            let mut decoder = toml::Decoder::new(toml);
            let config = try!(T::decode(&mut decoder));

            Ok(config)
        }
        Err(err) => Err(err),
    }
}

fn load_toml(file_path: &Path) -> Result<toml::Value, Box<Error>> {
    let mut config_file = try!(File::open(file_path));
    let mut config_content = String::new();
    try!(config_file.read_to_string(&mut config_content));

    let mut parser = toml::Parser::new(&config_content);
    match parser.parse() {
        Some(toml) => Ok(toml::Value::Table(toml)),
        None => Err(From::from(parser.errors.pop().unwrap())),
    }
}

enum Mode {
    Normal,
    MetadataOnly,
    DatumOnly,
}

enum ModeError {
    NoMetadata,
    NoValue,
    NoSuchMode,
}

fn run(config: &Config, mode: Mode, verbose: bool) -> bosun_emitter::EmitterResult {
    match mode {
        Mode::Normal => {
            msg("Sending meta data.", verbose);
            try!(emit_metadata(config));

            msg("Sending datum.", verbose);
            emit_datum(config)
        }
        Mode::MetadataOnly => {
            msg("Sending meta data.", verbose);
            emit_metadata(config)
        }
        Mode::DatumOnly => {
            msg("Sending datum.", verbose);
            emit_datum(config)
        }
    }
}

/// We support two modes officially and more mode unofficially.
/// 1. Send Datum with Metadata
/// 1. Send only Metadata
/// 1. Send Datum without Metadata -- only with `--force`
fn mode(config: &Config, force: bool) -> Result<Mode, ModeError> {
    let mode_config = (config.Metric.is_some(),
                       config.Value.is_some(),
                       config.Rate.is_some(),
                       config.Unit.is_some(),
                       config.Description.is_some(),
                       force);
    match mode_config {
        (true, true, true, true, true, _) => Ok(Mode::Normal),
        (true, _, true, true, true, _) => Ok(Mode::MetadataOnly),
        (true, true, _, _, _, true) => Ok(Mode::DatumOnly),
        (true, true, _, _, _, false) => Err(ModeError::NoMetadata),
        (true, false, _, _, _, _) => Err(ModeError::NoValue),
        _ => Err(ModeError::NoSuchMode),
    }
}

fn msg(msg: &str, verbose: bool) {
    if verbose {
        println!("{}", msg);
    }
}

fn exit_with_error(msg: &str, exit_code: i32) -> ! {
    println!("{}", msg);
    std::process::exit(exit_code);
}

fn emit_datum(config: &Config) -> bosun_emitter::EmitterResult {
    // unwraps are safe, because mode analysis already checked these values are set
    let client = BosunClient::new(&config.Host);
    let datum = Datum::now(config.Metric.as_ref().unwrap(),
                           config.Value.as_ref().unwrap(),
                           &config.Tags);
    client.emit_datum(&datum)
}

fn emit_metadata(config: &Config) -> bosun_emitter::EmitterResult {
    // unwraps are safe, because mode analysis already checked these values are set
    let client = BosunClient::new(&config.Host);
    let metadata = Metadata::new(config.Metric.as_ref().unwrap(),
                                 config.Rate.as_ref().unwrap(),
                                 config.Unit.as_ref().unwrap(),
                                 config.Description.as_ref().unwrap());
    client.emit_metadata(&metadata)
}
