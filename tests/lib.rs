extern crate bosun_emitter;
extern crate mktemp;
extern crate rustc_serialize;

use bosun_emitter::*;
use rustc_serialize::json::Json;
use mktemp::Temp;
use std::io::prelude::*;
use std::fs::File;
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::thread;

#[test]
fn send_metadata() {
    let metric = "lukas.tests.count";
    let rate = "counter";
    let unit = "Tests";
    let description = "Amount of Lukas Tests";

    let port = 18070; // Actually, we should generate a random port number and check, if it is free
    let server = run_server(port);
    let client = BosunClient::new(&format!("localhost:{}", port));
    let metadata = Metadata::new(&metric, &rate, &unit, &description);
    let result = client.emit_metadata(&metadata);
    assert!(result.is_ok());

    let output = server.recv()
                      .unwrap_or_else(|e| panic!("failed to wait on child: {}", e));

    assert!(output.find("Content-Type: application/json; charset=utf-8").is_some());
    let json = Json::from_str(output.lines().last().unwrap()).unwrap();
    assert!(json.is_array());
    let array = json.as_array().unwrap();
    assert_eq!(array.len(), 3);
    assert_eq!(array[0].find("metric").unwrap().as_string().unwrap(), metric);
    assert_eq!(array[0].find("name").unwrap().as_string().unwrap(), "unit");
    assert_eq!(array[0].find("value").unwrap().as_string().unwrap(), unit);
    assert_eq!(array[1].find("metric").unwrap().as_string().unwrap(), metric);
    assert_eq!(array[1].find("name").unwrap().as_string().unwrap(), "rate");
    assert_eq!(array[1].find("value").unwrap().as_string().unwrap(), rate);
    assert_eq!(array[2].find("metric").unwrap().as_string().unwrap(), metric);
    assert_eq!(array[2].find("name").unwrap().as_string().unwrap(), "desc");
    assert_eq!(array[2].find("value").unwrap().as_string().unwrap(), description);
}

#[test]
fn send_datum() {
    let metric = "lukas.tests.count";
    let now = now_in_ms();
    let value = "42";
    let tags: Tags = Tags::new();

    let port = 18071; // Actually, we should generate a random port number and check, if it is free
    let server = run_server(port);
    let client = BosunClient::new(&format!("localhost:{}", port));
    let datum = Datum::new(&metric, now, &value, &tags);
    let result = client.emit_datum(&datum);
    assert!(result.is_ok());

    let output = server.recv()
                      .unwrap_or_else(|e| panic!("failed to wait on child: {}", e));

    println!("---{}---", output);
    assert!(output.find("Content-Type: application/json; charset=utf-8").is_some());

    let json = Json::from_str(output.lines().last().unwrap()).unwrap();
    assert_eq!(json.find("metric").unwrap().as_string().unwrap(), metric);
    assert_eq!(json.find("timestamp").unwrap().as_i64().unwrap(), now);
    assert_eq!(json.find("value").unwrap().as_string().unwrap(), value);
    assert!(json.find("tags").unwrap().is_object());
    assert!(json.find("tags").unwrap().as_object().unwrap().is_empty());
}

fn run_server(port: u16) -> Receiver<String> {
    let (tx, rx) = channel();

    let listener = TcpListener::bind(("localhost", port)).unwrap();
    // accept connections and process them, spawning a new thread for each one
    thread::spawn(move|| {
        let stream = listener.accept();
        match stream {
            Ok((mut stream, _)) => {
                let mut data = [0; 1024];
                let len = stream.read(&mut data);
                let _ = stream.write("HTTP/1.1 204 NoContent\r\n".as_bytes());
                let data_str = String::from_utf8_lossy(&data[0..len.unwrap()]).to_string();
                let _ = tx.send(data_str);
            }
            Err(err) => {
                panic!("Failed to read from stream because {}", err);
            }
        }
        // close the socket server
        drop(listener);
    });

    rx
}

#[test]
fn load_scollector_config() {
    let scollector_toml = r#"
Host = "bosun:8070"
FullHost = false
Hostname = "webserver"

[Tags]
  hostgroup = "webservers"
  domain = "webserver.de"
  hosttype = "baremetal"
"#;
    let temp_file_path = Temp::new_file().unwrap().to_path_buf();
    let mut f = File::create(&temp_file_path).unwrap();
    let _ = f.write_all(scollector_toml.as_bytes()).unwrap();
    let _ = f.sync_data().unwrap();

    let bosun_config = BosunConfig::load_from_scollector_config(&temp_file_path).unwrap();

    assert_eq!(bosun_config.Host, "bosun:8070");
    assert_eq!(bosun_config.Hostname, "webserver");
    assert_eq!(bosun_config.Tags["hostgroup"], "webservers");
    assert_eq!(bosun_config.Tags["domain"], "webserver.de");
    assert_eq!(bosun_config.Tags["hosttype"], "baremetal");
}

