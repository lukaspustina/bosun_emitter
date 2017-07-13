# Bosun Emitter

[![Linux & OS X Build Status](https://img.shields.io/travis/lukaspustina/bosun_emitter.svg?label=Linux%20%26%20OS%20X%20Build%20Status)](https://travis-ci.org/lukaspustina/bosun_emitter) [![Windows Build status](https://img.shields.io/appveyor/ci/lukaspustina/bosun-emitter.svg?label=Windows%20Build%20Status)](https://ci.appveyor.com/project/lukaspustina/bosun-emitter/branch/master) [![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg?label=License)](./LICENSE) [![](https://img.shields.io/crates/v/bosun_emitter.svg)](https://crates.io/crates/bosun_emitter)

A command line tool and [Rust](http://www.rust-lang.org) library to emit metric data to [Bosun](http://bosun.org).


## Overview

> [Bosun](http://bosun.org) is an open-source, MIT licensed, monitoring and alerting system by [Stack Exchange](http://stackexchange.com/). It has an expressive domain specific language for evaluating alerts and creating detailed notifications. It also lets you test your alerts against history for a faster development experience. <sup>[[1]](http://bosun.org)</sup>

Bosun receives metric data mostly via [scollector](http://bosun.org/scollector/) which is Boson's agent running on each monitored host. scollector runs build-in as well as external collectors periodically to collect and transmit metrics on its hosts.

While it is easy to create external collectors suitable for most needs, there are cases in which sending a single, individual metric datum may be helpful. Such cases may comprise any individually run program such as a Cron job for backups or in general any other shell script. Further, it might be helpful to send metric data from your own application.

**bosun_emitter** is a library that makes it easy to send metric data and corresponding meta data describing the metric. Further, this crate contains a CLI standalone binary called `emit_bosun` that may be used on the command line or from any shell script to send a single metric datum.


## Library

Please see the [Rustdoc](http://lukaspustina.github.io/bosun_emitter) for documentation of the latest build. You can also find a crate on [crates.io](https://crates.io) and via Cargo.


## Command Line Tool

### Help

```
Emit a Bosun

USAGE:
    emit_bosun [FLAGS] [OPTIONS]

FLAGS:
    -h, --help           Prints help information
        --show-config    Prints config
    -V, --version        Prints version information
        --verbose        Enables verbose output

OPTIONS:
    -c, --config <FILE>                         Sets a custom config file
    -d, --description <DESCRIPTION>             Sets metric description
        --host <HOST:PORT>                      Sets Bosun server connection parameters; may include basic auth and https
        --hostname <HOSTNAME>                   Sets hostname
    -m, --metric <METRIC NAME>                  Sets metric name
    -r, --rate <RATE>                           Sets rate type [values: gauge, counter, rate]
    -t, --tags <KEY1=VALUE1,KEY2=VALUE2,...>    Sets tags
    -u, --unit <UNIT>                           Sets metric value unit
    -v, --value <VALUE>                         Sets metric value

Two modes are supported, i.e., sending a datum with meta data or sending only
meta data.  The modes are controlled whether a value `--value` is passed or
not. Please mind that in both cases the meta data is required.
```

### Example

```bash
emit_bosun -c examples/scollector.toml --host https://user:password@localhost:8070 --tags 'key1=value1,key2=value2' \
  --metric lukas.test --value 10 \
  --rate gauge --unit Tests -d "Amount of Lukas Tests" \
  --verbose
```


## Releases

### Source Code

You can find the source code for each release on the [GitHub Release](https://github.com/lukaspustina/bosun_emitter/releases) page.

### Binary

Travi CI creates Ubuntu Trusty packages for `emit_bosun` for each release. Please see the [Repository](https://packagecloud.io/lukaspustina/opensource) for details.

### Cargo

Rust crate releases can be found on [Crates.io](https://crates.io/crates/bosun_emitter).


## Contributing

I'll be happy about suggestions and pull requests.

