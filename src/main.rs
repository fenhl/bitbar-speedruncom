#![warn(trivial_casts)]
#![deny(unused, unused_qualifications)]
#![forbid(unused_import_braces)]

#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate srcomapi;
#[macro_use] extern crate wrapped_enum;
extern crate xdg_basedir;

mod cache;
mod config;
mod util;

use std::{
    env,
    fmt::{
        self,
        Write
    },
    io
};
use srcomapi::{
    client::{
        Auth,
        Client,
        NoAuth
    },
    model::{
        game::Game,
        notification::Notification
    }
};
use self::{
    cache::Cache,
    config::{
        Config,
        ConfigVariable,
        VariableMode
    },
    util::{
        Increment,
        NatJoin,
        format_duration
    }
};

/// A monocolored version speedrun.com's favicon.
const TROPHY: &str = "iVBORw0KGgoAAAANSUhEUgAAACQAAAAkCAYAAADhAJiYAAAABGdBTUEAALGPC/xhBQAAACBjSFJNAAB6JgAAgIQAAPoAAACA6AAAdTAAAOpgAAA6mAAAF3CculE8AAAACXBIWXMAABYlAAAWJQFJUiTwAAABWWlUWHRYTUw6Y29tLmFkb2JlLnhtcAAAAAAAPHg6eG1wbWV0YSB4bWxuczp4PSJhZG9iZTpuczptZXRhLyIgeDp4bXB0az0iWE1QIENvcmUgNS40LjAiPgogICA8cmRmOlJERiB4bWxuczpyZGY9Imh0dHA6Ly93d3cudzMub3JnLzE5OTkvMDIvMjItcmRmLXN5bnRheC1ucyMiPgogICAgICA8cmRmOkRlc2NyaXB0aW9uIHJkZjphYm91dD0iIgogICAgICAgICAgICB4bWxuczp0aWZmPSJodHRwOi8vbnMuYWRvYmUuY29tL3RpZmYvMS4wLyI+CiAgICAgICAgIDx0aWZmOk9yaWVudGF0aW9uPjE8L3RpZmY6T3JpZW50YXRpb24+CiAgICAgIDwvcmRmOkRlc2NyaXB0aW9uPgogICA8L3JkZjpSREY+CjwveDp4bXBtZXRhPgpMwidZAAAAxUlEQVRYCe2TUQ6EMAgFXe9/ZzeaTCLPtkJ0DW7oD4G28Bjaaao1JvAZbC+yx1mNy7GD27tH3FyYjZfAaamMErjahtGQmtDTZJTsRiodIY+gVbmZs7bm9F15PIKc9e45tu+83lCL6Z4Q+0+TMhpSvyEIYX9NypCh6KsIIfpuUk0yFHslIcRjo8SGREiK/QtCNHNGKkSGpOkIlSBG07NFqEeGeOQnnP0qcvasq1a6kXlUXyWjxIY10xEqQTo/9YuQEik/SuAL584NOmGKlr0AAAAASUVORK5CYII=";

#[derive(Debug)]
pub enum OtherError {
    InvalidBinPath,
    InvalidVariableMode,
    MissingCliArg,
    MissingConfig
}

wrapped_enum! {
    #[derive(Debug)]
    pub enum Error {
        #[allow(missing_docs)]
        Api(srcomapi::Error),
        #[allow(missing_docs)]
        Basedir(xdg_basedir::Error),
        #[allow(missing_docs)]
        Fmt(fmt::Error),
        #[allow(missing_docs)]
        Io(io::Error),
        #[allow(missing_docs)]
        Other(OtherError),
        #[allow(missing_docs)]
        SerDe(serde_json::Error)
    }
}

