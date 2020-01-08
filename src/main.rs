#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

use {
    std::{
        convert::Infallible,
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
        path::Path,
        process::{
            Command,
            ExitStatus
        }
    },
    bitbar::{
        ContentItem,
        Menu,
        MenuItem
    },
    chrono::{
        Duration,
        prelude::*
    },
    css_color_parser::ColorParseError,
    derive_more::From,
    serde_json::Value as Json,
    srcomapi::{
        client::{
            self,
            Client
        },
        model::{
            notification::Notification,
            run::{
                Run,
                RunStatus
            }
        }
    },
    crate::{
        config::Config,
        data::Data,
        model::Game,
        util::{
            CommandStatusExt as _,
            Increment as _,
            ResultNeverExt as _,
            format_duration
        }
    }
};

mod config;
mod data;
mod model;
mod util;

const IINA_PATH: &str = "/usr/local/bin/iina";

#[derive(Debug, From)]
pub(crate) enum Error {
    Api(srcomapi::Error),
    Basedir(xdg_basedir::Error),
    ColorParse(ColorParseError),
    CommandExit(&'static str, ExitStatus),
    EmptyTimespec,
    Fmt(fmt::Error),
    InvalidBinPath,
    Io(io::Error),
    MissingCliArg,
    MissingConfig,
    NoSuchCategory {
        game_name: String,
        cat_name: String
    },
    SerDe(serde_json::Error),
    Timespec(timespec::Error),
    UrlParse(url::ParseError)
}

impl From<Infallible> for Error {
    fn from(never: Infallible) -> Error {
        match never {}
    }
}

fn bitbar() -> Result<Menu, Error> {
    let mut items = Vec::default();
    let mut total = Some(0);
    let config = Config::new()?;
    let data = Data::new()?;
    let (client, notif_items, notif_total) = get_client(&config)?;
    items.extend(notif_items);
    total.incr_by(notif_total);
    let cache = model::Cache::new(&client);
    let mut game_sections = Vec::default();
    let current_exe = current_exe();
    for (game_name, game_config) in config.games {
        let game = Game::new(cache.clone(), game_name, game_config);
        let mut game_total = Some(0);
        let mut game_section = vec![
            MenuItem::Sep,
            MenuItem::Content(ContentItem::new(&game)
                .sub(game.src_games()?.into_iter().map(|src_game| Ok(ContentItem::new(&src_game)
                    .href(src_game.weblink().clone())?
                    .alt(ContentItem::new(src_game.id()))
                    .into()
                )).collect::<Result<Vec<_>, Error>>()?)
            ),
        ];
        let mut records = game.categories()
            .into_iter()
            .filter_map(|cat| cat.watchable_wrs(&data)
                .map(|wrs_result| {
                    if wrs_result.iter().any(|wr| data.runs.get(wr.id()).map_or(false, |run_data| run_data.watched)) {
                        None // don't show runs that are tied with watched runs
                    } else {
                        wrs_result.into_iter()
                            .filter(|wr| !data.runs.get(wr.id()).map_or(false, |run_data| run_data.deferred.map_or(false, |deferred_until| deferred_until > Utc::now())))
                            .next()
                    }.map(|wr| (cat, wr))
                })
                .transpose())
            .collect::<Result<Vec<_>, _>>()?;
        records.sort_by_key(|&(_, ref wr)| wr.time());
        let fastest_time = records.first().map(|&(_, ref wr)| wr.time());
        for (cat, wr) in records {
            game_total.incr();
            let wr_item = ContentItem::new(format!("New WR in {}: {}", cat, format_duration(wr.time())));
            game_section.push(if let Ok(ref bin) = current_exe {
                wr_item.sub(if wr.videos().next().is_some() {
                    if Path::new(IINA_PATH).exists() {
                        Box::new(iter::once(
                            ContentItem::new("Watch Run")
                                .command((bin.to_str().ok_or(Error::InvalidBinPath)?, "watch", wr.id()))
                                .into()
                        )) as Box<dyn Iterator<Item = MenuItem>>
                    } else {
                        let videos = wr.videos().collect::<Vec<_>>();
                        let single = videos.len() == 1;
                        Box::new(
                            videos.into_iter().enumerate().map(move |(i, video)|
                                ContentItem::new(if single { format!("Watch Run") } else { format!("Watch Part {}", i + 1) })
                                    .href(video.clone()).expect("failed to convert URL to URL") //TODO add support for opening certain websites in IINA
                                    .into()
                            )
                        )
                    }
                } else {
                    Box::new(iter::empty())
                }.chain({
                    let item = ContentItem::new("View Run Page")
                        .href(wr.weblink().clone())?
                        .into();
                    iter::once(item)
                }).chain(
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
                        .command([bin.to_str().ok_or(Error::InvalidBinPath)?, "check", wr.id()])
                        //.refresh() //TODO make sure multiple instances of bitbar-speedruncom running simultaneously works correctly, then uncomment this
                        .into(),
                    //TODO “mark as partially watched” submenu
                    ContentItem::new("Defer until Tomorrow")
                        .command([bin.to_str().ok_or(Error::InvalidBinPath)?, "defer", wr.id()])
                        //.refresh() //TODO make sure multiple instances of bitbar-speedruncom running simultaneously works correctly, then uncomment this
                        .into(),
                    ContentItem::new("Defer for a Week")
                        .command([bin.to_str().ok_or(Error::InvalidBinPath)?, "defer", wr.id(), "r:7d"])
                        //.refresh() //TODO make sure multiple instances of bitbar-speedruncom running simultaneously works correctly, then uncomment this
                        .into(),
                    ContentItem::new("Mark as Unwatchable")
                        .command([bin.to_str().ok_or(Error::InvalidBinPath)?, "unwatchable", wr.id()])
                        //.refresh() //TODO make sure multiple instances of bitbar-speedruncom running simultaneously works correctly, then uncomment this
                        .into()
                ]))
            } else {
                wr_item.href(wr.weblink().clone())?
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
                .template_image(&include_bytes!("../assets/trophy.png")[..])?
            )
        ).chain(items).collect()
    } else {
        Menu::default()
    })
}

