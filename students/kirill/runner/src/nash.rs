use reqwest::{Client, Method};
use runvil::{
    Error, ErrorCode, make_tool_error,
    plugin::{PackInfo, PackRegistry, Tool, ToolContext, Tools},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub fn install(packs: &mut PackRegistry, server: &str) -> Result<(), Error> {
    let server = server.trim_end_matches('/').to_owned();
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| make_tool_error(ErrorCode::ToolOther, error.to_string()))?;
    packs.insert(NASH, Tools::default().with(Nash { client, server }))
}

const NASH: PackInfo = PackInfo::new("kirill/nash").description("Send requests to the Nash host");

struct Nash {
    client: Client,
    server: String,
}

#[derive(Deserialize, JsonSchema)]
struct Request {
    method: Verb,
    #[schemars(description = "Path on the Nash host, starting with /api/.")]
    path: String,
    #[schemars(description = "JSON text to send with a PUT.")]
    body: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
enum Verb {
    Get,
    Put,
    Delete,
}

#[derive(Serialize)]
struct Reply {
    status: u16,
    body: String,
}

impl Tool for Nash {
    const NAME: &'static str = "request";
    const DESCRIPTION: &'static str =
        "Send one request to the Nash host and return its status and body as JSON.";
    type Args = Request;
    type Output = Reply;

    async fn call(&self, _context: ToolContext<'_>, args: Request) -> Result<Reply, Error> {
        if !args.path.starts_with("/api/") {
            return Err(make_tool_error(
                ErrorCode::ToolInvalidArguments,
                "path must start with /api/",
            ));
        }
        let method = match args.method {
            Verb::Get => Method::GET,
            Verb::Put => Method::PUT,
            Verb::Delete => Method::DELETE,
        };
        let mut request = self
            .client
            .request(method, format!("{}{}", self.server, args.path));
        if let Some(body) = args.body {
            request = request
                .header("content-type", "application/json")
                .body(body);
        }
        let unreachable = |error: reqwest::Error| {
            make_tool_error(
                ErrorCode::ToolTransport,
                format!("the Nash host at {} did not answer: {error}", self.server),
            )
        };
        let response = request.send().await.map_err(unreachable)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(unreachable)?;
        Ok(Reply { status, body })
    }
}
