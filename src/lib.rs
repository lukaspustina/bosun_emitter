//! bosun_emitter - A library to emit metric data to [Bosun](http://bosun.org) from your command line.
//!
//! > "[Bosun](http://bosun.org) is an open-source, MIT licensed, monitoring and alerting system by [Stack Exchange](http://stackexchange.com/). It has an expressive domain specific language for evaluating alerts and creating detailed notifications. It also lets you test your alerts against history for a faster development experience." <sup>[[1]](http://bosun.org)</sup>
//!
//! Bosun receives metric data mostly via [scollector](http://bosun.org/scollector/) which is Boson's agent running on each monitored host. scollector runs build-in as well as external collectors periodically to collect and trasmit metrics on its hosts.
//!
//! While it is easy to create external collectors and suitable for most needs, there are cases in which sending a single, individual metric datum may be helpful. Such cases may comprise any individually run program such as a Cron job for backups or in general any other shell script. Futher, it might be helpful to send metric data from your own application.
//!
//! **bosun_emitter** is a library that eases sending metric data and corresponding meta data describing the metric. Furhter, this crate contains a CLI standalone binary called `emit_bosun` that may be used on the command line or from any shell script to send a single metric datum.
//!
//! # Example
//!
//! ## Library
//!
//! ```no_run
//! use bosun_emitter::{BosunClient, Datum, EmitterError, Metadata, now_in_ms, Tags};
//!
//! let client = BosunClient::new("localhost:8070");
//! let metric = "lukas.tests.count";
//!
//! let metadata = Metadata::new(&metric, "counter", "Test", "Amount of Lukas Tests");
//! let _ = client.emit_metadata(&metadata);
//!
//! let tags: Tags = Tags::new();
//! let datum = Datum::new(&metric, now_in_ms(), "42", &tags);
//!
//! match client.emit_datum(&datum) {
//!     Ok(_) => {}
//!     Err(EmitterError::JsonParseError(e)) => panic!("Failed to create JSON document."),
//!     Err(EmitterError::EmitError(e)) => panic!("Failed to send."),
//!     Err(EmitterError::ReceiveError(e)) => panic!("Failed to create resource."),
//! }
//! ```
//!
//! ## CLI Tool -- Shell Script
//!
//! ```bash
//! #!/bin/bash
//!
//! local start=$(date +%s)
//! # Complex, time consuming backup ...
//! local now=$(date +%s)
//! local runtime=$((${now}-${start}))
//!
//! emit_bosun --host localhost:8070 --hostname backup-server --tags 'type=mongodb,database=production' --metric backup.runtime --value $runtime --rate gauge --unit sec -d "Backup runtime"
//! ```
//!
//! `emit_bosun` parses scollector config files for settings like Bosun server `Host, --host`, local hostname `Hostname, --hostname`, and tags `Tags, --tags`. So in case scollector is configured on your host, you can omit these CLI parameters and just pass the configuration file.
//!
//! ## CLI Tool -- Parsing scollector Configuration File
//!
//! ```bash
//! > emit_bosun -c /etc/bosun/scollector.conf --metric backup.runtime --value $runtime --rate gauge --unit sec -d "Backup Runtime in sec"
//! ```
//!
//! The above example scollector configuration file is the default path where `emit_bosun` looks for a configuration file. So you can omit even that parameter. In addition, tags passed on the command line will be merged with tags read from scollector's configuration file.
//!
//! ```bash
//! > emit_bosun --metric backup.runtime --value $runtime --rate gauge --unit sec -d "Backup Runtime in sec"
//! ```

#![deny(missing_docs)]

extern crate chrono;
extern crate hyper;
#[macro_use]
extern crate log;
extern crate rustc_serialize;

use chrono::Timelike;
use hyper::Client;
use hyper::header::ContentType;
use hyper::status::StatusCode;
use hyper::error::Error as HyperError;
use rustc_serialize::json;
use rustc_serialize::json::EncoderError;
use std::collections::HashMap;
use std::convert::From;
use std::fmt;

/// Result of an attempt to send meta data or a metric datum
pub type EmitterResult = Result<(), EmitterError>;

