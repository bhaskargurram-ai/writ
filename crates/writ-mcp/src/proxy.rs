//! The MCP proxy core (interception mode A, spec §6/§11): an MCP server to
//! the agent, an MCP client to the real server. `tools/call` is intercepted
//! and routed through a decision callback; everything else passes through.
//!
//! Wave 1 limitation (documented): downstream-initiated requests
//! (server→agent) are not proxied; SSE/HTTP transports land in wave 2.

use serde_json::Value;
use writ_core::call::{CallerIdentity, InterceptMode, ServerIdentity, ToolCall, TrustVerdict};
use writ_core::{Result, Timestamp, WritError};

use crate::jsonrpc::{JsonRpcError, JsonRpcMessage, JsonRpcRequest, JsonRpcResponse};
use crate::transport::Transport;

/// What policy decided about one `tools/call`.
pub enum ProxyDecision {
    Forward,
    /// Full structured refusal message (rule id + reason), shown to the
    /// model so it can self-correct (spec §7).
    Refuse { message: String },
}

/// Configuration for one proxied downstream server.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub server_name: String,
    /// Human name of the agent product on the upstream side.
    pub agent: String,
    pub trust: Option<TrustVerdict>,
}

/// JSON-RPC error code used for writ refusals (server-defined range).
pub const WRIT_REFUSAL_CODE: i64 = -32043;

pub struct McpProxy<A: Transport, D: Transport> {
    /// Public so tests and the CLI can inspect traffic after `run`.
    pub agent_side: A,
    pub downstream: D,
    pub config: ProxyConfig,
    pub session_id: String,
    /// Auto-discovered downstream tool schemas (spec §11), populated on
    /// tools/list responses.
    pub tool_schemas: Vec<Value>,
    decide: Box<dyn FnMut(&ToolCall) -> ProxyDecision>,
    observe: Option<Box<dyn FnMut(&ToolCall, &Value)>>,
    /// Result transform applied BEFORE the result re-enters the agent's
    /// context (the `redact` verdict, spec §7). The observation hook sees the
    /// original; the agent sees the transform's output.
    transform: Option<Box<dyn FnMut(&ToolCall, Value) -> Value>>,
}

impl<A: Transport, D: Transport> McpProxy<A, D> {
    pub fn new(
        agent_side: A,
        downstream: D,
        config: ProxyConfig,
        decide: Box<dyn FnMut(&ToolCall) -> ProxyDecision>,
    ) -> Self {
        let session_id = format!("mcp-{}-{}", std::process::id(), Timestamp::now().epoch_ms());
        McpProxy {
            agent_side,
            downstream,
            config,
            session_id,
            tool_schemas: Vec::new(),
            decide,
            observe: None,
            transform: None,
        }
    }

    pub fn on_result(&mut self, hook: Box<dyn FnMut(&ToolCall, &Value)>) {
        self.observe = Some(hook);
    }

    /// Register the redaction transform (see field docs for ordering).
    pub fn set_result_transform(&mut self, hook: Box<dyn FnMut(&ToolCall, Value) -> Value>) {
        self.transform = Some(hook);
    }

