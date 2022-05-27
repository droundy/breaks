use std::io::Write;
use std::{
    process::Command,
    time::{Duration, Instant},
};

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

    // let main_window = WindowDesc::new(ui_builder())
    //     .title(LocalizedString::new("open-save-demo").with_placeholder("Opening/Saving Demo"));
    // let data = "Type here.".to_owned();
    // AppLauncher::with_window(main_window)
    //     .delegate(Delegate)
    //     .log_to_console()
    //     .launch(data)
    //     .expect("launch failed");

    use State::*;
    let mut screen_time = Duration::from_secs(0);
    let mut state = WorkingSince(Instant::now());

    let cutoff = Duration::from_secs(60 * 10);
    let workday = Duration::from_secs(60 * 60 * 8);

    let night_time = Duration::from_secs(60 * 60 * 7);

    let just_started = Duration::from_secs(60 * 5);
    let good_chunk_of_work = Duration::from_secs(60 * 30);

    let prompt_gap = Duration::from_secs(60 * 5);
    let mut last_prompt = Instant::now();

    notify("Good morning!")?;

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
                if t > cutoff && !am_in_meet() {
                    let start_idle = now - t;
                    screen_time += start_idle.duration_since(start);
                    state = IdleSince(start_idle);
                    print!("\rYou have been working {}", screen_time.pretty());
                    println!("\nYou are now AFK!");
                } else {
                    let this_work = now.duration_since(start);
                    if this_work + screen_time > workday && last_prompt.elapsed() > just_started {
                        notify(&format!(
                            "End of day after {}",
                            (this_work + screen_time).pretty()
                        ))?;
                        last_prompt = now;
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
                        "\rYou have been working {} (and are {:7} in meet)",
                        (this_work + screen_time).pretty(),
                        am_in_meet()
                    );
                    std::io::stdout().flush()?;
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
                    print!("\rYou have been idle for {}", t.pretty());
                    std::io::stdout().flush()?;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

fn request_user(msg: &str) -> anyhow::Result<bool> {
    println!("\nasking: {}", msg);
    // msgbox::create("Break time", msg, msgbox::IconType::Info)?;
    Ok(true)
    // Ok(native_dialog::MessageDialog::new()
    //     .set_title(msg)
    //     .set_text("Did you?")
    //     .set_type(native_dialog::MessageType::Warning)
    //     .show_confirm()?)
}

fn notify(msg: &str) -> anyhow::Result<()> {
    println!("\n{}", msg);
    // msgbox::create("Break time", msg, msgbox::IconType::Info)?;
    // native_dialog::MessageDialog::new()
    //     .set_title(msg)
    //     .set_text("Spend time with your famil!y")
    //     .set_type(native_dialog::MessageType::Warning)
    //     .show_alert()?;
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



use druid::widget::{Align, Button, Flex, TextBox};
use druid::{
    commands, AppDelegate, AppLauncher, DelegateCtx, Env, FileDialogOptions, FileSpec,
    Handled, LocalizedString, Target, Widget, WindowDesc,
};

struct Delegate;

fn ui_builder() -> impl Widget<String> {
    let rs = FileSpec::new("Rust source", &["rs"]);
    let txt = FileSpec::new("Text file", &["txt"]);
    let other = FileSpec::new("Bogus file", &["foo", "bar", "baz"]);
    // The options can also be generated at runtime,
    // so to show that off we create a String for the default save name.
    let default_save_name = String::from("MyFile.txt");
    let save_dialog_options = FileDialogOptions::new()
        .allowed_types(vec![rs, txt, other])
        .default_type(txt)
        .default_name(default_save_name)
        .name_label("Target")
        .title("Choose a target for this lovely file")
        .button_text("Export");

    let input = druid::widget::Label::new(move|_s: &String,_env: &Env| "foo".to_string());
    let save = Button::new("Acknow").on_click(move |ctx, _, _| {
        ctx.submit_command(druid::commands::SHOW_SAVE_PANEL.with(save_dialog_options.clone()))
    });
    let open = Button::new("Open").on_click(move |ctx, _, _| {
        println!("opne");
    });

    let mut col = Flex::column();
    col.add_child(input);
    col.add_spacer(8.0);
    col.add_child(save);
    col.add_child(open);
    Align::centered(col)
}

impl AppDelegate<String> for Delegate {
    fn command(
        &mut self,
        _ctx: &mut DelegateCtx,
        _target: Target,
        cmd: &druid::Command,
        data: &mut String,
        _env: &Env,
    ) -> Handled {
        if let Some(file_info) = cmd.get(commands::SAVE_FILE_AS) {
            if let Err(e) = std::fs::write(file_info.path(), &data[..]) {
                println!("Error writing file: {}", e);
            }
            return Handled::Yes;
        }
        if let Some(file_info) = cmd.get(commands::OPEN_FILE) {
            match std::fs::read_to_string(file_info.path()) {
                Ok(s) => {
                    let first_line = s.lines().next().unwrap_or("");
                    *data = first_line.to_owned();
                }
                Err(e) => {
                    println!("Error opening file: {}", e);
                }
            }
            return Handled::Yes;
        }
        Handled::No
    }
}

