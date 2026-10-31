//! Local navigation measurements, excluded from ordinary builds.
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::navigation::{Direction, Folder};

pub struct Config {
    pub first_image: PathBuf,
    output: PathBuf,
    direction: Direction,
    rapid: bool,
    interval: Duration,
    settle: Duration,
    timeout: Duration,
    expected: usize,
    target: usize,
    full_events: bool,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

impl Config {
    pub fn from_args() -> io::Result<Option<Self>> {
        Self::parse(std::env::args_os().skip(1).collect())
    }

    fn parse(args: Vec<OsString>) -> io::Result<Option<Self>> {
        if args.first().is_none_or(|arg| arg != "--perf") {
            return Ok(None);
        }
        if args.len() != 10 {
            return Err(invalid(
                "Expected --perf FOLDER OUTPUT normal|rapid forward|backward INTERVAL_MS SETTLE_MS IMAGE_LIMIT TIMEOUT_SECONDS full|minimal",
            ));
        }
        let number = |index: usize, minimum: u64, maximum: u64| -> io::Result<u64> {
            args[index]
                .to_str()
                .and_then(|arg| arg.parse().ok())
                .filter(|value| (minimum..=maximum).contains(value))
                .ok_or_else(|| invalid("Invalid measurement duration or image limit."))
        };
        let direction = match args[4].to_str() {
            Some("forward") => Direction::Next,
            Some("backward") => Direction::Previous,
            _ => return Err(invalid("Invalid measurement direction.")),
        };
        let rapid = match args[3].to_str() {
            Some("normal") => false,
            Some("rapid") => true,
            _ => return Err(invalid("Invalid measurement scenario.")),
        };
        let full_events = match args[9].to_str() {
            Some("full") => true,
            Some("minimal") => false,
            _ => return Err(invalid("Invalid event logging mode.")),
        };
        let start = Folder::measurement_start(&PathBuf::from(&args[1]), direction)
            .map_err(|error| invalid(&error))?;
        let (index, total) = start.position();
        let expected = (number(7, 1, 1000)? as usize).min(total);
        let target = match direction {
            Direction::Next => index + expected - 1,
            Direction::Previous => index - expected + 1,
        };
        Ok(Some(Self {
            first_image: start.path().to_owned(),
            output: PathBuf::from(&args[2]),
            direction,
            rapid,
            expected,
            target,
            full_events,
            interval: Duration::from_millis(number(5, 10, 60_000)?),
            settle: Duration::from_millis(number(6, 0, 60_000)?),
            timeout: Duration::from_secs(number(8, 1, 3600)?),
        }))
    }
}

pub enum Action {
    Wait,
    Navigate(Direction),
    Close,
}

struct Event {
    time_ms: f64,
    kind: &'static str,
    id: usize,
    index: usize,
    total: usize,
    width: u32,
    height: u32,
    value_ms: f64,
    animated: bool,
    format: &'static str,
}

pub struct Session {
    config: Config,
    started: Instant,
    next: Instant,
    settling: Option<Instant>,
    loading: Option<Instant>,
    frame_pending: bool,
    active: bool,
    finished: bool,
    position: (usize, usize),
    dimensions: [u32; 2],
    animated: bool,
    format: &'static str,
    loads: usize,
    requests: usize,
    ignored: usize,
    navigation_ms: f64,
    started_unix_ms: u128,
    loader_total_ms: f64,
    frame_total_ms: f64,
    events: Vec<Event>,
}

impl Session {
    pub fn new(config: Config) -> io::Result<Self> {
        if !config.output.is_dir()
            || ["ready.json", "events.csv", "outcome.json"]
                .iter()
                .any(|file| config.output.join(file).exists())
        {
            return Err(invalid(
                "Measurement output must be a fresh, existing directory.",
            ));
        }
        let now = Instant::now();
        let started_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis();
        let mut ready = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(config.output.join("ready.json"))?;
        writeln!(
            ready,
            "{{\"started_unix_ms\":{started_unix_ms},\"expected_images\":{}}}",
            config.expected
        )?;
        Ok(Self {
            config,
            started: now,
            next: now,
            settling: None,
            loading: None,
            frame_pending: false,
            active: false,
            finished: false,
            position: (0, 0),
            dimensions: [0, 0],
            animated: false,
            format: "unknown",
            loads: 0,
            requests: 0,
            ignored: 0,
            navigation_ms: 0.0,
            started_unix_ms,
            loader_total_ms: 0.0,
            frame_total_ms: 0.0,
            events: Vec::new(),
        })
    }

