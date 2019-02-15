use std::{
    collections::HashMap,
    fs::File
};
use serde_derive::{
    Deserialize,
    Serialize
};
use serde_json;
use xdg_basedir;
use crate::Error;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct CacheRun {
    pub(crate) watched: bool
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Cache {
    pub(crate) runs: HashMap<String, CacheRun>
}

impl Cache {
    pub(crate) fn new() -> Result<Cache, Error> {
        let dirs = xdg_basedir::get_data_home().into_iter().chain(xdg_basedir::get_data_dirs());
        Ok(dirs.filter_map(|cache_dir| File::open(cache_dir.join("bitbar/plugin-cache/srcomapi.json")).ok())
            .next().map_or(Ok(Cache::default()), serde_json::from_reader)?)
    }

    pub(crate) fn save(self) -> Result<(), Error> {
        let dirs = xdg_basedir::get_data_home().into_iter().chain(xdg_basedir::get_data_dirs());
        for cache_dir in dirs {
            let cache_path = cache_dir.join("bitbar/plugin-cache/srcomapi.json");
            if cache_path.exists() {
                if let Some(()) = File::create(cache_path).ok()
                    .and_then(|cache_file| serde_json::to_writer_pretty(cache_file, &self).ok())
                {
                    return Ok(());
                }
            }
        }
        let cache_path = xdg_basedir::get_data_home()?.join("bitbar/plugin-cache/srcomapi.json");
        let cache_file = File::create(cache_path)?;
        serde_json::to_writer_pretty(cache_file, &self)?;
        Ok(())
    }
}
