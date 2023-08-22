use std::path::PathBuf;

use anyhow::{bail, Result};
use directories::ProjectDirs;
use itertools::Itertools;
use rctf::Context;
use tokio::{
    fs::{File, self},
    io::{AsyncReadExt, AsyncWriteExt},
};

#[tokio::main]
async fn main() -> Result<()> {
    // let ssh_settings: Settings = Config::builder()
    //     .add_source(config::Environment::with_prefix("RCTF"))
    //     .add_source(config::File::with_name("./rctf.ini"))
    //     .build()?
    //     .try_deserialize()?;

    let rctf_history_path = {
        if let Some(project_dirs) = ProjectDirs::from("", "", "rctf") {
            let mut path = PathBuf::from(project_dirs.cache_dir());
            fs::create_dir_all(&path).await?;
            path.push("history");
            Some(path)
        } else {
            None
        }
    };

    let rctf_history = if let Some(ref rctf_history_path) = rctf_history_path {
        match File::open(rctf_history_path).await {
            Ok(mut file) => {
                let mut buf = String::new();
                file.read_to_string(&mut buf).await?;
                Some(buf.split("\n").map(|s| s.to_string()).collect())
            }
            Err(e) if e.kind() == tokio::io::ErrorKind::NotFound => None,
            Err(e) => bail!(e),
        }
    } else {
        None
    };

    let mut context = Context::new(rctf_history)?;
    context.start().await?;

    if let Some(rctf_history_path) = rctf_history_path {
        let mut rctf_history = File::create(rctf_history_path).await?;
        rctf_history.write_all(&mut context.rctf_history().iter().join("\n").as_bytes()).await?;
    }

    Ok(())
}