fn get_client(config: &Config) -> Result<(Client, Vec<MenuItem>, Option<usize>), Error> {
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
    let mut items = Vec::default();
    let mut total = Some(0);
    Ok((if let Some(ref key) = config.api_key {
        let auth_client = client_builder.auth(&key).build()?;
        let notifications = Notification::list::<Vec<_>>(&auth_client)?.into_iter().filter(|note| !note.read()).collect::<Vec<_>>();
        if !notifications.is_empty() {
            total.incr_by(Some(notifications.len()));
            items.push(MenuItem::Sep);
            for note in notifications {
                items.push(ContentItem::new(&note)
                    .href(note.weblink().clone())?
                    .into()
                );
            }
        }
        auth_client.into()
    } else {
        client_builder.build()?
    }, items, total))
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
    data.runs.entry(args.next().ok_or(Error::MissingCliArg)?).or_default().watched = true;
    data.save()?;
    Ok(())
}

fn defer(mut args: env::Args) -> Result<(), Error> {
    let mut data = Data::new()?;
    let mut run = data.runs.entry(args.next().ok_or(Error::MissingCliArg)?).or_default();
    let mut args = args.peekable();
    run.deferred = Some(if args.peek().is_some() {
        timespec::next(args)?.ok_or(Error::EmptyTimespec)?
    } else {
        Utc::now() + Duration::days(1)
    });
    data.save()?;
    Ok(())
}

fn unwatchable(mut args: env::Args) -> Result<(), Error> {
    let mut data = Data::new()?;
    data.runs.entry(args.next().ok_or(Error::MissingCliArg)?).or_default().unwatchable = true;
    data.save()?;
    Ok(())
}

fn watch(mut args: env::Args) -> Result<(), Error> {
    let client = get_client(&Config::new()?)?.0;
    let run = Run::from_id(&client, args.next().ok_or(Error::MissingCliArg)?)?;
    for video_url in run.videos() {
        Command::new(IINA_PATH)
            .arg("--separate-windows")
            .arg("--no-stdin")
            .arg("--keep-running")
            .arg(video_url.to_string())
            .check("iina")?;
    }
    let mut data = Data::new()?;
    data.runs.entry(run.id().to_string()).or_default().watched = true;
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
            "unwatchable" => { unwatchable(args).notify("error in unwatchable cmd"); }
            "watch" => { watch(args).notify("error in watch cmd"); }
            subcmd => { panic!("unknown subcommand: {:?}", subcmd); }
        }
    } else {
        match bitbar() {
            Ok(menu) => { print!("{}", menu); }
            Err(e) => {
                let mut error_menu = vec![
                    ContentItem::new("?").template_image(&include_bytes!("../assets/trophy.png")[..]).never_unwrap().into(),
                    MenuItem::Sep
                ];
                match e {
                    Error::Api(srcomapi::Error::Reqwest(e)) => {
                        error_menu.push(MenuItem::new(format!("API returned error: {}", e)));
                        if let Some(url) = e.url() {
                            error_menu.push(ContentItem::new(format!("URL: {}", url))
                                .href(url.clone()).expect("failed to add link to error menu")
                                .color("blue").expect("failed to parse the color blue")
                                .into());
                        }
                    }
                    Error::MissingConfig => { error_menu.push(MenuItem::new(format!("missing or invalid configuration file"))); } //TODO better error message
                    Error::NoSuchCategory { game_name, cat_name } => { error_menu.push(MenuItem::new(format!("reference to unconfigured category {} in game {}", cat_name, game_name))); }
                    Error::SerDe(e) => { error_menu.push(MenuItem::new(format!("error in config file: {}", e))); }
                    e => { error_menu.push(MenuItem::new(format!("{:?}", e))); } //TODO handle separately
                }
                print!("{}", Menu(error_menu));
            }
        }
    }
}
