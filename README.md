# Breaks

This tool reminds you to take breaks, and tracks how long you have been using your
computer.  It should portable, but has only been tested on the mac.  Please file
a bug report if it doesn't work on your operating system.

`breaks` assumes that your computer is only used for work, and equates total screen
time with time spent working.  By default you are limited to an 8 hour work day, and
breaks will remind you to stop working after that much time.  The next work day starts
after your computer has been idle for a minimum amount of time.

`breaks` also supports reminders to do something periodically, such as exercise or
take a break.  These reminders are triggered after a certain amount of working time.

In both scenarios (end of day and break), a reminder is announced verbally, and later
is emphasized by repeated verbal reminders (and hiding of other apps on the Mac).  If
you persist long enough in ignoring the reminder, `breaks` will lock your screen.
At any point if you acknowledge the reminder by pressing the "done" button, `breaks`
will believe you and stop pestering you, so lying is absolutely possible.

`breaks` has some rudimentary logic to keep reminders from being too intrusive.  It
attempts to avoid reminders during a video meeting (very rudimentary, but works for me
on the Mac with Meet... file a bug report if it doesn't work for you!).  It also avoids
putting reminders in too close proximity.  Finally, it tries to ensure that when you
get back to work you either get the reminder very soon, or after you've had a good chunk
of time to focus.

## Configuring your breaks

Run `breaks` once with `cargo run` (or just `breaks` if it is in your path).  This will
create a file in your home directory called `~/.config/breaks.toml`, or (if that doesn't
work out) possibly a file called `breaks.toml` in the current directory.  The file should
be reasonably self-explanatory.  Times can be specified in hours and minutes in any of
the following formats:
- 1 hour
- 2 hours
- 2h
- 2:20 (this means two hours and 20 minutes)
- 1 minute
- 30 minutes
- 30m

Please file a bug report if you have a nice way to write a time that doesn't parse
correctly.