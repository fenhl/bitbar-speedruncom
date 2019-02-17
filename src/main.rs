#![deny(unused, unused_qualifications)]
#![forbid(unused_import_braces)]

mod cache;
mod config;
mod model;
mod util;

use std::{
    env,
    fmt,
    io,
    iter
};
use bitbar::{
    ContentItem,
    Menu,
    MenuItem
};
use css_color_parser::ColorParseError;
use srcomapi::{
    client::{
        Auth,
        Client,
        NoAuth
    },
    model::notification::Notification
};
use wrapped_enum::wrapped_enum;
use crate::{
    cache::Cache,
    config::Config,
    model::Game,
    util::{
        Increment,
        NatJoin,
        format_duration
    }
};

/// A monocolored version speedrun.com's favicon.
const TROPHY: &str = "iVBORw0KGgoAAAANSUhEUgAAACQAAAAkCAYAAADhAJiYAAAABGdBTUEAALGPC/xhBQAAACBjSFJNAAB6JgAAgIQAAPoAAACA6AAAdTAAAOpgAAA6mAAAF3CculE8AAAACXBIWXMAABYlAAAWJQFJUiTwAAABWWlUWHRYTUw6Y29tLmFkb2JlLnhtcAAAAAAAPHg6eG1wbWV0YSB4bWxuczp4PSJhZG9iZTpuczptZXRhLyIgeDp4bXB0az0iWE1QIENvcmUgNS40LjAiPgogICA8cmRmOlJERiB4bWxuczpyZGY9Imh0dHA6Ly93d3cudzMub3JnLzE5OTkvMDIvMjItcmRmLXN5bnRheC1ucyMiPgogICAgICA8cmRmOkRlc2NyaXB0aW9uIHJkZjphYm91dD0iIgogICAgICAgICAgICB4bWxuczp0aWZmPSJodHRwOi8vbnMuYWRvYmUuY29tL3RpZmYvMS4wLyI+CiAgICAgICAgIDx0aWZmOk9yaWVudGF0aW9uPjE8L3RpZmY6T3JpZW50YXRpb24+CiAgICAgIDwvcmRmOkRlc2NyaXB0aW9uPgogICA8L3JkZjpSREY+CjwveDp4bXBtZXRhPgpMwidZAAAAxUlEQVRYCe2TUQ6EMAgFXe9/ZzeaTCLPtkJ0DW7oD4G28Bjaaao1JvAZbC+yx1mNy7GD27tH3FyYjZfAaamMErjahtGQmtDTZJTsRiodIY+gVbmZs7bm9F15PIKc9e45tu+83lCL6Z4Q+0+TMhpSvyEIYX9NypCh6KsIIfpuUk0yFHslIcRjo8SGREiK/QtCNHNGKkSGpOkIlSBG07NFqEeGeOQnnP0qcvasq1a6kXlUXyWjxIY10xEqQTo/9YuQEik/SuAL584NOmGKlr0AAAAASUVORK5CYII=";

#[derive(Debug)]
pub enum OtherError { //TODO fix wrapped_enum macro and change visibility to pub(crate)
    InvalidBinPath,
    MissingCliArg,
    MissingConfig,
    NoSuchCategory {
        game_name: String,
        cat_name: String
    }
}

