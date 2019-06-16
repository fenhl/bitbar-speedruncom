use std::{
    collections::HashMap,
    fs::File
};
use chrono::prelude::*;
use serde_derive::{
    Deserialize,
    Serialize
};
use serde_json;
use xdg_basedir;
use crate::Error;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct RunData {
    #[serde(default)]
    pub(crate) deferred: Option<DateTime<Utc>>,
    #[serde(default)]
    pub(crate) unwatchable: bool,
    pub(crate) watched: bool
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Data {
    pub(crate) runs: HashMap<String, RunData>
}

impl Data {
    pub(crate) fn new() -> Result<Data, Error> {
        let dirs = xdg_basedir::get_data_home().into_iter().chain(xdg_basedir::get_data_dirs());
        Ok(dirs.filter_map(|data_dir| File::open(data_dir.join("bitbar/plugin-cache/srcomapi.json")).ok())
            .next().map_or(Ok(Data::default()), serde_json::from_reader)?)
    }

    pub(crate) fn save(self) -> Result<(), Error> {
        let dirs = xdg_basedir::get_data_home().into_iter().chain(xdg_basedir::get_data_dirs());
        for data_dir in dirs {
            let data_path = data_dir.join("bitbar/plugin-cache/srcomapi.json");
            if data_path.exists() {
                if let Some(()) = File::create(data_path).ok()
                    .and_then(|data_file| serde_json::to_writer_pretty(data_file, &self).ok())
                {
                    return Ok(());
                }
            }
        }
        let data_path = xdg_basedir::get_data_home()?.join("bitbar/plugin-cache/srcomapi.json");
        let data_file = File::create(data_path)?;
        serde_json::to_writer_pretty(data_file, &self)?;
        Ok(())
    }
}
