//! Walking-skeleton transport: subscribe to this device's inbox over Convex and
//! echo a reply. Throwaway — superseded by the real envelope + auth in later 2a.

use std::{collections::BTreeMap, env};
use convex::{ConvexClient, FunctionResult, Value};
use futures::StreamExt;

pub async fn run() -> anyhow::Result<()> {
    dotenvy::from_filename(".env.local").ok();
    let url = env::var("CONVEX_URL").map_err(|_| anyhow::anyhow!("set CONVEX_URL to the deployment URL"))?;
    let mut client = ConvexClient::new(&url).await?;
    let mut writer = client.clone();

    let mut args = BTreeMap::new();
    args.insert("recipient".to_string(), Value::String("agent".to_string()));
    let mut sub = client.subscribe("messages:inbox", args).await?;

    println!("agent relay: listening on Convex inbox…");
    while let Some(FunctionResult::Value(Value::Array(rows))) = sub.next().await {
        for row in rows {
            if let Value::Object(obj) = row {
                if let Some(Value::String(body)) = obj.get("body") {
                    println!("agent received: {body}");
                    let mut reply = BTreeMap::new();
                    reply.insert("recipient".to_string(), Value::String("app".to_string()));
                    reply.insert("body".to_string(), Value::String(format!("ack: {body}")));
                    writer.mutation("messages:send", reply).await?;
                }
            }
        }
    }
    Ok(())
}