wrapped_enum! {
    #[derive(Debug)]
    pub enum Error { //TODO fix wrapped_enum macro and change visibility to pub(crate)
        #[allow(missing_docs)]
        Api(srcomapi::Error),
        #[allow(missing_docs)]
        Basedir(xdg_basedir::Error),
        #[allow(missing_docs)]
        ColorParse(ColorParseError),
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

fn bitbar() -> Result<Menu, Error> {
    let mut items = Vec::default();
    let mut total = Some(0);
    let config = Config::new()?;
    let cache = Cache::new()?;
    let client;
    if let Some(key) = config.api_key {
        let auth_client = Client::<Auth>::new(concat!("bitbar-speedruncom/", env!("CARGO_PKG_VERSION")), &key)?;
        let notifications = Notification::list::<Vec<_>>(&auth_client)?.into_iter().filter(|note| !note.read()).collect::<Vec<_>>();
        if !notifications.is_empty() {
            total.incr_by(Some(notifications.len()));
            items.push(MenuItem::Sep);
            for note in notifications {
                items.push(ContentItem::new(&note)
                    .href(note.weblink().clone())
                    .into()
                );
            }
        }
        client = auth_client.into();
    } else {
        client = Client::<NoAuth>::new(concat!("bitbar-speedruncom/", env!("CARGO_PKG_VERSION")))?;
    }
    let runtime_cache = model::Cache::new(&client);
    for (game_name, game_config) in config.games {
        let game = Game::new(runtime_cache.clone(), game_name, game_config);
        let mut game_total = Some(0);
        let mut game_section = vec![
            MenuItem::Sep,
            MenuItem::Content(ContentItem::new(&game)
                .sub(game.src_games()?.into_iter().map(|src_game| ContentItem::new(&src_game)
                    .href(src_game.weblink().clone())
                    .alt(ContentItem::new(src_game.id()))
                    .into()
                ))
            ),
        ];
        let mut records = game.categories()
            .into_iter()
            //.filter_map(|cat| cat.wr().transpose().map(|wr| wr.map(|wr| (cat, wr)))) //TODO use when transpose is stabilized (#47338)
            .filter_map(|cat| match cat.wr() {
                Ok(Some(wr)) => Some(Ok((cat, wr))),
                Ok(None) => None,
                Err(e) => Some(Err(e))
            })
            .collect::<Result<Vec<_>, _>>()?;
        records.sort_by_key(|&(_, ref wr)| wr.time());
        for (cat, wr) in records {
            if !cache.runs.get(wr.id()).map_or(false, |cache_run| cache_run.watched) {
                game_total.incr();
                let wr_item = ContentItem::new(format!("New WR in {}: {} by {}", cat, format_duration(wr.time()), wr.runners()?.natjoin_fallback("no one")));
                game_section.push(if let Some(ref bin) = config.bin {
                    wr_item.sub(vec![
                        ContentItem::new("View Run")
                            .href(wr.weblink().clone())
                            .into(),
                        ContentItem::new("Mark as Watched")
                            .command(vec![bin.to_str().ok_or(OtherError::InvalidBinPath)?, "check", wr.id()])
                            .refresh()
                            .into()
                        //TODO “mark as partially watched” submenu
                    ])
                } else {
                    wr_item.href(wr.weblink().clone())
                }.into());
            }
        }
        //TODO Unconfigured categories check
        /*
        for (unknown_cat, _) in game_config.categories {
            game_total = None;
            writeln!(&mut game_text, "Unknown category ID {} in config|color=red", unknown_cat)?;
        }
        */
        if game_total.map_or(true, |t| t > 0) {
            total.incr_by(game_total);
            items.extend(game_section);
        }
    }
    //TODO check for any followed games not in config
    Ok(if total.map_or(true, |total| total > 0) {
        iter::once(
            MenuItem::Content(ContentItem::new(total.map_or("?".into(), |total| total.to_string()))
                .template_image(TROPHY)
            )
        ).chain(items).collect()
    } else {
        Menu::default()
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
            subcmd => { panic!("unknown subcommand: {:?}", subcmd); }
        }
    } else {
        match bitbar() {
            Ok(menu) => { print!("{}", menu); }
            Err(e) => {
                let mut error_menu = vec![
                    ContentItem::new("?").template_image(TROPHY).into(),
                    MenuItem::Sep
                ];
                match e {
                    Error::Api(srcomapi::Error::Reqwest(e)) => {
                        error_menu.push(MenuItem::new(format!("API returned error: {}", e)));
                        if let Some(url) = e.url() {
                            error_menu.push(ContentItem::new(format!("URL: {}", url))
                                .href(url.clone())
                                .color("blue")?
                                .into());
                        }
                    }
                    Error::Other(OtherError::MissingConfig) => { error_menu.push(MenuItem::new(format!("missing or invalid configuration file"))); } //TODO better error message
                    Error::Other(OtherError::NoSuchCategory { game_name, cat_name }) => { error_menu.push(MenuItem::new(format!("reference to unconfigured category {} in game {}", cat_name, game_name))); }
                    Error::SerDe(e) => { error_menu.push(MenuItem::new(format!("error in config file: {}", e))); }
                    e => { error_menu.push(MenuItem::new(format!("{:?}", e))); } //TODO handle separately
                }
                print!("{}", Menu(error_menu));
            }
        }
    }
    Ok(())
}
