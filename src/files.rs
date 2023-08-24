pub mod cache {
    use std::path::PathBuf;

    use anyhow::bail;
    use anyhow::Result;
    use directories::ProjectDirs;
    use tokio::fs;
    use tokio::fs::File;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    pub async fn load<'a, Data, T, F>(filename: &str, deserialize: F) -> Result<Option<T>>
    where
        Data: From<Vec<u8>>,
        F: FnOnce(Data) -> Result<T>,
    {
        let Some(path) = create_path(filename).await else {
            return Ok(None);
        };

        match File::open(path).await {
            Ok(mut file) => {
                let mut buf = vec![];
                file.read_to_end(&mut buf).await?;
                let res = deserialize(buf.into())?;
                Ok(Some(res))
            }
            Err(e) if e.kind() == tokio::io::ErrorKind::NotFound => Ok(None),
            Err(e) => bail!(e),
        }
    }

    pub async fn save<'a, Data, T, F>(filename: &str, data: Data, serialize: F) -> Result<()>
    where
        T: Into<Box<[u8]>>,
        F: FnOnce(Data) -> Result<T>,
    {
        let Some(path) = create_path(filename).await else {
            bail!("Failed to get cache directory.");
        };

        let mut file = File::create(path).await?;
        file.write_all(&serialize(data)?.into()).await?;

        Ok(())
    }

    async fn create_path(filename: &str) -> Option<PathBuf> {
        let Some(dir) = ProjectDirs::from("", "", "rctf").map(|dir| dir.cache_dir().to_owned())
        else {
            return None;
        };
        match fs::create_dir_all(&dir).await {
            Ok(_) => Some(dir.join(filename)),
            Err(_) => None,
        }
    }
}
