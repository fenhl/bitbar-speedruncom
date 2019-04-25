#![deny(unused, unused_qualifications)]
#![forbid(unused_import_braces)]

mod config;
mod data;
mod model;
mod util;

use std::{
    env::{
        self,
        current_exe
    },
    fmt,
    fs::File,
    io::{
        self,
        prelude::*
    },
    iter,
};
use bitbar::{
    ContentItem,
    Menu,
    MenuItem
};
use chrono::{
    Duration,
    prelude::*
};
use css_color_parser::ColorParseError;
use serde_json::Value as Json;
use srcomapi::{
    client,
    model::{
        notification::Notification,
        run::RunStatus
    }
};
use wrapped_enum::wrapped_enum;
use crate::{
    config::Config,
    data::Data,
    model::Game,
    util::{
        Increment,
        format_duration
    }
};

/// A monocolored version speedrun.com's favicon.
const TROPHY: &str = "iVBORw0KGgoAAAANSUhEUgAAACQAAAAkCAYAAADhAJiYAAAABGdBTUEAALGPC/xhBQAAACBjSFJNAAB6JgAAgIQAAPoAAACA6AAAdTAAAOpgAAA6mAAAF3CculE8AAAACXBIWXMAABYlAAAWJQFJUiTwAAABWWlUWHRYTUw6Y29tLmFkb2JlLnhtcAAAAAAAPHg6eG1wbWV0YSB4bWxuczp4PSJhZG9iZTpuczptZXRhLyIgeDp4bXB0az0iWE1QIENvcmUgNS40LjAiPgogICA8cmRmOlJERiB4bWxuczpyZGY9Imh0dHA6Ly93d3cudzMub3JnLzE5OTkvMDIvMjItcmRmLXN5bnRheC1ucyMiPgogICAgICA8cmRmOkRlc2NyaXB0aW9uIHJkZjphYm91dD0iIgogICAgICAgICAgICB4bWxuczp0aWZmPSJodHRwOi8vbnMuYWRvYmUuY29tL3RpZmYvMS4wLyI+CiAgICAgICAgIDx0aWZmOk9yaWVudGF0aW9uPjE8L3RpZmY6T3JpZW50YXRpb24+CiAgICAgIDwvcmRmOkRlc2NyaXB0aW9uPgogICA8L3JkZjpSREY+CjwveDp4bXBtZXRhPgpMwidZAAAAxUlEQVRYCe2TUQ6EMAgFXe9/ZzeaTCLPtkJ0DW7oD4G28Bjaaao1JvAZbC+yx1mNy7GD27tH3FyYjZfAaamMErjahtGQmtDTZJTsRiodIY+gVbmZs7bm9F15PIKc9e45tu+83lCL6Z4Q+0+TMhpSvyEIYX9NypCh6KsIIfpuUk0yFHslIcRjo8SGREiK/QtCNHNGKkSGpOkIlSBG07NFqEeGeOQnnP0qcvasq1a6kXlUXyWjxIY10xEqQTo/9YuQEik/SuAL584NOmGKlr0AAAAASUVORK5CYII=";

#[derive(Debug)]
pub enum OtherError { //TODO fix wrapped_enum macro and change visibility to pub(crate)
    EmptyTimespec,
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
        SerDe(serde_json::Error),
        #[allow(missing_docs)]
        Timespec(timespec::Error)
    }
}

