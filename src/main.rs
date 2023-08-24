use anyhow::Result;
use rctf::{files::cache, Context};

#[tokio::main]
async fn main() -> Result<()> {
    // let ssh_settings: Settings = Config::builder()
    //     .add_source(config::Environment::with_prefix("RCTF"))
    //     .add_source(config::File::with_name("./rctf.ini"))
    //     .build()?
    //     .try_deserialize()?;

    let rctf_history = cache::load("history").ok().flatten();

    let mut context = Context::new(rctf_history)?;
    context.start().await?;

    cache::save("history", context.rctf_history()).ok();

    Ok(())
}
