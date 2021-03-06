use {
    std::{
        convert::Infallible,
        fmt,
        process::Command,
        time::Duration
    },
    crate::Error
};

pub(crate) trait CommandStatusExt {
    type Ok;

    fn check(&mut self, name: &'static str) -> Result<Self::Ok, Error>;
}

impl CommandStatusExt for Command {
    type Ok = ();

    fn check(&mut self, name: &'static str) -> Result<(), Error> {
        let status = self.status()?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::CommandExit(name, status))
        }
    }
}

pub(crate) trait Increment {
    fn incr_by(&mut self, amount: Option<usize>);

    fn incr(&mut self) {
        self.incr_by(Some(1));
    }
}

impl Increment for Option<usize> {
    fn incr_by(&mut self, amount: Option<usize>) {
        *self = self.and_then(|value| amount.map(|amount| value + amount));
    }
}

pub(crate) trait NatJoin {
    fn natjoin(self) -> Option<String>;

    fn natjoin_fallback(self, fallback: impl ToString) -> String where Self: Sized {
        self.natjoin().unwrap_or_else(|| fallback.to_string())
    }
}

impl<T: fmt::Display, I: IntoIterator<Item = T>> NatJoin for I {
    fn natjoin(self) -> Option<String> {
        let collection = self.into_iter().map(|item| item.to_string()).collect::<Vec<_>>();
        match collection.len() {
            0 => None,
            1 => Some(collection[0].to_string()),
            2 => Some(format!("{} and {}", collection[0], collection[1])),
            _ => {
                let (last, head) = collection.split_last().unwrap();
                Some(format!("{}, and {}", head.join(", "), last))
            }
        }
    }
}

pub(crate) trait ResultNeverExt {
    type Ok;

    fn never_unwrap(self) -> Self::Ok;
}

impl<T> ResultNeverExt for Result<T, Infallible> {
    type Ok = T;

    fn never_unwrap(self) -> T {
        match self {
            Ok(x) => x,
            Err(never) => match never {}
        }
    }
}

pub(crate) fn format_duration(duration: Duration) -> String {
    const ONE_HOUR: Duration = Duration::from_secs(3600);
    const ONE_MINUTE: Duration = Duration::from_secs(60);

    if duration == Duration::default() {
        return "0s".into();
    }
    let mut result = if duration >= ONE_HOUR {
        let hours = duration.as_secs() / 3600;
        let minutes = (duration.as_secs() % 3600) / 60;
        let seconds = duration.as_secs() % 60;
        format!("{}h {:02}m {:02}", hours, minutes, seconds)
    } else if duration >= ONE_MINUTE {
        let minutes = duration.as_secs() / 60;
        let seconds = duration.as_secs() % 60;
        format!("{}m {:02}", minutes, seconds)
    } else {
        duration.as_secs().to_string()
    };
    if duration.subsec_nanos() > 0 {
        if duration.subsec_nanos() % 1_000_000 == 0 {
            result += &format!(".{:03}", duration.subsec_millis());
        } else if duration.subsec_nanos() % 1_000 == 0 {
            result += &format!(".{:06}", duration.subsec_micros());
        } else {
            result += &format!(".{:09}", duration.subsec_nanos());
        }
    }
    result + "s"
}