    fn record(&mut self, kind: &'static str, value_ms: f64) {
        if !self.config.full_events
            && !matches!(
                kind,
                "settle_started"
                    | "completed"
                    | "incomplete"
                    | "timeout"
                    | "load_error"
                    | "aborted"
            )
        {
            return;
        }
        self.events.push(Event {
            time_ms: self.started.elapsed().as_secs_f64() * 1000.0,
            kind,
            value_ms,
            id: self.loads + usize::from(kind == "load_started"),
            index: self.position.0,
            total: self.position.1,
            width: self.dimensions[0],
            height: self.dimensions[1],
            animated: self.animated,
            format: self.format,
        });
    }

    pub fn load_started(&mut self) {
        self.loading = Some(Instant::now());
        self.record("load_started", 0.0);
    }

    pub fn ignored(&mut self) {
        self.ignored += 1;
        self.record("navigation_ignored", 0.0);
    }

    pub fn image_ready(
        &mut self,
        position: (usize, usize),
        dimensions: [u32; 2],
        animated: bool,
        loader_ms: f64,
        path: &Path,
    ) {
        self.loads += 1;
        self.position = position;
        self.dimensions = dimensions;
        self.animated = animated;
        self.format = match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => "jpeg",
            "png" => "png",
            "webp" => "webp",
            "gif" => "gif",
            "bmp" => "bmp",
            "tif" | "tiff" => "tiff",
            "heic" | "heif" => "heic",
            _ => "unknown",
        };
        self.loader_total_ms += loader_ms;
        self.frame_pending = true;
        self.record("image_ready", loader_ms);
    }

    pub fn tick(&mut self, pending: bool, error: bool) -> Action {
        if self.finished {
            return Action::Wait;
        }
        if error || self.started.elapsed() >= self.config.timeout {
            self.finish(if error { "load_error" } else { "timeout" });
            return Action::Close;
        }
        let now = Instant::now();
        if self.frame_pending {
            self.frame_pending = false;
            let frame_ms = self
                .loading
                .take()
                .map_or(0.0, |start| start.elapsed().as_secs_f64() * 1000.0);
            self.frame_total_ms += frame_ms;
            self.record("frame_queued", frame_ms);
            self.active = true;
            if !self.config.rapid || self.loads == 1 {
                self.next = now + self.config.interval;
            }
            let reached = match self.config.direction {
                Direction::Next => self.position.0 >= self.config.target,
                Direction::Previous => self.position.0 <= self.config.target,
            };
            if reached {
                self.navigation_ms = self.started.elapsed().as_secs_f64() * 1000.0;
                self.settling = Some(now);
                self.record("settle_started", 0.0);
            }
        }
        if let Some(start) = self.settling {
            if now.duration_since(start) >= self.config.settle {
                self.finish(if self.loads == self.config.expected {
                    "completed"
                } else {
                    "incomplete"
                });
                return Action::Close;
            }
        } else if self.active && now >= self.next && (self.config.rapid || !pending) {
            self.requests += 1;
            self.next = now + self.config.interval;
            self.record("navigation_request", 0.0);
            return Action::Navigate(self.config.direction);
        }
        Action::Wait
    }

