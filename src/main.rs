use anyhow::Result;
use rctf::Context;

#[tokio::main]
async fn main() -> Result<()> {
    // let ssh_settings: Settings = Config::builder()
    //     .add_source(config::Environment::with_prefix("RCTF"))
    //     .add_source(config::File::with_name("./rctf.ini"))
    //     .build()?
    //     .try_deserialize()?;

    let context = Context::new()?;

    context.start().await?;

    Ok(())
}