fn bitbar() -> Result<String, Error> {
    let mut text = String::default();
    let mut total = Some(0);
    let config = Config::new()?;
    let cache = Cache::new()?;
    let client;
    if let Some(key) = config.api_key {
        let auth_client = Client::<Auth>::new(concat!("bitbar-speedruncom/", env!("CARGO_PKG_VERSION")), &key)?;
        let notifications = Notification::list::<Vec<_>>(&auth_client)?.into_iter().filter(|note| !note.read()).collect::<Vec<_>>();
        if !notifications.is_empty() {
            total.incr_by(Some(notifications.len()));
            writeln!(&mut text, "---")?;
            for note in notifications {
                writeln!(&mut text, "{}", note)?; //TODO link
            }
        }
        client = auth_client.into();
    } else {
        client = Client::<NoAuth>::new(concat!("bitbar-speedruncom/", env!("CARGO_PKG_VERSION")))?;
    }
    for (game_id, mut game_config) in config.games {
        let game = Game::from_id(&client, game_id)?;
        let mut game_text = String::default();
        let mut game_total = Some(0);
        writeln!(&mut game_text, "---")?;
        writeln!(&mut game_text, "{}|href={}", game, game.weblink())?;
        writeln!(&mut game_text, "{}|alternate=true", game.id())?;
        'cat: for cat in game.categories::<Vec<_>>()? {
            let mut cat_config = game_config.categories.remove(cat.id()).unwrap_or_default();
            if cat_config.ignore { continue; }
            if cat.is_il() {
                game_total = None; //TODO
                writeln!(&mut game_text, "Unconfigured IL category: {} ({})|color=red", cat, cat.id())?; //TODO
            } else {
                for var in cat.variables::<Vec<_>>()? {
                    if let Some(var_config) = cat_config.variables.remove(var.id()) {
                        match var_config.mode {
                            VariableMode::Collapse => { continue; }
                            VariableMode::Expand => {
                                game_total = None; //TODO
                                writeln!(&mut game_text, "Variable mode “expand” not yet implemented|color=red")?; //TODO
                                continue 'cat;
                            }
                        }
                    } else {
                        game_total = None;
                        writeln!(&mut game_text, "Unconfigured variable {} ({}) for category {} ({})|color=red", var, var.id(), cat, cat.id())?;
                        writeln!(&mut game_text, "--Possible values:")?;
                        for value in var.values() {
                            writeln!(&mut game_text, "--{} ({})", value.label(), value.id())?;
                        }
                        if let Some(ref bin) = config.bin {
                            writeln!(&mut game_text, "-----")?;
                            writeln!(&mut game_text, "--Collapse|bash={} param1=conf-var param2=collapse param3={} param4={} param5={} terminal=false refresh=true", bin.to_str().ok_or(OtherError::InvalidBinPath)?, game.id(), cat.id(), var.id())?;
                            writeln!(&mut game_text, "--Expand|bash={} param1=conf-var param2=expand param3={} param4={} param5={} terminal=false refresh=true", bin.to_str().ok_or(OtherError::InvalidBinPath)?, game.id(), cat.id(), var.id())?;
                            //TODO BitBar UI for creating subcategory relations
                        }
                        continue 'cat;
                    }
                }
                for (unknown_var, _) in cat_config.variables {
                    game_total = None;
                    writeln!(&mut game_text, "Unknown variable ID {} in config for category {} ({})|color=red", unknown_var, cat, cat.id())?;
                }
                let wr = match cat.wr()? {
                    Some(wr) => wr,
                    None => {
                        game_total = None;
                        writeln!(&mut game_text, "Missing or tied WR in {}|color=red", cat)?;
                        continue;
                    }
                };
                if !cache.runs.get(wr.id()).map_or(false, |cache_run| cache_run.watched) {
                    game_total.incr();
                    if let Some(ref bin) = config.bin {
                        writeln!(&mut game_text, "New WR in {}: {} by {}", cat, format_duration(wr.time()), wr.runners()?.natjoin_fallback("no one"))?;
                        writeln!(&mut game_text, "--View Run|href={}", wr.weblink())?;
                        writeln!(&mut game_text, "--Mark as Watched|bash={} param1=check param2={} terminal=false refresh=true", bin.to_str().ok_or(OtherError::InvalidBinPath)?, wr.id())?;
                        //TODO “mark as partially watched” submenu
                    } else {
                        writeln!(&mut game_text, "New WR in {}: {} by {}|href={}", cat, format_duration(wr.time()), wr.runners()?.natjoin_fallback("no one"), wr.weblink())?;
                    }
                }
            }
        }
        for (unknown_cat, _) in game_config.categories {
            game_total = None;
            writeln!(&mut game_text, "Unknown category ID {} in config|color=red", unknown_cat)?;
        }
        if game_total.map_or(true, |t| t > 0) {
            total.incr_by(game_total);
            write!(&mut text, "{}", game_text)?;
        }
    }
    Ok(if total.map_or(true, |total| total > 0) {
        format!("{}|templateImage={}\n{}", total.map_or("?".into(), |total| total.to_string()), TROPHY, text)
    } else {
        String::default()
    })
}

fn main() -> Result<(), Error> { //TODO handle errors in commands in a way that makes them visible when invoked from BitBar
    let mut args = env::args();
    let _ = args.next(); // ignore executable name
    if let Some(arg) = args.next() {
        match &arg[..] {
            "check" => {
                let mut cache = Cache::new()?;
                cache.runs.entry(args.next().ok_or(OtherError::MissingCliArg)?).or_default().watched = true;
                cache.save()?;
            }
            "conf-var" => {
                let mut config = Config::new()?;
                let mode = args.next().ok_or(OtherError::MissingCliArg)?.parse()?;
                match mode {
                    VariableMode::Collapse => {
                        config
                            .games.entry(args.next().ok_or(OtherError::MissingCliArg)?).or_default()
                            .categories.entry(args.next().ok_or(OtherError::MissingCliArg)?).or_default()
                            .variables.insert(args.next().ok_or(OtherError::MissingCliArg)?, ConfigVariable {
                                mode: VariableMode::Collapse
                            });
                    }
                    VariableMode::Expand => {
                        config
                            .games.entry(args.next().ok_or(OtherError::MissingCliArg)?).or_default()
                            .categories.entry(args.next().ok_or(OtherError::MissingCliArg)?).or_default()
                            .variables.insert(args.next().ok_or(OtherError::MissingCliArg)?, ConfigVariable {
                                mode: VariableMode::Expand
                            });
                    }
                }
                config.save()?;
            }
            subcmd => { panic!("unknown subcommand: {:?}", subcmd); }
        }
    } else {
        match bitbar() {
            Ok(text) => { print!("{}", text); }
            Err(e) => {
                println!("?|templateImage={}", TROPHY);
                println!("---");
                match e {
                    Error::Api(srcomapi::Error::Reqwest(e)) => {
                        println!("API returned error: {}", e);
                        if let Some(url) = e.url() {
                            println!("URL: {}", url);
                        }
                    }
                    Error::Other(OtherError::MissingConfig) => { println!("missing or invalid configuration file"); } //TODO better error message
                    Error::SerDe(e) => { println!("error in config file: {}", e); }
                    e => { println!("{:?}", e); } //TODO handle separately
                }
            }
        }
    }
    Ok(())
}
