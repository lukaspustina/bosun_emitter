//! emit_bosun -- Emits a single metric datum and corresponding metric meta data to [Bosun](http:////bosun.org).

extern crate clap;
extern crate env_logger;
#[macro_use]
extern crate log;

extern crate bosun_emitter;

use clap::{Arg, ArgMatches, App};
use std::error::Error;
use std::path::Path;

use bosun_emitter::{BosunClient, Metadata, Datum, Tags, EmitterError, BosunConfig};

static VERSION: &'static str = env!("CARGO_PKG_VERSION");
static DEFAULT_CONFIG_FILE: &'static str = "/etc/bosun/scollector.conf";

#[derive(Debug)]
struct Config {
    host: String,
    hostname: String,
    metric: Option<String>,
    value: Option<String>,
    rate: Option<String>,
    unit: Option<String>,
    description: Option<String>,
    tags: Tags,
}

impl Config {
    pub fn default() -> Config {
        let bosun_config = BosunConfig::default();
        Config::from_bosun_config(bosun_config)
    }

    pub fn from_bosun_config(bosun_config: BosunConfig) -> Config {
        Config {
            host: bosun_config.Host,
            hostname: bosun_config.Hostname,
            metric: None,
            value: None,
            rate: None,
            unit: None,
            description: None,
            tags: bosun_config.Tags
        }
    }
}

fn main() {
    if env_logger::init().is_err() {
        exit_with_error("Could not initiliaze logger", -1);
    }

    let app = App::new("Emit a Bosun")
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
                                .help("Forces metric datum to be send even without meta data"));
    let cli_args = app.get_matches();

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
        Err(ModeError::NoMetadata) => exit_with_error("Cannot send datum without meta data.", -11),
        Err(ModeError::NoValue) => exit_with_error("Cannot send datum without value.", -12),
        Err(ModeError::NoSuchMode) => {
            println!("Command line arguments combination does not make any sense.\n\n{}", cli_args.usage());
            exit_with_error("", -13);
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
    let bosun_config_file_path = Path::new(cli_args.value_of("config").unwrap_or(DEFAULT_CONFIG_FILE));
    let mut config: Config = if bosun_config_file_path.exists() {
        let bosun_config: BosunConfig = try!(BosunConfig::load_from_scollector_config(&bosun_config_file_path));
        Config::from_bosun_config(bosun_config)
    } else {
        Config::default()
    };

    if cli_args.is_present("host") {
        config.host = cli_args.value_of("host").unwrap().to_string();
    }

    config.tags.insert("host".to_string(), config.hostname.to_string());
    if cli_args.is_present("hostname") {
        config.hostname = cli_args.value_of("hostname").unwrap().to_string();
        config.tags.insert("host".to_string(), config.hostname.to_string());
    }

    if cli_args.is_present("metric") {
        config.metric = Some(cli_args.value_of("metric").unwrap().to_string());
    }

    if cli_args.is_present("value") {
        config.value = Some(cli_args.value_of("value").unwrap().to_string());
    }

    if cli_args.is_present("rate") {
        config.rate = Some(cli_args.value_of("rate").unwrap().to_string());
    }

    if cli_args.is_present("unit") {
        config.unit = Some(cli_args.value_of("unit").unwrap().to_string());
    }

    if cli_args.is_present("description") {
        config.description = Some(cli_args.value_of("description").unwrap().to_string());
    }

    if cli_args.is_present("tags") {
        let tags_string = cli_args.value_of("tags").unwrap().to_string();
        try!(parse_tags(&mut config, &tags_string));
    }

    Ok(config)
}

fn parse_tags(config: &mut Config, tags_string: &str) -> Result<(), String> {
    let tags = tags_string.split(',');
    for tag in tags {
        let kv = tag.split('=').collect::<Vec<&str>>();
        if kv.len() != 2 {
            return Err(format!("unable to parse tags: '{}'", tags_string));
        }
        let k = kv[0].to_string();
        let v = kv[1].to_string();
        config.tags.insert(k, v);
    }

    Ok(())
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
    let mode_config = (config.metric.is_some(),
                       config.value.is_some(),
                       config.rate.is_some(),
                       config.unit.is_some(),
                       config.description.is_some(),
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
    let client = BosunClient::new(&config.host);
    let datum = Datum::now(config.metric.as_ref().unwrap(),
                           config.value.as_ref().unwrap(),
                           &config.tags);
    client.emit_datum(&datum)
}

fn emit_metadata(config: &Config) -> bosun_emitter::EmitterResult {
    // unwraps are safe, because mode analysis already checked these values are set
    let client = BosunClient::new(&config.host);
    let metadata = Metadata::new(config.metric.as_ref().unwrap(),
                                 config.rate.as_ref().unwrap(),
                                 config.unit.as_ref().unwrap(),
                                 config.description.as_ref().unwrap());
    client.emit_metadata(&metadata)
}



#[cfg(test)]
mod tests {
    use super::{Config, parse_tags};

    #[test]
    fn parse_tags_test_okay() {
        let mut config = Config::default();
        let tags = "key1=val1,key2=val2";
        let _ = parse_tags(&mut config, &tags);
        assert_eq!(config.tags.len(), 2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn parse_tags_test_fails_wrong_kv_separator() {
        let mut config = Config::default();
        let tags = "key1=val1,key2:val2";
        let _ = parse_tags(&mut config, &tags);
        assert_eq!(config.tags.len(), 2);
    }
}


