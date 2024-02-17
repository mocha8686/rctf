pub mod cache {
    use std::{
        fs::{self, File},
        io::Read,
        path::PathBuf,
    };

    use anyhow::{bail, Result};
    use directories::ProjectDirs;
    use serde::{Deserialize, Serialize};

    pub fn load<T>(filename: &str) -> Result<T>
    where
        for<'de> T: Deserialize<'de>,
    {
        let Some(path) = create_path(filename) else {
            bail!("Failed to create path to {}.", filename);
        };

        match File::open(path) {
            Ok(mut file) => {
                let mut buf = String::new();
                file.read_to_string(&mut buf)?;
                let res: T = serde_json::from_str(&buf)?;
                Ok(res)
            }
            Err(e) => bail!(e),
        }
    }

    pub fn save<T: Serialize>(filename: &str, data: T) -> Result<()> {
        let Some(path) = create_path(filename) else {
            bail!("Failed to get cache directory.");
        };

        let file = File::create(path)?;
        serde_json::to_writer(file, &data)?;

        Ok(())
    }

    fn create_path(filename: &str) -> Option<PathBuf> {
        let Some(dir) =
            ProjectDirs::from("", "", env!("CARGO_PKG_NAME")).map(|dir| dir.cache_dir().to_owned())
        else {
            return None;
        };
        match fs::create_dir_all(&dir) {
            Ok(_) => Some(dir.join(filename)),
            Err(_) => None,
        }
    }
}
