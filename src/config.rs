use {
    std::{
        collections::{
            BTreeMap,
            BTreeSet
        },
        fs::File
    },
    serde::{
        Deserialize,
        Serialize
    },
    crate::Error
};

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ConfigCategory {
    pub(crate) src_categories: BTreeSet<String>,
    pub(crate) variable_state: BTreeMap<String, BTreeSet<String>>,
    pub(crate) subcategories: BTreeSet<String>,
    pub(crate) levels: BTreeSet<String> //TODO read in model
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ConfigGame {
    /// maps SRC game IDs to their ignored categories
    pub(crate) src_games: BTreeMap<String, Vec<String>>,
    pub(crate) categories: BTreeMap<String, ConfigCategory>
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Config {
    pub(crate) api_key: Option<String>,
    pub(crate) games: BTreeMap<String, ConfigGame>
}

impl Config {
    pub(crate) fn new() -> Result<Config, Error> {
        let dirs = xdg_basedir::get_config_home().into_iter().chain(xdg_basedir::get_config_dirs());
        let file = dirs.filter_map(|cfg_dir| File::open(cfg_dir.join("bitbar/plugins/speedruncom.json")).ok())
            .next().ok_or(Error::MissingConfig)?;
        Ok(serde_json::from_reader(file)?)
    }

    /*
    pub(crate) fn save(self) -> Result<(), Error> {
        let dirs = xdg_basedir::get_config_home().into_iter().chain(xdg_basedir::get_config_dirs());
        for cfg_dir in dirs {
            let cfg_path = cfg_dir.join("bitbar/plugins/speedruncom.json");
            if cfg_path.exists() {
                if let Some(()) = File::create(cfg_path).ok()
                    .and_then(|cfg_file| serde_json::to_writer_pretty(cfg_file, &self).ok())
                {
                    return Ok(());
                }
            }
        }
        let cfg_path = xdg_basedir::get_config_home()?.join("bitbar/plugins/speedruncom.json");
        let cfg_file = File::create(cfg_path)?;
        serde_json::to_writer_pretty(cfg_file, &self)?;
        Ok(())
    }
    */
}
