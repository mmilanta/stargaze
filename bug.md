# Bug: Per-frame window title changes flood the Wayland compositor and can fill `$XDG_RUNTIME_DIR`

**Status:** Open
**Severity:** High (can render the whole desktop session unable to launch apps)
**Component:** `src/main.rs` (event loop / window title)
**Affects:** Progressive rendering mode (the default; `ControlFlow::Poll`, redraw every frame)

## Summary

While stargaze is converging an image, `spp` advances every frame. Because the
sample count is embedded in the window title, the title changes on essentially
every rendered frame. The event loop runs in `ControlFlow::Poll` and calls
`request_redraw()` from `about_to_wait()`, so this happens at display refresh
rate (typically 60+ times per second) for as long as the app runs.

A Wayland title change is a `windowtitlev2` event (the `zwp_...` stable
`wlr-foreign-toplevel` protocol / xdg toplevel title). Compositors and shells
that log or persist these events — notably **quickshell**, used by the Omarchy
desktop — receive a continuous high-rate stream. In the reported case the
quickshell log grew to **3.1 GB**, filling the user's entire
`/run/user/1000` tmpfs (3.1 GB). Once that filesystem was full, the `uwsm`
app daemon failed with `[Errno 28] No space left on device`, set its
`app_daemon_error` flag, and every subsequent application launch failed with:

```
App failure Invalid arguments:
```

i.e. **stargaze's title churn is able to break launching unrelated apps
(browser, terminal, …) for the rest of the session.**

## Environment

- stargaze `0.1.0` (`Cargo.toml`)
- `winit 0.30.13`, `wgpu 30.0.1`
- Wayland session managed by `uwsm` 0.26.7 on Hyprland (Omarchy 4.0.4)
- Shell: `quickshell`

## Steps to reproduce

1. Run stargaze on a Wayland session with a compositor/shell that logs
   `windowtitlev2` events (e.g. quickshell):
   ```sh
   cargo run --release
   ```
2. Let the progressive renderer converge (default behavior; time can be
   paused — the `spp` counter still advances).
3. Observe the shell's log growth, e.g.:
   ```sh
   watch -n1 'ls -l /run/user/1000/quickshell/by-id/*/log.qslog'
   ```
4. Optionally observe the title updates directly:
   ```sh
   # while stargaze runs
   hyprctl -j clients | jq -r '.[] | select(.title|startswith("stargaze")) | .title'
   ```

## Expected

Window title updates should be limited to changes a human can perceive
(at most a few times per second), or the volatile counters should be left out
of the title. A rendering app should never be able to fill
`$XDG_RUNTIME_DIR` or break session-level app launching.

## Actual

The title is rewritten every frame with an ever-increasing `spp` value, e.g.:

```
stargaze  ·  t = 0.95 d (0.003 yr)  ·  paused  ·  3307460 spp  ·  auto exp +0.00 EV  ·  ...
stargaze  ·  t = 0.95 d (0.003 yr)  ·  paused  ·  3307461 spp  ·  auto exp +0.00 EV  ·  ...
stargaze  ·  t = 0.95 d (0.003 yr)  ·  paused  ·  3307462 spp  ·  auto exp +0.00 EV  ·  ...
```

The title guard compares the whole formatted string, so it always differs
once the sample counter moves.

## Root cause

In `src/main.rs`, the title is rebuilt and set inside `WindowEvent::RedrawRequested`
(lines ~739–748):

```rust
let title = format!(
    "stargaze  ·  t = {:.2} d ({:.3} yr)  ·  {}  ·  {} spp  ·  {exposure}{lock}  ·  click body=lock  ... ",
    self.state.sim_time,
    self.state.sim_time / 365.256,
    rate,
    r.samples(),
);
if title != self.last_title {
    window.set_title(&title);
    self.last_title = title;
}
```

`about_to_wait()` unconditionally requests another redraw, and the loop is set
to `ControlFlow::Poll` (lines ~902–916):

```rust
fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    if let Some(window) = self.window.as_ref() {
        window.request_redraw();
    }
}
...
event_loop.set_control_flow(ControlFlow::Poll);
```

`r.samples()` increases during progressive accumulation
(`src/pathtracer.rs`, `history.samples += count`), so the title string changes
on every frame and the `title != last_title` guard never suppresses anything.

## Impact

- Continuous `windowtitlev2` traffic to the compositor and any shell that
  observes/logs it.
- In at least one real session: a 3.1 GB quickshell log filled
  `/run/user/1000` (tmpfs), which broke the `uwsm` app daemon and made all
  application launches fail with `App failure Invalid arguments:`.
- Log/disk churn is unbounded and proportional to runtime.

## Suggested fix

Throttle title updates to a low, human-visible rate and/or drop high-frequency
counters from the title. For example, add a timestamp to `App` and only update
the title at most ~4×/second:

```rust
// in App
last_title: String,
last_title_update: std::time::Instant,
```

```rust
// in WindowEvent::RedrawRequested, replacing the existing guard
let now = std::time::Instant::now();
if title != self.last_title
    && now.duration_since(self.last_title_update) >= std::time::Duration::from_millis(250)
{
    window.set_title(&title);
    self.last_title = title;
    self.last_title_update = now;
}
```

Alternatively, format `spp` as a rounded/abbreviated value (e.g. `3.3M spp`)
so it changes far less often, or remove `spp` from the title entirely and rely
on the HUD. The throttle approach is preferred because `sim_time`/exposure can
also change continuously.

## Evidence

`/run/user/1000/quickshell/by-id/<id>/log.qslog` contained a tight loop of:

```
Received event: "windowtitlev2>>57f8af3ca580,stargaze  ·  t = 0.95 d (0.003 yr)  ·  paused  ·  3307460 spp  ·  auto exp +0.00 EV  ·  ..."
qs::wayland::toplevel::wlr::ToplevelHandle(0x...) got toplevel "stargaze  ·  ...  3307460 spp  ·  ..."
Received event: "windowtitlev2>>57f8af3ca580,stargaze  ·  ...  3307461 spp  ·  ..."
```

with the `spp` value incrementing by one on each successive line, reaching
3.1 GB.

Related failure once `/run/user/1000` was full:

```
uwsm_app-daemon[...]: [Errno 28] No space left on device
uwsm_app-daemon[...]: error flag /run/user/1000/uwsm/app_daemon_error exists
uwsm_app-daemon[...]: received:
uwsm_app-daemon[...]: sent: error 'Invalid arguments: ' 2
```

## Workaround (until fixed)

Truncate the log in place and clear the uwsm error flag, then restart the
app daemon:

```sh
: > /run/user/1000/quickshell/by-id/*/log.qslog
rm -f /run/user/1000/uwsm/app_daemon_error
systemctl --user restart wayland-wm-app-daemon.service
```

This restores app launching until stargaze (or any other title-churning app)
runs again. Throttling stargaze's title updates is the real fix.
