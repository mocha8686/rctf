use anyhow::Result;
use rctf::{files::cache, Context};

const RCTF_HISTORY_FILENAME: &str = "history";
const TERMCRAFT_HISTORY_FILENAME: &str = "termcraft_history";

#[tokio::main]
async fn main() -> Result<()> {
    // let ssh_settings: Settings = Config::builder()
    //     .add_source(config::Environment::with_prefix("RCTF"))
    //     .add_source(config::File::with_name("./rctf.ini"))
    //     .build()?
    //     .try_deserialize()?;

    let rctf_history = cache::load(RCTF_HISTORY_FILENAME).ok();
    let termcraft_history = cache::load(TERMCRAFT_HISTORY_FILENAME).ok();

    let mut context = Context::new(rctf_history, termcraft_history)?;
    context.start().await?;

    cache::save(RCTF_HISTORY_FILENAME, context.rctf_history()).ok();
    cache::save(TERMCRAFT_HISTORY_FILENAME, context.termcraft_history()).ok();

    Ok(())
}
