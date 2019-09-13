use {
    std::{
        cell::RefCell,
        collections::HashMap,
        fmt,
        iter::{
            self,
            FromIterator as _
        },
        rc::Rc
    },
    itertools::Itertools as _,
    srcomapi::{
        client::Client,
        model::{
            category::{
                Category as SrcCategory,
                ToLeaderboard as _
            },
            game::Game as SrcGame,
            level::Level,
            run::Run,
            variable::Filter
        }
    },
    crate::{
        Error,
        config::{
            ConfigCategory,
            ConfigGame
        },
        data::Data
    }
};

pub(crate) struct Cache {
    client: Client,
    src_categories: HashMap<String, SrcCategory>,
    src_games: HashMap<String, SrcGame>,
    levels: HashMap<String, Level>,
    wrs: HashMap<(String, String), Vec<Run>>
}

impl Cache {
    pub(crate) fn new(client: &Client) -> Rc<RefCell<Cache>> {
        Rc::new(RefCell::new(Cache {
            client: client.clone(),
            levels: HashMap::default(),
            src_categories: HashMap::default(),
            src_games: HashMap::default(),
            wrs: HashMap::default()
        }))
    }

    fn src_category(&mut self, cat_id: &str) -> Result<SrcCategory, Error> {
        if let Some(cat) = self.src_categories.get(cat_id) { return Ok(cat.clone()); }
        self.src_categories.insert(cat_id.to_string(), SrcCategory::from_id(&self.client, cat_id)?);
        Ok(self.src_categories[cat_id].clone())
    }

    fn src_game(&mut self, game_id: &str) -> Result<SrcGame, Error> {
        if let Some(game) = self.src_games.get(game_id) { return Ok(game.clone()); }
        self.src_games.insert(game_id.to_string(), SrcGame::from_id(&self.client, game_id)?);
        Ok(self.src_games[game_id].clone())
    }

    fn level(&mut self, level_id: &str) -> Result<Level, Error> {
        if let Some(level) = self.levels.get(level_id) { return Ok(level.clone()); }
        self.levels.insert(level_id.to_string(), Level::from_id(&self.client, level_id)?);
        Ok(self.levels[level_id].clone())
    }
}

pub(crate) struct Game {
    cache: Rc<RefCell<Cache>>,
    name: String,
    config: ConfigGame
}

impl Game {
    pub(crate) fn new(cache: Rc<RefCell<Cache>>, name: String, config: ConfigGame) -> Game {
        Game { cache, name, config }
    }

    pub(crate) fn src_games(&self) -> Result<Vec<SrcGame>, Error> {
        self.config.src_games.keys().map(|game_id| self.cache.borrow_mut().src_game(game_id)).collect()
    }

    pub(crate) fn categories(&self) -> Vec<Category> {
        self.config.categories.keys().map(|name| Category {
            cache: self.cache.clone(),
            game_name: self.name.clone(),
            game_config: self.config.clone(),
            name: name.to_string()
        }).collect()
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}

pub(crate) struct Category {
    cache: Rc<RefCell<Cache>>,
    game_name: String,
    game_config: ConfigGame,
    name: String,
}

impl Category {
    fn config(&self) -> Result<ConfigCategory, Error> {
        Ok(self.game_config.categories.get(&self.name).ok_or(Error::NoSuchCategory { game_name: self.game_name.clone(), cat_name: self.name.clone() })?.clone())
    }

    pub(crate) fn src_categories(&self) -> Result<Vec<SrcCategory>, Error> {
        self.config()?.src_categories.iter().map(|cat_id| self.cache.borrow_mut().src_category(cat_id)).collect()
    }

    fn levels(&self) -> Result<Vec<Level>, Error> {
        self.config()?.levels.iter().map(|level_id| self.cache.borrow_mut().level(level_id)).collect()
    }

    pub(crate) fn watchable_wrs(&self, data: &Data) -> Result<Vec<Run>, Error> {
        if let Some(runs) = self.cache.borrow().wrs.get(&(self.game_name.clone(), self.name.clone())) { return Ok(runs.clone()); }
        let mut wrs = Vec::default();
        let subcategories = if self.config()?.variable_state.is_empty() {
            Box::new(self.src_categories()?.into_iter().map(|cat| (None, cat)))
        } else {
            Box::new(
                self.config()?.variable_state.into_iter()
                    .map(|(var_id, values)| values.into_iter().map(|value| (var_id.clone(), value)).collect::<Vec<_>>())
                    .multi_cartesian_product()
                    .map(|filter| Some(Filter::from_iter(filter)))
                    .cartesian_product(self.src_categories()?)
            ) as Box<dyn Iterator<Item = _>>
        };
        for (filter, src_cat) in subcategories {
            let lbs: Box<dyn Iterator<Item = srcomapi::Result<Vec<Run>>>> = if src_cat.is_il() {
                Box::new(self.levels()?.into_iter().map(|level| if let Some(ref filter) = filter {
                    (&level, &src_cat).filtered_leaderboard::<Vec<_>>(filter)
                } else {
                    (&level, &src_cat).leaderboard::<Vec<_>>()
                }))
            } else {
                Box::new(iter::once(if let Some(filter) = filter {
                    src_cat.filtered_leaderboard::<Vec<_>>(&filter)
                } else {
                    src_cat.leaderboard::<Vec<_>>()
                })) as Box<dyn Iterator<Item = _>>
            };
            for lb in lbs {
                wrs.extend({
                    lb?
                        .into_iter()
                        .filter(|run| !data.runs.get(run.id()).map_or(false, |run_data| run_data.unwatchable))
                        .scan(None, |fastest_time, run| match *fastest_time {
                            Some(t) => if run.time() > t { None } else { Some(run) },
                            None => {
                                *fastest_time = Some(run.time());
                                Some(run)
                            }
                        })
                });
            }
        }
        let sub_wrss = self.config()?.subcategories.into_iter().map(|subcat_name| Category {
            cache: self.cache.clone(),
            game_name: self.game_name.clone(),
            game_config: self.game_config.clone(),
            name: subcat_name.to_string()
        }.watchable_wrs(data)).collect::<Result<Vec<_>, _>>()?;
        for sub_wrs in sub_wrss {
            wrs.extend(sub_wrs);
        }
        let runs = if let Some(fastest_time) = wrs.iter().map(|run| run.time()).min() {
            wrs.into_iter().filter(|run| run.time() == fastest_time).collect()
        } else {
            Vec::default()
        };
        self.cache.borrow_mut().wrs.insert((self.game_name.clone(), self.name.clone()), runs.clone());
        Ok(runs)
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}
