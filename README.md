# jsonrpc-demo-rs

implement [JSON-RPC](https://www.jsonrpc.org/index.html) over [NDJSON](https://github.com/ndjson/ndjson-spec) with Rust

## Usage

Run the program in a shell:

```shell
cargo run
```

Send requests in another shell:

```shell
$ echo -e '{"jsonrpc":"2.0","method":"add","params":[1,2,3,4],"id": 1}\n{"jsonrpc":"2.0","method":"add","params":[-1,1],"id":2}' | nc 127.0.0.1 7878
{"jsonrpc":"2.0","id":1,"result":10}
{"jsonrpc":"2.0","id":2,"result":0}

# with error:

$ echo -e '{"jsonrpc":"2.0","method":"add","par}\n{"jsonrpc":"2.0","method":"add","params":[-1,1],"id":2}' | nc 127.0.0.1 7878
{"jsonrpc":"2.0","error":{"code":-32700,"message":"JSON Parse Error at 37: unexpected EOF"},"id":null}
{"jsonrpc":"2.0","id":2,"result":0}
```