/// Errors which may occurr while sending a either metadata or a metric data.
#[derive(Debug)]
pub enum EmitterError {
    /// Failed to create JSON.
    JsonParseError(EncoderError),
    /// Failed to send JSON.
    EmitError(HyperError),
    /// Failed to create Datum on server.
    ReceiveError(StatusCode),
}

impl From<HyperError> for EmitterError {
    fn from(err: HyperError) -> EmitterError {
        EmitterError::EmitError(err)
    }
}

impl From<EncoderError> for EmitterError {
    fn from(err: EncoderError) -> EmitterError {
        EmitterError::JsonParseError(err)
    }
}

/// Encapsulates Bosun server connection.
#[derive(Debug)]
pub struct BosunClient {
    /// `<HOSTNAME|IP ADDR>:<PORT>`
    pub host: String,
}

impl BosunClient {
    /// Creates a new BosunClient.
    pub fn new(host: &str) -> BosunClient {
        BosunClient { host: host.to_string() }
    }

    /// Creates an URL String for a given API endpoint from host.
    ///
    fn as_api_url(&self, api_endpoint: &str) -> String {
        fmt::format(format_args!("http://{}/{}", self.host, api_endpoint))
    }

    /// Sends metric meta data to Bosun server.
    ///
    /// # Example
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use bosun_emitter::{BosunClient, Metadata};
    /// let metadata = Metadata::new("lukas.tests.count", "counter", "Tests", "Amount of Lukas Tests");
    ///
    /// let client = BosunClient::new("localhost:8070");
    /// let _ = client.emit_metadata(&metadata);
    /// ```
    pub fn emit_metadata(&self, metadata: &Metadata) -> EmitterResult {
        let encoded = try!(metadata.to_json());
        let res = BosunClient::send_to_bosun_api(&self.as_api_url("api/metadata/put"), &encoded);
        info!("Sent medata '{:?}' to '{:?}' with result: '{:?}'.",
              encoded,
              self.host,
              res);

        res
    }

    /// Sends metric datum to Bosun server.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use bosun_emitter::{BosunClient, Datum, Tags};
    /// let mut tags = Tags::new();
    /// tags.insert("host".to_string(), "test-vm".to_string());
    /// let datum = Datum::new("lukas.tests.count", 1458066838, "1", &tags);
    ///
    /// let client = BosunClient::new("localhost:8070");
    /// let _ = client.emit_datum(&datum);
    /// ```
    pub fn emit_datum(&self, datum: &Datum) -> EmitterResult {
        let encoded = try!(datum.to_json());
        let res = BosunClient::send_to_bosun_api(&self.as_api_url("api/put"), &encoded);
        info!("Sent datum '{:?}' to '{:?}' with result: '{:?}'.",
              encoded,
              &self.host,
              res);

        res
    }

    fn send_to_bosun_api(url: &str, json: &str) -> EmitterResult {
        let client = Client::new();
        let req = client.post(url).header(ContentType::json()).body(json);
        let res = try!(req.send());
        debug!("send_to_bosun_api: Request to '{}', Result: '{:?}'.",
               url,
               res);

        match res.status {
            StatusCode::NoContent => Ok(()),
            status_code => Err(EmitterError::ReceiveError(status_code)),
        }
    }
}

#[derive(RustcEncodable, Debug)]
/// Represents metric meta data.
pub struct Metadata<'a> {
    /// Metric name
    pub metric: &'a str,
    /// Metric rate type: [gauge, counter rate]
    pub rate: &'a str,
    /// Metric unit
    pub unit: &'a str,
    /// Metric description
    pub description: &'a str,
}

// TODO: Add check for rate type: [gauge, counter rate]
impl<'a> Metadata<'a> {
    /// Creates a new metric metadata.
    pub fn new(metric: &'a str,
               rate: &'a str,
               unit: &'a str,
               description: &'a str)
               -> Metadata<'a> {
        Metadata {
            metric: metric,
            rate: rate,
            unit: unit,
            description: description,
        }
    }