    pub fn wake_after(&self, pending: bool) -> Duration {
        let timeout = self.started + self.config.timeout;
        let deadline = if let Some(start) = self.settling {
            (start + self.config.settle).min(timeout)
        } else if self.active && (self.config.rapid || !pending) {
            self.next.min(timeout)
        } else {
            timeout
        };
        deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1))
    }

    fn finish(&mut self, status: &'static str) {
        self.finished = true;
        self.record(status, 0.0);
        if let Err(error) = self.write_results(status) {
            eprintln!("Measurement output failed: {error}");
        }
    }

    fn write_results(&self, status: &str) -> io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.config.output.join("events.csv"))?;
        writeln!(
            file,
            "elapsed_ms,event,load_id,index,total,width,height,value_ms,animated,format"
        )?;
        for event in &self.events {
            writeln!(
                file,
                "{:.3},{},{},{},{},{},{},{:.3},{},{}",
                event.time_ms,
                event.kind,
                event.id,
                event.index,
                event.total,
                event.width,
                event.height,
                event.value_ms,
                event.animated,
                event.format
            )?;
        }
        let mut outcome = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.config.output.join("outcome.json"))?;
        writeln!(
            outcome,
            "{{\"status\":\"{status}\",\"images_loaded\":{},\"expected_images\":{},\"navigation_requests\":{},\"ignored_requests\":{},\"navigation_ms\":{:.3},\"elapsed_ms\":{:.3},\"started_unix_ms\":{},\"mean_loader_ms\":{:.3},\"mean_request_to_frame_queued_ms\":{:.3}}}",
            self.loads,
            self.config.expected,
            self.requests,
            self.ignored,
            self.navigation_ms,
            self.started.elapsed().as_secs_f64() * 1000.0,
            self.started_unix_ms,
            self.loader_total_ms / self.loads.max(1) as f64,
            self.frame_total_ms / self.loads.max(1) as f64
        )
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.finished {
            self.finish("aborted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(output: PathBuf) -> Config {
        Config {
            first_image: PathBuf::new(),
            output,
            direction: Direction::Next,
            rapid: false,
            interval: Duration::from_millis(10),
            settle: Duration::ZERO,
            timeout: Duration::from_secs(1),
            expected: 1,
            target: 1,
            full_events: true,
        }
    }

    #[test]
    fn requires_explicit_valid_arguments_and_never_treats_regular_open_as_measurement() {
        assert!(Config::parse(vec![]).unwrap().is_none());
        assert!(Config::parse(vec!["photo.png".into()]).unwrap().is_none());
        assert!(Config::parse(vec!["--perf".into()]).is_err());
    }

    #[test]
    fn navigation_waits_for_normal_loads_but_rapid_requests_continue() {
        for rapid in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let mut settings = config(directory.path().to_owned());
            settings.expected = 3;
            settings.target = 3;
            settings.rapid = rapid;
            let mut session = Session::new(settings).unwrap();
            session.load_started();
            session.image_ready((1, 3), [1, 1], false, 1.0, Path::new("test.png"));
            assert!(matches!(session.tick(false, false), Action::Wait));
            session.next = Instant::now() - Duration::from_secs(1);
            let action = session.tick(true, false);
            assert_eq!(matches!(action, Action::Navigate(Direction::Next)), rapid);
            if rapid {
                session.ignored();
                assert_eq!(session.ignored, 1);
            } else {
                assert!(matches!(session.tick(false, false), Action::Navigate(_)));
            }
        }
    }

    #[test]
    fn minimal_logs_preserve_totals_and_timeout_is_not_success() {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = config(directory.path().to_owned());
        settings.full_events = false;
        let mut session = Session::new(settings).unwrap();
        session.load_started();
        session.ignored();
        session.image_ready((1, 1), [1, 1], false, 3.0, Path::new("private.png"));
        assert!(matches!(session.tick(false, false), Action::Close));
        assert_eq!(session.loads, 1);
        assert_eq!(session.ignored, 1);
        assert_eq!(session.loader_total_ms, 3.0);
        let events = std::fs::read_to_string(directory.path().join("events.csv")).unwrap();
        assert!(!events.contains("image_ready"));
        assert!(events.contains("completed"));

        let other = tempfile::tempdir().unwrap();
        let mut timed = Session::new(config(other.path().to_owned())).unwrap();
        timed.started = Instant::now() - Duration::from_secs(2);
        assert!(matches!(timed.tick(false, false), Action::Close));
        let outcome = std::fs::read_to_string(other.path().join("outcome.json")).unwrap();
        assert!(outcome.contains("\"status\":\"timeout\""));
    }

    #[test]
    fn explicit_folder_starts_at_naturally_sorted_endpoints_without_recursion() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["photo10.png", "photo2.png", "photo1.png"] {
            std::fs::write(directory.path().join(name), []).unwrap();
        }
        let nested = directory.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("photo0.png"), []).unwrap();
        for (direction, name, target) in
            [("forward", "photo1.png", 2), ("backward", "photo10.png", 2)]
        {
            let args = [
                OsString::from("--perf"),
                directory.path().into(),
                directory.path().into(),
                "normal".into(),
                direction.into(),
                "10".into(),
                "0".into(),
                "2".into(),
                "10".into(),
                "full".into(),
            ];
            let settings = Config::parse(args.into()).unwrap().unwrap();
            assert_eq!(settings.first_image.file_name().unwrap(), name);
            assert_eq!(settings.expected, 2);
            assert_eq!(settings.target, target);
        }
        assert!(Folder::measurement_start(&nested.join("missing"), Direction::Next).is_err());
    }

    #[test]
    fn completed_run_reports_actual_frame_boundary_and_rejects_output_reuse() {
        let directory = tempfile::tempdir().unwrap();
        let mut session = Session::new(config(directory.path().to_owned())).unwrap();
        session.load_started();
        session.image_ready((1, 1), [640, 480], false, 12.5, Path::new("photo.png"));
        assert!(matches!(session.tick(false, false), Action::Close));
        let events = std::fs::read_to_string(directory.path().join("events.csv")).unwrap();
        assert!(events.contains("image_ready"));
        assert!(events.contains("frame_queued"));
        assert!(!events.contains("photo.png"));
        assert!(Session::new(config(directory.path().to_owned())).is_err());
    }

    #[test]
    fn errors_and_skipped_images_do_not_produce_successful_runs() {
        for error in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let mut settings = config(directory.path().to_owned());
            settings.expected = 2;
            let mut session = Session::new(settings).unwrap();
            session.image_ready((1, 2), [1, 1], false, 0.0, Path::new("photo.png"));
            assert!(matches!(session.tick(false, error), Action::Close));
            let result = std::fs::read_to_string(directory.path().join("outcome.json")).unwrap();
            assert!(!result.contains("\"status\":\"completed\""));
        }
    }
}