    fn make_call(&self, req: &JsonRpcRequest) -> ToolCall {
        let params = req.params.clone().unwrap_or(Value::Null);
        ToolCall {
            call_id: format!("{}:{}", self.session_id, req.id),
            session_id: self.session_id.clone(),
            caller: CallerIdentity {
                agent: self.config.agent.clone(),
                agent_version: None,
                user: std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .ok(),
                non_human_id: None,
            },
            mode: InterceptMode::Mcp,
            tool: params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("<unknown>")
                .to_string(),
            args: params.get("arguments").cloned().unwrap_or(Value::Null),
            server: Some(ServerIdentity {
                name: self.config.server_name.clone(),
                transport: "stdio".to_string(),
                version: None,
            }),
            trust: self.config.trust,
            captured_at: Timestamp::now(),
        }
    }
    /// Forward a request downstream and relay its response upstream.
    /// Server→agent notifications received while waiting are relayed;
    /// server→agent requests get a method-not-found error (documented
    /// wave-1 limitation above).
    fn forward_request(&mut self, req: &JsonRpcRequest) -> Result<()> {
        self.downstream.send(&JsonRpcMessage::Request(req.clone()))?;
        loop {
            match self.downstream.recv()? {
                Some(JsonRpcMessage::Response(resp)) => {
                    if resp.id == req.id {
                        if req.method == "tools/list" {
                            self.cache_tools(&resp);
                        }
                        self.agent_side.send(&JsonRpcMessage::Response(resp))?;
                        return Ok(());
                    }
                    tracing::warn!(id = %resp.id, "dropping unmatched downstream response");
                }
                Some(JsonRpcMessage::Notification(n)) => {
                    self.agent_side.send(&JsonRpcMessage::Notification(n))?;
                }
                Some(JsonRpcMessage::Request(r)) => {
                    let err = JsonRpcResponse::error(
                        r.id,
                        JsonRpcError {
                            code: -32601,
                            message: "writ proxy: server-initiated requests are not proxied (wave 1)"
                                .into(),
                            data: None,
                        },
                    );
                    self.downstream.send(&JsonRpcMessage::Response(err))?;
                }
                None => {
                    return Err(WritError::Intercept(
                        "downstream closed while awaiting response".into(),
                    ))
                }
            }
        }
    }

    fn cache_tools(&mut self, resp: &JsonRpcResponse) {
        if let Some(tools) = resp
            .result
            .as_ref()
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
        {
            self.tool_schemas = tools.clone();
        }
    }

    /// Intercept a `tools/call`: ask the decider, then forward or refuse.
    fn handle_tool_call(&mut self, req: &JsonRpcRequest) -> Result<()> {
        let call = self.make_call(req);
        match (self.decide)(&call) {
            ProxyDecision::Forward => {
                self.downstream.send(&JsonRpcMessage::Request(req.clone()))?;
                match self.downstream.recv()? {
                    Some(JsonRpcMessage::Response(mut resp)) => {
                        // 1. Observe the ORIGINAL (ledger hashes the unmasked
                        //    result — spec §7 records "a hash of the original").
                        if let (Some(hook), Some(result)) =
                            (&mut self.observe, resp.result.clone())
                        {
                            hook(&call, &result);
                        }
                        // 2. Transform (redact) before the agent sees it.
                        if let (Some(t), Some(result)) = (&mut self.transform, resp.result.take())
                        {
                            resp.result = Some(t(&call, result));
                        }
                        self.agent_side.send(&JsonRpcMessage::Response(resp))?;
                        Ok(())
                    }
                    Some(other) => {
                        self.agent_side.send(&other)?;
                        Err(WritError::Intercept(
                            "unexpected downstream message during tools/call".into(),
                        ))
                    }
                    None => Err(WritError::Intercept(
                        "downstream closed during tools/call".into(),
                    )),
                }
            }
            ProxyDecision::Refuse { message } => {
                let resp = JsonRpcResponse::error(
                    req.id.clone(),
                    JsonRpcError {
                        code: WRIT_REFUSAL_CODE,
                        message,
                        data: Some(serde_json::json!({ "writ_refusal": true })),
                    },
                );
                self.agent_side.send(&JsonRpcMessage::Response(resp))
            }
        }
    }

    /// Run the proxy loop until the agent closes the connection (EOF).
    pub fn run(&mut self) -> Result<()> {
        loop {
            match self.agent_side.recv()? {
                None => return Ok(()),
                Some(JsonRpcMessage::Request(req)) => {
                    if req.method == "tools/call" {
                        self.handle_tool_call(&req)?;
                    } else {
                        self.forward_request(&req)?;
                    }
                }
                Some(JsonRpcMessage::Notification(n)) => {
                    self.downstream.send(&JsonRpcMessage::Notification(n))?;
                }
                Some(JsonRpcMessage::Response(resp)) => {
                    // Agent answering a server-initiated request; forward.
                    self.downstream.send(&JsonRpcMessage::Response(resp))?;
                }
            }
        }
    }
}
