use std::time::{Duration, Instant};

enum State {
    IdleSince(Instant),
    WorkingSince(Instant),
}

struct Break {
    prompt: String,
    after: Duration,
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
    fn check(&mut self, worktime: Duration) -> anyhow::Result<bool> {
        if worktime > self.after + self.last_done {
            if request_user(&self.prompt)? {
                self.last_done = worktime;
            }
            return Ok(true);
        }
        Ok(false)
    }
}

fn main() -> Result<(), anyhow::Error> {
    use State::*;
    let mut screen_time = Duration::from_secs(0);
    let mut state = WorkingSince(Instant::now());

    let cutoff = Duration::from_secs(60);
    let workday = Duration::from_secs(60 * 60 * 8);

    let night_time = Duration::from_secs(60 * 60 * 7);

    let just_started = Duration::from_secs(60 * 5);
    let good_chunk_of_work = Duration::from_secs(60 * 30);

    let prompt_gap = Duration::from_secs(60 * 5);
    let mut last_prompt = Instant::now();

    let mut breaks = vec![
        Break::new(
            "Time for a 7-minute exersize",
            Duration::from_secs(60 * 60 * 3),
        ),
        Break::new("Switch to standing desk", Duration::from_secs(60 * 60 * 4)),
    ];
    loop {
        let t = idle_time()?;
        let now = Instant::now();
        match state {
            WorkingSince(start) => {
                if t > cutoff {
                    let start_idle = now - t;
                    screen_time += start_idle.duration_since(start);
                    state = IdleSince(start_idle);
                    println!("\nYou are now AFK!");
                } else {
                    let this_work = now.duration_since(start);
                    if this_work + screen_time > workday {
                        notify(&format!("emd ogh day"))?;
                    } else if this_work < just_started || this_work > good_chunk_of_work {
                        for b in breaks.iter_mut() {
                            if now.duration_since(last_prompt) > prompt_gap {
                                if b.check(this_work + screen_time)? {
                                    last_prompt = now;
                                }
                            }
                        }
                    }
                    print!(
                        "\rYou have been working {}    ",
                        (this_work + screen_time).pretty()
                    );
                }
            }
            IdleSince(start) => {
                let start_idle = now - t;
                if start_idle.duration_since(start) > cutoff {
                    state = WorkingSince(start_idle);
                    println!("\nYou just started working again.");
                } else if t > night_time && screen_time > Duration::from_secs(0) {
                    println!("\nI think it is a new day.  Resetting.");
                    screen_time = Duration::from_secs(0);
                } else {
                    print!("\rYou have been idle for {}      ", t.pretty());
                }
            }
        }
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn request_user(msg: &str) -> anyhow::Result<bool> {
    println!("asking: {}", msg);
    Ok(native_dialog::MessageDialog::new()
        .set_title(msg)
        .set_text("Did you?")
        .set_type(native_dialog::MessageType::Warning)
        .show_confirm()?)
}

fn notify(msg: &str) -> anyhow::Result<()> {
    println!("\n{}", msg);
    msgbox::create("End of day", msg, msgbox::IconType::None)?;
    Ok(())
}

fn idle_time() -> anyhow::Result<Duration> {
    let idle = user_idle::UserIdle::get_time().map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(idle.duration())
}

trait Pretty {
    fn pretty(&self) -> String;
}
impl Pretty for Duration {
    fn pretty(&self) -> String {
        let secs = self.as_secs();
        let total_minutes = (secs + 30) / 60;
        let hours = total_minutes / 60;
        let minutes = total_minutes - hours * 60;
        format!("{:2}:{:02}", hours, minutes)
    }
}
