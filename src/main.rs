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
        let is_notify = match Json::parse_with_trailing_whitespace(&line) {
            Err(_) => response_error(writer, -32700, Json::Null, "Parse error").map(|_| false)?,
            Ok(j) => handle_json(writer, &j)?,
        };
        if !is_notify {
            write!(writer, "\n")?;
        }
    }
    Ok(())
}

fn response_error(
    writer: &mut dyn Write,
    code: i32,
    id: Json,
    message: &str,
) -> std::io::Result<()> {
    write!(
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

// return whether the request is notification
fn handle_json(writer: &mut dyn Write, j: &Json) -> std::io::Result<bool> {
    match j {
        Json::Object(kvs) => handle_single_json(writer, &kvs),
        Json::Array(js) => {
            if js.is_empty() {
                return response_invalid_request(writer).map(|_| false);
            }
            let mut res = Vec::<u8>::new();
            let mut is_first = true;
            for j in js {
                let mut res_j = Vec::<u8>::new();
                let is_notify = match j {
                    Json::Object(kvs) => handle_single_json(&mut res_j, &kvs)?,
                    _ => response_invalid_request(&mut res_j).map(|_| false)?,
                };
                if !is_notify {
                    if is_first {
                        is_first = false;
                        write!(res, "[")?;
                    } else {
                        write!(res, ",")?;
                    }
                    res.write_all(&res_j)?;
                }
            }
            if !is_first {
                writer.write_all(&res)?;
                write!(writer, "]")?;
                Ok(false)
            } else {
                Ok(true)
            }
        }
        _ => response_invalid_request(writer).map(|_| false),
    }
}

// return whether the request is notification
fn handle_single_json(writer: &mut dyn Write, kvs: &[(String, Json)]) -> std::io::Result<bool> {
    let Some(Json::String(jsonrpc_version)) = get(&kvs, "jsonrpc") else {
        return response_invalid_request(writer).map(|_| false);
    };
    if jsonrpc_version != "2.0" {
        return response_invalid_request(writer).map(|_| false);
    }

    // simple ignore notification
    let Some(id) = get(&kvs, "id") else {
        return Ok(true);
    };
    match id {
        Json::Number(_) | Json::String(_) => (),
        Json::Null => return Ok(true),
        _ => return response_invalid_request(writer).map(|_| false),
    }

    let Some(Json::String(method)) = get(&kvs, "method") else {
        return response_error(writer, -32601, id.clone(), "Method not found").map(|_| false);
    };
    match method.as_str() {
        "add" => handle_method_add(writer, kvs, id).map(|_| false),
        "subtract" => handle_method_subtract(writer, kvs, id).map(|_| false),
        _ => response_error(writer, -32601, id.clone(), "Method not found").map(|_| false),
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
    let mut result = 0.0;
    for arg in args {
        if let Json::Number(x) = arg {
            result += x;
        } else {
            return response_invalid_parameters(writer, id);
        }
    }
    let res = Json::Object(vec![
        ("jsonrpc".into(), "2.0".into()),
        ("id".into(), id.clone()),
        ("result".into(), Json::Number(result)),
    ])
    .stringify();
    write!(writer, "{res}")
}

fn handle_method_subtract(
    writer: &mut dyn Write,
    kvs: &[(String, Json)],
    id: &Json,
) -> std::io::Result<()> {
    let Some(Json::Array(args)) = get(&kvs, "params") else {
        return response_invalid_parameters(writer, id);
    };
    if args.len() != 2 {
        return response_invalid_parameters(writer, id);
    }
    let mut arg_nums = [0.0; 2];
    for (i, arg) in args.iter().enumerate() {
        if let Json::Number(x) = arg {
            arg_nums[i] = *x;
        } else {
            return response_invalid_parameters(writer, id);
        }
    }
    let result = arg_nums[0] - arg_nums[1];
    let res = Json::Object(vec![
        ("jsonrpc".into(), "2.0".into()),
        ("id".into(), id.clone()),
        ("result".into(), Json::Number(result)),
    ])
    .stringify();
    write!(writer, "{res}")
}
