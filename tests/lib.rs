extern crate bosun_emitter;
extern crate rustc_serialize;

use bosun_emitter::*;
use rustc_serialize::json::Json;
use std::io::{Read, Write};
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
    let _ = client.emit_metadata(&metadata);
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
    let _ = client.emit_datum(&datum);
    let output = server.recv()
                      .unwrap_or_else(|e| panic!("failed to wait on child: {}", e));

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

