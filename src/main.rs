use std::io::prelude::*;
use std::io::BufReader;
use std::net::{TcpListener, TcpStream};

mod json;

use json::Json;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    for stream in listener.incoming() {
        handle_client(stream?)?;
    }
    Ok(())
}

fn handle_client(stream: TcpStream) -> std::io::Result<()> {
    let writer = &mut stream.try_clone()?;
    let buf = BufReader::new(&stream);
    for line in buf.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match Json::parse_with_trailing_whitespace(&line) {
            Err(e) => {
                response_error(writer, -32700, Json::Null, &format!("{e}"))?;
                continue;
            }
            Ok(j) => handle_json(writer, &j)?,
        };
    }
    Ok(())
}

fn response_error(
    writer: &mut dyn Write,
    code: i32,
    id: Json,
    message: &str,
) -> std::io::Result<()> {
    writeln!(
        writer,
        "{}",
        Json::Object(vec![
            ("jsonrpc".into(), "2.0".into()),
            (
                "error".into(),
                Json::Object(vec![
                    ("code".into(), Json::Number(code as f64)),
                    ("message".into(), message.into()),
                ])
            ),
            ("id".into(), id),
        ])
        .stringify()
    )
}

fn response_invalid_request(writer: &mut dyn Write) -> std::io::Result<()> {
    response_error(writer, -32600, Json::Null, "Invalid Request")
}

fn handle_json(writer: &mut dyn Write, j: &Json) -> std::io::Result<()> {
    match j {
        Json::Object(kvs) => {
            let Some(Json::String(jsonrpc_version)) = get(&kvs, "jsonrpc") else {
                return response_invalid_request(writer);
            };
            if jsonrpc_version != "2.0" {
                return response_invalid_request(writer);
            }

            let Some(id) = get(&kvs, "id") else {
                return response_invalid_request(writer);
            };
            match id {
                Json::Null | Json::Number(_) | Json::String(_) => (),
                _ => return response_invalid_request(writer),
            }

            let Some(Json::String(method)) = get(&kvs, "method") else {
                return response_error(writer, -32601, id.clone(), "Method not found");
            };
            match method.as_str() {
                "add" => handle_method_add(writer, kvs, id),
                _ => response_error(writer, -32601, id.clone(), "Method not found"),
            }
        }
        _ => response_invalid_request(writer),
    }
}

fn get<'a>(kvs: &'a [(String, Json)], key: &str) -> Option<&'a Json> {
    for (k, v) in kvs {
        if k == key {
            return Some(v);
        }
    }
    return None;
}

fn response_invalid_parameters(writer: &mut dyn Write, id: &Json) -> std::io::Result<()> {
    response_error(writer, -32602, id.clone(), "Invalid method parameter")
}

fn handle_method_add(
    writer: &mut dyn Write,
    kvs: &[(String, Json)],
    id: &Json,
) -> std::io::Result<()> {
    let Some(Json::Array(args)) = get(&kvs, "params") else {
        return response_invalid_parameters(writer, id);
    };
    if !args.iter().all(|a| matches!(a, Json::Number(_))) {
        return response_invalid_parameters(writer, id);
    }
    let result = args.iter().fold(0.0, |acc, x| {
        if let Json::Number(x) = x {
            acc + *x
        } else {
            acc
        }
    });
    let res = Json::Object(vec![
        ("jsonrpc".into(), "2.0".into()),
        ("id".into(), id.clone()),
        ("result".into(), Json::Number(result)),
    ])
    .stringify();
    writeln!(writer, "{res}")
}
