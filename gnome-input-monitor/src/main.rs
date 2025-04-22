use zbus::{Connection, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let connection = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.IBus",  // Service name
        "/org/freedesktop/IBus", // Object path
        "org.freedesktop.IBus",  // Interface name
    )
    .await?;

    // Call the GetCurrentEngine method
    // The actual method might differ slightly depending on IBus version or configuration.
    // Common methods are GetCurrentEngine or CurrentEngineDesc
    // We'll try GetCurrentEngine first.
    match proxy.call_method("GetCurrentEngine", &()).await {
        Ok(reply) => {
            let engine_name: String = reply.body().deserialize()?;
            println!("Current IBus engine: {}", engine_name);
        }
        Err(e) => {
            eprintln!("Failed to get current engine: {}", e);
            // Optionally, try another method name if the first one fails
            // eprintln!("Trying CurrentEngineDesc...");
            // match proxy.call_method("CurrentEngineDesc", &()).await {
            //     Ok(reply) => {
            //         let engine_desc: String = reply.body().deserialize()?;
            //         println!("Current IBus engine description: {}", engine_desc);
            //     }
            //     Err(e2) => {
            //         eprintln!("Failed to get current engine description either: {}", e2);
            //     }
            // }
        }
    }

    Ok(())
}
