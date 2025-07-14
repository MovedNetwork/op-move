use {serde::Serialize, umi_api::jsonrpc::JsonRpcResponse};

#[derive(Debug, Serialize)]
pub struct MirrorLog<'a> {
    pub request: &'a serde_json::Value,
    pub op_move_response: &'a JsonRpcResponse,
    pub port: &'a str,
}