fn bitbar() -> Result<Menu, Error> {
    let mut items = Vec::default();
    let mut total = Some(0);
    let config = Config::new()?;
    let data = Data::new()?;
    let mut client_builder = client::Builder::new(concat!("bitbar-speedruncom/", env!("CARGO_PKG_VERSION")))
        .cache_timeout(Duration::hours(12)..Duration::hours(24));
    if let Ok(cache) = xdg_basedir::get_cache_home() {
        let cache_path = cache.join("bitbar/speedruncom.json");
        if File::open(&cache_path).map_err(Error::Io).and_then(|cache_file| serde_json::from_reader::<_, Json>(cache_file).map_err(Error::SerDe)).is_err() {
            writeln!(File::create(&cache_path)?, "{{}}")?;
        }
        client_builder = client_builder.disk_cache(cache_path)?;
    };
    let client_builder = client_builder.num_tries(4);
    let client = if let Some(key) = config.api_key {
        let auth_client = client_builder.auth(&key).build()?;
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
        auth_client.into()
    } else {
        client_builder.build()?
    };
    let cache = model::Cache::new(&client);
    let mut game_sections = Vec::default();
    for (game_name, game_config) in config.games {
        let game = Game::new(cache.clone(), game_name, game_config);
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
            .filter_map(|cat| cat.wr()
                .map(|wr_result| wr_result
                    .filter(|wr| !data.runs.get(wr.id()).map_or(false, |run_data| run_data.watched || run_data.deferred.map_or(false, |deferred_until| deferred_until > Utc::now())))
                    .map(|wr| (cat, wr)))
                .transpose())
            .collect::<Result<Vec<_>, _>>()?;
        records.sort_by_key(|&(_, ref wr)| wr.time());
        let fastest_time = records.first().map(|&(_, ref wr)| wr.time());
        for (cat, wr) in records {
            game_total.incr();
            let wr_item = ContentItem::new(format!("New WR in {}: {}", cat, format_duration(wr.time())));
            game_section.push(if let Ok(bin) = current_exe() {
                wr_item.sub(iter::once(
                    ContentItem::new("View Run")
                        .href(wr.weblink().clone())
                        .into()
                ).chain(
                    wr.runners()?
                        .into_iter()
                        .map(|runner| MenuItem::new(format!("Runner: {}", runner)))
                ).chain(vec![
                    MenuItem::new(match wr.date() {
                        Some(date) => format!("Recorded {}", date),
                        None => "Recorded in the Old Days".into()
                    }),
                    MenuItem::new(match wr.status() {
                        RunStatus::New => "Not yet verified".into(),
                        RunStatus::Verified { verify_date: Some(date), .. } => format!("Verified {}", date),
                        RunStatus::Verified { verify_date: None, .. } => "Verified in the Old Days".into(),
                        RunStatus::Rejected { .. } => "REJECTED".into()
                    }),
                    MenuItem::Sep,
                    ContentItem::new("Mark as Watched")
                        .command(vec![bin.to_str().ok_or(OtherError::InvalidBinPath)?, "check", wr.id()])
                        .refresh()
                        .into(),
                    //TODO “mark as partially watched” submenu
                    ContentItem::new("Defer until Tomorrow")
                        .command(vec![bin.to_str().ok_or(OtherError::InvalidBinPath)?, "defer", wr.id()])
                        .refresh()
                        .into(),
                    ContentItem::new("Defer for a Week")
                        .command(vec![bin.to_str().ok_or(OtherError::InvalidBinPath)?, "defer", wr.id(), "r:7d"])
                        .refresh()
                        .into()
                ]))
            } else {
                wr_item.href(wr.weblink().clone())
            }.into());
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
            let pos = game_sections.binary_search_by_key(&fastest_time, |&(time, _)| time).unwrap_or_else(|i| i);
            game_sections.insert(pos, (fastest_time, game_section));
        }
    }
    //TODO check for any followed games not in config
    for (_, game_section) in game_sections {
        items.extend(game_section);
    }
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

fn notify(summary: impl fmt::Display, body: impl fmt::Display) -> ! {
    //let _ = notify_rust::set_application(&notify_rust::get_bundle_identifier_or_default("BitBar")); //TODO uncomment when https://github.com/h4llow3En/mac-notification-sys/issues/8 is fixed
    let _ = notify_rust::Notification::default()
        .summary(&summary.to_string())
        .sound_name("Funk")
        .body(&body.to_string())
        .show();
    panic!("{}: {}", summary, body);
}

trait ResultExt {
    type Ok;

    fn notify(self, summary: impl fmt::Display) -> Self::Ok;
}

impl<T, E: fmt::Debug> ResultExt for Result<T, E> {
    type Ok = T;

    fn notify(self, summary: impl fmt::Display) -> T {
        match self {
            Ok(t) => t,
            Err(e) => { notify(summary, format!("{:?}", e)); }
        }
    }
}

fn check(mut args: env::Args) -> Result<(), Error> {
    let mut data = Data::new()?;
    data.runs.entry(args.next().ok_or(OtherError::MissingCliArg)?).or_default().watched = true;
    data.save()?;
    Ok(())
}

fn defer(mut args: env::Args) -> Result<(), Error> {
    let mut data = Data::new()?;
    let mut run = data.runs.entry(args.next().ok_or(OtherError::MissingCliArg)?).or_default();
    let mut args = args.peekable();
    run.deferred = Some(if args.peek().is_some() {
        timespec::next(args)?.ok_or(OtherError::EmptyTimespec)?
    } else {
        Utc::now() + Duration::days(1)
    });
    data.save()?;
    Ok(())
}

fn main() {
    let mut args = env::args();
    let _ = args.next(); // ignore executable name
    if let Some(arg) = args.next() {
        match &arg[..] {
            "check" => { check(args).notify("error in check cmd"); }
            "defer" => { defer(args).notify("error in defer cmd"); }
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
                                .color("blue").expect("failed to parse the color blue")
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
}
