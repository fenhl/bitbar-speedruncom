use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    iter::{
        self,
        FromIterator
    },
    rc::Rc
};
use itertools::Itertools;
use srcomapi::{
    client::Client,
    model::{
        category::{
            Category as SrcCategory,
            ToLeaderboard
        },
        game::Game as SrcGame,
        level::Level,
        run::Run,
        variable::Filter
    }
};
use crate::config::{
    ConfigCategory,
    ConfigGame
};

pub(crate) struct Cache {
    client: Client,
    src_categories: HashMap<String, SrcCategory>,
    src_games: HashMap<String, SrcGame>,
    levels: HashMap<String, Level>,
    wrs: HashMap<(String, String), Option<Run>>
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

    fn src_category(&mut self, cat_id: &str) -> Result<SrcCategory, srcomapi::Error> {
        if let Some(cat) = self.src_categories.get(cat_id) { return Ok(cat.clone()); }
        self.src_categories.insert(cat_id.to_string(), SrcCategory::from_id(&self.client, cat_id)?);
        Ok(self.src_categories[cat_id].clone())
    }

    fn src_game(&mut self, game_id: &str) -> Result<SrcGame, srcomapi::Error> {
        if let Some(game) = self.src_games.get(game_id) { return Ok(game.clone()); }
        self.src_games.insert(game_id.to_string(), SrcGame::from_id(&self.client, game_id)?);
        Ok(self.src_games[game_id].clone())
    }

    fn level(&mut self, level_id: &str) -> Result<Level, srcomapi::Error> {
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

    pub(crate) fn src_games(&self) -> Result<Vec<SrcGame>, srcomapi::Error> {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn config(&self) -> ConfigCategory {
        self.game_config.categories[&self.name].clone()
    }

    pub(crate) fn src_categories(&self) -> Result<Vec<SrcCategory>, srcomapi::Error> {
        self.config().src_categories.iter().map(|cat_id| self.cache.borrow_mut().src_category(cat_id)).collect()
    }

    fn levels(&self) -> Result<Vec<Level>, srcomapi::Error> {
        self.config().levels.iter().map(|level_id| self.cache.borrow_mut().level(level_id)).collect()
    }

    pub(crate) fn wr(&self) -> Result<Option<Run>, srcomapi::Error> {
        if let Some(opt_run) = self.cache.borrow().wrs.get(&(self.game_name.clone(), self.name.clone())) { return Ok(opt_run.clone()); }
        let mut wrs = if self.config().variable_state.is_empty() {
            self.src_categories()?
                .into_iter()
                .flat_map(|src_cat| if src_cat.is_il() {
                    match self.levels() {
                        Ok(levels) => Box::new(levels.into_iter().map(move |level| (&level, &src_cat).wr())) as Box<dyn Iterator<Item = _>>,
                        Err(e) => Box::new(iter::once(Err(e))) as Box<dyn Iterator<Item = _>>
                    }
                } else {
                    Box::new(iter::once(src_cat.wr())) as Box<dyn Iterator<Item = _>>
                })
                .collect::<Result<Vec<_>, _>>()
        } else {
            self.config().variable_state.iter()
                .map(|(var_id, values)| values.iter().map(move |value| (var_id, value)))
                .multi_cartesian_product()
                .cartesian_product(self.src_categories()?)
                .flat_map(|(filter, src_cat)| if src_cat.is_il() {
                    match self.levels() {
                        Ok(levels) => Box::new(levels.into_iter().map(move |level| (&level, &src_cat).filtered_wr(&Filter::from_iter(filter.clone())))) as Box<dyn Iterator<Item = _>>,
                        Err(e) => Box::new(iter::once(Err(e))) as Box<dyn Iterator<Item = _>>
                    }
                } else {
                    Box::new(iter::once(src_cat.filtered_wr(&Filter::from_iter(filter)))) as Box<dyn Iterator<Item = _>>
                })
                .collect()
        }?;
        let sub_wrs: Vec<Option<Run>> = self.config().subcategories.into_iter().map(|subcat_name| Category {
            cache: self.cache.clone(),
            game_name: self.game_name.clone(),
            game_config: self.game_config.clone(),
            name: subcat_name.to_string()
        }.wr()).collect::<Result<_, _>>()?;
        wrs.extend(sub_wrs);
        let opt_run = wrs.into_iter().filter_map(|x| x).min_by_key(|run| run.time());
        self.cache.borrow_mut().wrs.insert((self.game_name.clone(), self.name.clone()), opt_run.clone());
        Ok(opt_run)
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.name.fmt(f)
    }
}
