use anyhow::Context;
use druid::{Data, Lens, TimerToken};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

mod hours;
use hours::Pretty;

use std::io::Write;
use std::{
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
enum Status {
    IdleSince(Instant),
    WorkingSince(Instant),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Break {
    prompt: String,
    #[serde(with = "hours")]
    after: Duration,
    #[serde(skip)]
    last_done: Duration,
}

impl Break {
    fn new<S: Into<String>>(prompt: S, after: Duration) -> Self {
        Break {
            prompt: prompt.into(),
            after,
            last_done: Duration::from_secs(0),
        }
    }
    fn check(&mut self, worktime: Duration) -> bool {
        worktime > self.after + self.last_done
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(with = "hours")]
    max_idle_time_while_working: Duration,
    #[serde(with = "hours")]
    workday: Duration,
    #[serde(with = "hours")]
    day_resets_after: Duration,
    #[serde(with = "hours")]
    just_started: Duration,
    #[serde(with = "hours")]
    good_chunk_of_work: Duration,
    #[serde(with = "hours")]
    minimum_time_between_breaks: Duration,

    #[serde(with = "hours")]
    when_to_emphasize_break: Duration,
    #[serde(with = "hours")]
    when_to_lock_screen: Duration,
    breaks: Vec<Break>,
}

impl Default for Config {
    fn default() -> Self {
        let breaks = vec![
            Break::new(
                "Time for a 7-minute exersize",
                Duration::from_secs(60 * 60 * 3),
            ),
            Break::new(
                "Switch to standing desk",
                Duration::from_secs(60 * 60 * 4 + 60),
            ),
        ];
        Config {
            breaks,
            max_idle_time_while_working: Duration::from_secs(60 * 10),
            workday: Duration::from_secs(60 * 60 * 8),
            day_resets_after: Duration::from_secs(60 * 60 * 7),

            just_started: Duration::from_secs(60 * 6),
            good_chunk_of_work: Duration::from_secs(60 * 30),
            minimum_time_between_breaks: Duration::from_secs(60 * 5), // should be < just_started,

            when_to_emphasize_break: Duration::from_secs(60 * 2),
            when_to_lock_screen: Duration::from_secs(60 * 10),
        }
    }
}
impl Config {
    fn config_path() -> std::path::PathBuf {
        if let Some(h) = home::home_dir() {
            std::fs::create_dir_all(h.join(".config/")).ok();
            h.join(".config/breaks.toml")
        } else {
            "breaks.toml".into()
        }
    }
    fn load() -> anyhow::Result<Self> {
        if let Ok(contents) = std::fs::read_to_string(Self::config_path()) {
            // If file is readable but not parsable, we want to die with a nice error.
            toml::de::from_str(&contents)
                .context(format!("Unable to parse {:?}", Self::config_path()))
        } else {
            let c = Default::default();
            std::fs::write(
                Self::config_path(),
                toml::ser::to_string_pretty(&c).unwrap(),
            )
            .ok();
            Ok(c)
        }
    }
}

#[derive(Clone, Data, Lens)]
struct State {
    #[data(ignore)]
    tts: Option<Arc<Mutex<tts::Tts>>>,
    #[data(ignore)]
    config: Config,

    am_prompting: Option<String>,
    status_report: String,
    latest_update: String,

    #[data(ignore)]
    status: Status,
    #[data(ignore)]
    breaks: Vec<Break>,
    screen_time: Duration,

    last_prompt: Instant,
    am_emphasizing: bool,
}

impl Default for State {
    fn default() -> Self {
        State::new(Config::default())
    }
}

impl State {
    fn load() -> anyhow::Result<Self> {
        Ok(State::new(Config::load()?))
    }
}

impl State {
    fn new(config: Config) -> State {
        State {
            tts: tts::Tts::default()
                .ok()
                .map(|tts| Arc::new(Mutex::new(tts))),
            status: Status::WorkingSince(Instant::now()),
            screen_time: Duration::from_secs(0),
            last_prompt: Instant::now(),
            breaks: config.breaks.clone(),
            am_prompting: None,
            status_report: "".to_string(),
            latest_update: "".to_string(),
            am_emphasizing: false,
            config,
        }
    }
    fn say(&self, msg: &str) {
        self.tts
            .as_ref()
            .map(|tts| tts.lock().unwrap().speak(msg, false));
    }
    fn prompt(&mut self, msg: String) {
        self.say(msg.as_str());
        self.am_prompting = Some(msg);
    }
    fn announce(&self) {
        if let Some(p) = self.am_prompting.as_ref() {
            self.say(p.as_str());
        }
    }
    fn update(&mut self) -> anyhow::Result<()> {
        use Status::*;
        let config = &self.config;
        let t = idle_time()?;
        let now = Instant::now();
        match self.status {
            WorkingSince(start) => {
                if t > config.max_idle_time_while_working && !am_in_meet() {
                    let start_idle = now - t;
                    self.screen_time += start_idle.duration_since(start);
                    self.status = IdleSince(start_idle);
                    self.status_report = format!(
                        "After working {} you are now AFK!",
                        self.screen_time.pretty()
                    );
                } else {
                    let this_work = now.duration_since(start);
                    if this_work + self.screen_time > config.workday
                        && self.last_prompt.elapsed() > config.just_started
                    {
                        self.prompt(format!(
                            "End of day after {}",
                            (this_work + self.screen_time).pretty()
                        ));
                        self.last_prompt = now;
                    } else if (this_work < config.just_started
                        || this_work > config.good_chunk_of_work)
                        && self.am_prompting.is_none()
                        && !am_in_meet()
                    {
                        let mut prompt = None;
                        for b in self.breaks.iter_mut() {
                            if b.check(this_work + self.screen_time) {
                                let prompt_gap = now.duration_since(self.last_prompt);
                                if self.am_prompting.is_some() {
                                    self.status_report =
                                        format!("Postponing {}, see above.", b.prompt);
                                } else if am_in_meet() {
                                    self.status_report =
                                        format!("Postponing {} while you meet.", b.prompt);
                                } else if prompt_gap < self.config.minimum_time_between_breaks {
                                    self.status_report = format!(
                                        "Postponing {} for {}.",
                                        b.prompt,
                                        (config.minimum_time_between_breaks - prompt_gap).pretty()
                                    );
                                } else {
                                    prompt = Some(b.prompt.clone());
                                    self.last_prompt = Instant::now();
                                    b.last_done = this_work + self.screen_time;
                                }
                            }
                        }
                        if let Some(p) = prompt {
                            self.prompt(p);
                        }
                    }
                    self.latest_update = format!(
                        "You've been working for {}",
                        (this_work + self.screen_time).pretty()
                    );
                    std::io::stdout().flush()?;
                }
            }
            IdleSince(start) => {
                let start_idle = now - t;
                if start_idle.duration_since(start) > config.max_idle_time_while_working {
                    self.status = WorkingSince(start_idle);
                    self.status_report = format!(
                        "You resumed working after a {} break.",
                        start_idle.duration_since(start).pretty()
                    );
                } else if t > config.day_resets_after && self.screen_time > Duration::from_secs(0) {
                    self.status_report = format!("I think it is a new day.  Resetting.");
                    self.screen_time = Duration::from_secs(0);
                    for b in self.breaks.iter_mut() {
                        b.last_done = Duration::from_secs(0);
                    }
                } else {
                    self.latest_update = format!("You've been idle for {}", t.pretty());
                    std::io::stdout().flush()?;
                }
            }
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let state = State::load()?;

    let main_window = WindowDesc::new(ui_builder())
        .title(LocalizedString::new("open-save-demo").with_placeholder("Opening/Saving Demo"));
    AppLauncher::with_window(main_window)
        .delegate(Delegate)
        .log_to_console()
        .launch(state)
        .expect("launch failed");
    Ok(())
}

fn idle_time() -> anyhow::Result<Duration> {
    let idle = user_idle::UserIdle::get_time().map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(idle.duration())
}

fn am_in_meet() -> bool {
    if let Ok(output) = Command::new("pmset").arg("-g").output() {
        let mut output = &output.stdout[..];
        while !output.starts_with(b"Google Chrome") && !output.is_empty() {
            output = &output[1..];
        }
        output.starts_with(b"Google Chrome")
    } else {
        false
    }
}

use druid::widget::{Align, Button, Flex};
use druid::{AppDelegate, AppLauncher, Env, LocalizedString, Widget, WindowDesc};

struct Delegate;

fn ui_builder() -> impl Widget<State> {
    let prompt = druid::widget::Label::new(move |s: &State, _: &Env| {
        if let Some(p) = &s.am_prompting {
            p.clone()
        } else {
            "".to_string()
        }
    })
    .with_text_size(32.0);
    let status_report =
        druid::widget::Label::new(move |s: &State, _: &Env| s.status_report.clone())
            .with_text_size(24.0);
    let latest = druid::widget::Label::new(move |s: &State, _: &Env| s.latest_update.clone())
        .with_text_size(18.0);
    let done = Button::new("Done").on_click(move |ctx, state: &mut State, _| {
        state.am_emphasizing = false;
        if let Some(prompt) = std::mem::replace(&mut state.am_prompting, None) {
            state.status_report = format!("Well done with the {}!", prompt);
            ctx.submit_command(druid::commands::SHOW_ALL);
        }
    });

    let mut col = Flex::column();
    col.add_child(prompt);
    col.add_spacer(8.0);
    col.add_child(status_report);
    col.add_spacer(8.0);
    col.add_child(latest);
    col.add_spacer(8.0);
    col.add_child(done);
    col.add_child(TimerWidget {
        timer_id: TimerToken::INVALID,
    });
    Align::centered(col)
}

impl AppDelegate<State> for Delegate {}

struct TimerWidget {
    timer_id: TimerToken,
}
impl Widget<State> for TimerWidget {
    fn event(
        &mut self,
        ctx: &mut druid::EventCtx,
        event: &druid::Event,
        data: &mut State,
        _: &Env,
    ) {
        match event {
            druid::Event::WindowConnected => {
                // Start the timer when the application launches
                self.timer_id = ctx.request_timer(Duration::from_secs(10));
            }
            druid::Event::Timer(id) => {
                if *id == self.timer_id {
                    data.update().unwrap();
                    print!("\rupdate: {}", data.latest_update);
                    std::io::stdout().flush().ok();
                    ctx.request_layout();

                    if data.am_prompting.is_some() {
                        ctx.submit_command(druid::commands::SHOW_WINDOW);
                        if data.last_prompt.elapsed() > data.config.when_to_emphasize_break {
                            ctx.submit_command(druid::commands::HIDE_OTHERS);
                            data.last_prompt = Instant::now();
                        }
                        if data.am_emphasizing {
                            data.announce();
                        }
                    }
                    self.timer_id = ctx.request_timer(Duration::from_secs(10));
                }
            }
            _ => (),
        }
    }

    fn lifecycle(&mut self, _: &mut druid::LifeCycleCtx, _: &druid::LifeCycle, _: &State, _: &Env) {
    }

    fn update(&mut self, _: &mut druid::UpdateCtx, _: &State, _: &State, _: &Env) {}

    fn layout(
        &mut self,
        ctx: &mut druid::LayoutCtx,
        _: &druid::BoxConstraints,
        _: &State,
        _: &Env,
    ) -> druid::Size {
        if self.timer_id == TimerToken::INVALID {
            self.timer_id = ctx.request_timer(Duration::from_secs(10));
        }
        druid::Size::new(0.0, 0.0)
    }

    fn paint(&mut self, _: &mut druid::PaintCtx, _: &State, _: &Env) {}
}
