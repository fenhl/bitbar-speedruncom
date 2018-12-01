use std::{
    collections::{
        BTreeMap,
        HashMap
    },
    fs::File,
    path::PathBuf,
    str::FromStr
};
use serde_json;
use xdg_basedir;
use super::{
    Error,
    OtherError
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VariableMode {
    /// The variable is ignored. All runs regardless of value of this variable are considered part of the same category.
    Collapse,
    /// Each possible value of the variable is considered a separate category.
    Expand
}

impl FromStr for VariableMode {
    /// The given string is not a variable mode
    type Err = Error;

    fn from_str(s: &str) -> Result<VariableMode, Error> {
        match s {
            "collapse" => Ok(VariableMode::Collapse),
            "expand" => Ok(VariableMode::Expand),
            _ => Err(OtherError::InvalidVariableMode.into())
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigVariable {
    pub mode: VariableMode
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ConfigCategory {
    pub ignore: bool,
    //pub subcategories: Vec<String>, //TODO transitively include subcategories when determining both current world record and fastest watched run, even if the subcategory is ignored
    pub variables: HashMap<String, ConfigVariable>
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ConfigGame {
    pub categories: HashMap<String, ConfigCategory>,
    //pub show_with: Option<String> //TODO use this to group games together, treating them as the same game. Useful for extension categories
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Config {
    pub api_key: Option<String>,
    pub bin: Option<PathBuf>,
    pub games: BTreeMap<String, ConfigGame>
}

impl Config {
    pub fn new() -> Result<Config, Error> {
        let dirs = xdg_basedir::get_config_home().into_iter().chain(xdg_basedir::get_config_dirs());
        let file = dirs.filter_map(|cfg_dir| File::open(cfg_dir.join("bitbar/plugins/speedruncom.json")).ok())
            .next().ok_or(OtherError::MissingConfig)?;
        Ok(serde_json::from_reader(file)?)
    }

    pub fn save(self) -> Result<(), Error> {
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
}
