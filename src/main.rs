use anyhow::Result;
use config::Config;
use rctf::{Context, Settings};

#[tokio::main]
async fn main() -> Result<()> {
    let ssh_settings: Settings = Config::builder()
        .add_source(config::Environment::with_prefix("RCTF"))
        .add_source(config::File::with_name("./rctf.ini"))
        .build()?
        .try_deserialize()?;

    let context = Context::new(ssh_settings)?;

    context.start().await?;

    Ok(())
}