    /// Encodes Metadata to JSON as String.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate bosun_emitter;
    /// # extern crate rustc_serialize;
    /// # use bosun_emitter::Metadata;
    /// # use rustc_serialize::json::{Json};
    /// #
    /// # fn main() {
    /// let metadata = Metadata::new("lukas.tests.count", "counter", "Tests", "Amount of Lukas Tests");
    /// let json = metadata.to_json().unwrap();
    ///
    /// # let json_json = Json::from_str(&json).unwrap();
    /// # let expected = "[{\"value\":\"Tests\",\"name\":\"unit\",\"metric\":\"lukas.tests.count\"},{\"metric\":\"lukas.tests.count\",\"value\":\"counter\",\"name\":\"rate\"},{\"name\":\"desc\",\"metric\":\"lukas.tests.count\",\"value\":\"Amount of Lukas Tests\"}]";
    /// # let expected_json = Json::from_str(&expected).unwrap();
    /// # assert_eq!(expected_json, json_json);
    /// # }
    /// ```
    pub fn to_json(&self) -> Result<String, EmitterError> {
        let mut metadata = [HashMap::new(), HashMap::new(), HashMap::new()];
        metadata[0].insert("metric", self.metric);
        metadata[0].insert("name", "unit");
        metadata[0].insert("value", self.unit);
        metadata[1].insert("metric", self.metric);
        metadata[1].insert("name", "rate");
        metadata[1].insert("value", self.rate);
        metadata[2].insert("metric", self.metric);
        metadata[2].insert("name", "desc");
        metadata[2].insert("value", self.description);

        let json = try!(json::encode(&metadata));
        debug!("Metadata::to_json '{:?}', '{:?}'", &self, json);

        Ok(json)
    }
}

/// Metric tags equivalent to Rust's `HashMap<String, String>`
pub type Tags = HashMap<String, String>;

/// Represents a metric datum.
#[derive(RustcEncodable, Debug)]
pub struct Datum<'a> {
    /// Metric name
    pub metric: &'a str,
    /// Unix timestamp in either _s_ or _ms_
    pub timestamp: i64,
    /// Value as string representation
    pub value: &'a str,
    /// Tags for this metric datum
    pub tags: &'a Tags,
}

impl<'a> Datum<'a> {
    /// Creates a new metric datum with a specified timestamp in ms.
    pub fn new(metric: &'a str,
               timestamp: i64,
               value: &'a str,
               // TODO: make me use refs
               tags: &'a Tags)
               -> Datum<'a> {
        Datum {
            metric: metric,
            timestamp: timestamp,
            value: value,
            tags: tags,
        }
    }
    /// Creates a new metric datum with timestamp _now_.
    pub fn now(metric: &'a str,
               value: &'a str,
               // TODO: make me use refs
               tags: &'a Tags)
               -> Datum<'a> {
        Datum {
            metric: metric,
            timestamp: now_in_ms(),
            value: value,
            tags: tags,
        }
    }

    /// Encodes a Datum to JSON as String.
    ///
    /// # Example
    ///
    /// ```
    /// # use bosun_emitter::{Datum, Tags};
    /// let mut tags = Tags::new();
    /// tags.insert("host".to_string(), "test-vm".to_string());
    /// let datum = Datum::new("lukas.tests.count", 1458066838, "1", &tags);
    /// let json = datum.to_json().unwrap();
    ///
    /// # let expected = "{\"metric\":\"lukas.tests.count\",\"timestamp\":1458066838,\"value\":\"1\",\"tags\":{\"host\":\"test-vm\"}}";
    /// # assert_eq!(expected, json);
    /// ```
    pub fn to_json(&self) -> Result<String, EmitterError> {
        let json = try!(json::encode(self));
        debug!("Datum::to_json '{:?}', '{:?}'", &self, json);

        Ok(json)
    }
}

/// Returns Unix timestamp in ms.
pub fn now_in_ms() -> i64 {
    let now = chrono::Local::now();
    now.timestamp() * 1000 + (now.nanosecond() / 1000) as i64
}

#[cfg(test)]
#[test]
fn test_as_api_url() {
    let host = "localhost:8070";
    let client = BosunClient::new(host);

    assert_eq!(client.as_api_url("api"), "http://localhost:8070/api");
    assert_eq!(client.as_api_url("metadata/api"),
               "http://localhost:8070/metadata/api");
}
