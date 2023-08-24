use anyhow::Result;
use itertools::Itertools;
use rctf::{files::cache, Context};

#[tokio::main]
async fn main() -> Result<()> {
    // let ssh_settings: Settings = Config::builder()
    //     .add_source(config::Environment::with_prefix("RCTF"))
    //     .add_source(config::File::with_name("./rctf.ini"))
    //     .build()?
    //     .try_deserialize()?;

    let rctf_history = cache::load("history", |data: Box<[u8]>| {
        Ok(std::str::from_utf8(&data)?
            .split("\n")
            .map(|s| s.to_string())
            .collect())
    })
    .await
    .ok()
    .flatten();

    let mut context = Context::new(rctf_history)?;
    context.start().await?;

    cache::save("history", context.rctf_history(), |history| {
        Ok(history.iter().join("\n").as_bytes().to_owned())
    })
    .await
    .ok();

    Ok(())
}
