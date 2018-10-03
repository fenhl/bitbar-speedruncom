#![warn(trivial_casts)]
#![deny(unused, unused_qualifications)]
#![forbid(unused_import_braces)]

#[macro_use] extern crate wrapped_enum;

use std::fmt::{
    self,
    Write
};

/// A monocolored version speedrun.com's favicon.
const TROPHY: &str = "iVBORw0KGgoAAAANSUhEUgAAACQAAAAkCAYAAADhAJiYAAAABGdBTUEAALGPC/xhBQAAACBjSFJNAAB6JgAAgIQAAPoAAACA6AAAdTAAAOpgAAA6mAAAF3CculE8AAAACXBIWXMAABYlAAAWJQFJUiTwAAABWWlUWHRYTUw6Y29tLmFkb2JlLnhtcAAAAAAAPHg6eG1wbWV0YSB4bWxuczp4PSJhZG9iZTpuczptZXRhLyIgeDp4bXB0az0iWE1QIENvcmUgNS40LjAiPgogICA8cmRmOlJERiB4bWxuczpyZGY9Imh0dHA6Ly93d3cudzMub3JnLzE5OTkvMDIvMjItcmRmLXN5bnRheC1ucyMiPgogICAgICA8cmRmOkRlc2NyaXB0aW9uIHJkZjphYm91dD0iIgogICAgICAgICAgICB4bWxuczp0aWZmPSJodHRwOi8vbnMuYWRvYmUuY29tL3RpZmYvMS4wLyI+CiAgICAgICAgIDx0aWZmOk9yaWVudGF0aW9uPjE8L3RpZmY6T3JpZW50YXRpb24+CiAgICAgIDwvcmRmOkRlc2NyaXB0aW9uPgogICA8L3JkZjpSREY+CjwveDp4bXBtZXRhPgpMwidZAAAAxUlEQVRYCe2TUQ6EMAgFXe9/ZzeaTCLPtkJ0DW7oD4G28Bjaaao1JvAZbC+yx1mNy7GD27tH3FyYjZfAaamMErjahtGQmtDTZJTsRiodIY+gVbmZs7bm9F15PIKc9e45tu+83lCL6Z4Q+0+TMhpSvyEIYX9NypCh6KsIIfpuUk0yFHslIcRjo8SGREiK/QtCNHNGKkSGpOkIlSBG07NFqEeGeOQnnP0qcvasq1a6kXlUXyWjxIY10xEqQTo/9YuQEik/SuAL584NOmGKlr0AAAAASUVORK5CYII=";

wrapped_enum! {
    #[derive(Debug)]
    enum Error {
        Fmt(fmt::Error)
    }
}

fn bitbar() -> Result<String, Error> {
    let mut text = String::default();
    writeln!(&mut text, "?|templateImage={}\n", TROPHY)?;
    writeln!(&mut text, "---")?;
    writeln!(&mut text, "Not yet implemented")?;
    Ok(text)
}

fn main() {
    match bitbar() {
        Ok(text) => { print!("{}", text); }
        Err(e) => {
            println!("?|templateImage={}", TROPHY);
            println!("---");
            println!("{:?}", e); //TODO handle different kinds of errors separately
        }
    }
}
