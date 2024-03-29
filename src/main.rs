use std::{
    sync::{Arc, Mutex},
    thread::spawn,
};

use clap::Parser;
use thiserror::Error;
use zbus::{blocking::Connection, dbus_proxy};

slint::include_modules!();

#[derive(Error, Debug)]
pub enum AppletError {
    #[error("slint::PlatformError")]
    SlintPlatform(#[from] slint::PlatformError),

    #[error("zbus::Error")]
    Zbus(#[from] zbus::Error),

    #[error("tokio::task::JoinError")]
    TokioTaskJoin(#[from] tokio::task::JoinError),

    #[error("unknown AppletError")]
    Unknown,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Hides switch that controls Invert
    #[arg(short = 'i', long, default_value_t = false)]
    hide_invert: bool,
    /// Hides slider that controls Temperature
    #[arg(short = 't', long, default_value_t = false)]
    hide_temperature: bool,
    /// Hides slider that controls Brightness
    #[arg(short = 'b', long, default_value_t = false)]
    hide_brightness: bool,
    /// Hides slider that controls Gamma
    #[arg(short = 'g', long, default_value_t = false)]
    hide_gamma: bool,
    /// Hides "speech bubble" caret at bottom of applet
    #[arg(short = 'c', long, default_value_t = false)]
    hide_caret: bool,
    /// Hides text labels of control widgets
    #[arg(short = 'l', long, default_value_t = false)]
    hide_labels: bool,
    /// Hides text value of active control widgets
    #[arg(short = 'v', long, default_value_t = false)]
    hide_value: bool,
    /// Set this flag to never automatically fade the window.
    #[arg(short = 'f', long, default_value_t = false)]
    never_fade: bool,
    /// Set applet window outer padding
    #[arg(short = 'p', long, default_value_t = 8)]
    outer_padding: usize,
    /// Set applet window width (horizontal)
    #[arg(short = 'x', long, default_value_t = 100)]
    window_width: usize,
    /// Set applet window height (vertical)
    #[arg(short = 'y', long, default_value_t = 220)]
    window_height: usize,
    /// 'Reset' value for temperature. (1000 - 10000)
    #[arg(short = 'T', long, default_value_t = 6500)]
    default_temperature: i16,
    /// 'Reset' value for brightness. (0.0 - 1.0)
    #[arg(short = 'B', long, default_value_t = 1.0)]
    default_brightness: f64,
    /// 'Reset' value for gamma. ( 0.5 - 1.5)
    #[arg(short = 'G', long, default_value_t = 1.0)]
    default_gamma: f64,
}

// # DBus interface proxy for: `rs.wl.gammarelay`
// Source: `Interface '/' from service 'rs.wl-gammarelay' on session bus`.
// This block was (mostly) generated by `zbus-xmlgen` `3.1.1` from DBus introspection data.
#[dbus_proxy(
    default_service = "rs.wl-gammarelay",
    interface = "rs.wl.gammarelay",
    default_path = "/"
)]
trait GammaRelay {
    /// ToggleInverted method
    fn toggle_inverted(&self) -> zbus::Result<()>;
    /// UpdateBrightness method
    fn update_brightness(&self, delta_brightness: f64) -> zbus::Result<()>;
    /// UpdateGamma method
    fn update_gamma(&self, delta_gamma: f64) -> zbus::Result<()>;
    /// UpdateTemperature method
    fn update_temperature(&self, delta_temp: i16) -> zbus::Result<()>;
    /// Brightness property
    #[dbus_proxy(property)]
    fn brightness(&self) -> zbus::Result<f64>;
    /// Gamma property
    #[dbus_proxy(property)]
    fn gamma(&self) -> zbus::Result<f64>;
    /// Inverted property
    #[dbus_proxy(property)]
    fn inverted(&self) -> zbus::Result<bool>;
    /// Temperature property
    #[dbus_proxy(property)]
    fn temperature(&self) -> zbus::Result<u16>;
}

// remapping values from low1-high1 to low2-high2 is done like
// low2 + (value - low1) * (high2 - low2) / (high1 - low1)

// 1000 - 10000 to
//  0.0 - 1.0
fn dbus_temperature_to_ui_value(dbus_value: u16) -> f64 {
    (dbus_value as f64 - 1000.0) * (1.0 / 9000.0)
}
fn dbus_temperature_delta_to_ui_value(dbus_value: i16) -> f64 {
    (dbus_value as f64) * (1.0 / 9000.0)
}
fn ui_temperature_delta_to_dbus_value(ui_value: f64) -> i16 {
    (ui_value * 9000.0) as i16
}
fn dbus_temperature_to_string(dbus_value: i16) -> String {
    format!("{dbus_value} K")
}
fn dbus_temperature_rounded(dbus_value: i16) -> i16 {
    ((dbus_value as f64 / 100.0) as i16) * 100
}

fn dbus_brightness_to_ui_value(dbus_value: f64) -> f64 {
    dbus_value
}
fn ui_brightness_delta_to_dbus_value(ui_value: f64) -> f64 {
    ui_value
}
fn dbus_brightness_to_string(dbus_value: f64) -> String {
    let percentage = dbus_value * 100.0;
    format!("{percentage:3.0} %")
}
fn dbus_brightness_rounded(dbus_value: f64) -> f64 {
    (dbus_value * 100.0).round() / 100.0
}

// 0.5 - 1.5 to
// 0.0 - 1.0
fn dbus_gamma_to_ui_value(dbus_value: f64) -> f64 {
    dbus_value - 0.5
}
fn dbus_gamma_to_string(dbus_value: f64) -> String {
    format!("{dbus_value:.2} γ")
}
fn dbus_gamma_rounded(dbus_value: f64) -> f64 {
    (dbus_value * 100.0).round() / 100.0
}

fn create_proxy() -> Result<GammaRelayProxyBlocking<'static>, AppletError> {
    let connection = spawn(|| Connection::session().expect("rust: create zbus connection"))
        .join()
        .expect("rust: create zbus connection");
    let arc_connection = Arc::new(Mutex::new(connection));
    let create_proxy = spawn(move || {
        let locked_connection = arc_connection.lock().unwrap();
        GammaRelayProxyBlocking::new(&locked_connection).expect("rust: create zbus connection")
    });
    Ok(create_proxy.join().expect("rust: export proxy"))
}

#[derive(Default, Clone, Copy)]
struct SettingState {
    value: f64,
    delta_accumulation: f64,
    default: f64,
}

struct Settings {
    invert: SettingState,
    temperature: SettingState,
    brightness: SettingState,
    gamma: SettingState,
}

impl Settings {
    fn invalidate_deltas(&mut self) {
        self.invert.delta_accumulation = 0.0;
        self.temperature.delta_accumulation = 0.0;
        self.brightness.delta_accumulation = 0.0;
        self.gamma.delta_accumulation = 0.0;
    }

    fn set_invert(&mut self, v: bool) {
        self.invert.delta_accumulation += 1.0;
        self.invert.value = if v { 1.0 } else { 0.0 };
    }

    fn set_temperature(&mut self, v: f64) {
        self.temperature.delta_accumulation += v - self.temperature.value;
        self.temperature.value = v;
    }

    fn set_brightness(&mut self, v: f64) {
        self.brightness.delta_accumulation += v - self.brightness.value;
        self.brightness.value = v;
    }

    fn set_gamma(&mut self, v: f64) {
        self.gamma.delta_accumulation += v - self.gamma.value;
        self.gamma.value = v;
    }
}

const TICK_DELTA: u64 = 7;

fn main() -> Result<(), AppletError> {
    let args = Args::parse();
    let app = WlGammaRelayApplet::new()?;
    let proxy = Arc::<Mutex<GammaRelayProxyBlocking<'_>>>::new(Mutex::new(
        create_proxy().expect("rust: create proxy"),
    ));

    // initialize window state and ui values
    let settings = {
        // initialize startup ui parameters based on arguments
        let default_temperature = args.default_temperature as f64;
        let default_brightness = args.default_brightness;
        let default_gamma = args.default_gamma;

        app.global::<Startup>().set_show_invert(!(args.hide_invert));
        app.global::<Startup>()
            .set_show_temperature(!(args.hide_temperature));
        app.global::<Startup>()
            .set_show_brightness(!(args.hide_brightness));
        app.global::<Startup>().set_show_gamma(!(args.hide_gamma));
        app.global::<Startup>().set_show_caret(!args.hide_caret);
        app.global::<Startup>().set_show_labels(!args.hide_labels);
        app.global::<Startup>().set_show_value(!args.hide_value);
        app.global::<Startup>().set_never_fade(args.never_fade);
        app.global::<Startup>()
            .set_outer_padding(args.outer_padding as i32);
        app.global::<Startup>()
            .set_window_height(args.window_height as i32);
        app.global::<Startup>()
            .set_window_width(args.window_width as i32);
        app.global::<Startup>()
            .set_default_temperature(
                dbus_temperature_to_ui_value(default_temperature as u16) as f32
            );
        app.global::<Startup>()
            .set_default_brightness(dbus_brightness_to_ui_value(default_brightness) as f32);
        app.global::<Startup>()
            .set_default_gamma(dbus_gamma_to_ui_value(default_gamma) as f32);

        let proxy_ref = proxy.clone();
        let get_dbus_state = spawn(move || {
            let proxy = proxy_ref.lock().expect("rust: unlock proxy");
            let startup_dbus_invert = proxy.inverted().expect("rust: get inverted from dbus");
            let startup_dbus_temperature = proxy
                .temperature()
                .expect("rust: get temperature from dbus");
            let startup_dbus_brightness =
                proxy.brightness().expect("rust: get brightness from dbus");
            let startup_dbus_gamma = proxy.gamma().expect("rust: get gamma from dbus");

            (
                if startup_dbus_invert { 1.0 } else { 0.0 },
                dbus_temperature_to_ui_value(startup_dbus_temperature),
                dbus_brightness_to_ui_value(startup_dbus_brightness),
                dbus_gamma_to_ui_value(startup_dbus_gamma),
                startup_dbus_temperature,
                startup_dbus_brightness,
                startup_dbus_gamma,
            )
        });

        let (
            startup_inverted,
            startup_temperature,
            startup_brightness,
            startup_gamma,
            startup_dbus_temperature,
            startup_dbus_brightness,
            startup_dbus_gamma,
        ) = get_dbus_state.join().expect("rust: get dbus state");

        if !args.hide_temperature {
            app.global::<Parameters>()
                .set_value_text(dbus_temperature_to_string(startup_dbus_temperature as i16).into());
        } else if !args.hide_brightness {
            app.global::<Parameters>()
                .set_value_text(dbus_brightness_to_string(startup_dbus_brightness).into());
        } else if !args.hide_gamma {
            app.global::<Parameters>()
                .set_value_text(dbus_gamma_to_string(startup_dbus_gamma).into());
        } else {
            app.global::<Startup>().set_show_value(false);
        }

        // initialize parameter ui values based on current gammarelay state
        app.global::<Parameters>()
            .set_invert(startup_inverted > 0.0);
        app.global::<Parameters>()
            .set_temperature(startup_temperature as f32);
        app.global::<Parameters>()
            .set_brightness(startup_brightness as f32);
        app.global::<Parameters>().set_gamma(startup_gamma as f32);

        Arc::<Mutex<Settings>>::new(Mutex::new(Settings {
            invert: SettingState {
                value: startup_inverted,
                delta_accumulation: 0.0,
                default: 0.0,
            },
            temperature: SettingState {
                value: startup_temperature,
                delta_accumulation: 0.0,
                default: default_temperature,
            },
            brightness: SettingState {
                value: startup_brightness,
                delta_accumulation: 0.0,
                default: default_brightness,
            },
            gamma: SettingState {
                value: startup_gamma,
                delta_accumulation: 0.0,
                default: default_gamma,
            },
        }))
    };

    // create tick binding which runs opacity management (slint-side)
    // and also hides (which destroys) the window when done fading out.
    {
        let app_weak = app.as_weak();
        app.on_tick(move |delta| {
            let binding = app_weak.unwrap();
            binding.invoke_manage_opacity(delta);
            let startup_fade_in = binding.global::<Startup>().get_fade_in();
            let current_opacity = binding.global::<Parameters>().get_window_opacity();
            if !startup_fade_in && current_opacity < 0.0 {
                let _ = binding.window().hide();
            }
        });
    }

    // on invert toggle widget changed, set the settings...
    {
        let settings_ref = settings.clone();
        app.global::<Parameters>().on_invert_changed(move |value| {
            let mut settings = settings_ref.lock().expect("rust: unlock settings");
            settings.set_invert(value);
        });
    }

    // on slider widget set to default...
    {
        let app_weak = app.as_weak();
        let proxy_ref = proxy.clone();
        let settings_ref = settings.clone();
        app.global::<Parameters>().on_slider_default(move |name| {
            // compare server value to default value and apply the lossless delta.
            // also set the settings value and invalidate deltas.
            let mut settings = settings_ref.lock().expect("rust: unlock settings");
            let app = app_weak.unwrap();
            match &*name {
                "temperature" => {
                    let proxy = proxy_ref.lock().expect("rust: unlock proxy");
                    let server_value =
                        proxy.temperature().expect("rust: get server temperature") as i16;
                    let hard_delta = settings.temperature.default as i16 - server_value;
                    proxy
                        .update_temperature(hard_delta as i16)
                        .expect("rust: expect set temperature");
                    settings.set_temperature(dbus_temperature_delta_to_ui_value(
                        server_value + hard_delta,
                    ));
                    app.global::<Parameters>().set_value_text(
                        dbus_temperature_to_string(settings.temperature.default as i16).into(),
                    );
                    settings.invalidate_deltas();
                }
                "brightness" => {
                    let proxy = proxy_ref.lock().expect("rust: unlock proxy");
                    let server_value = proxy.brightness().expect("rust: get server brightness");
                    let hard_delta = settings.brightness.default - server_value;
                    proxy
                        .update_brightness(hard_delta)
                        .expect("rust: expect set brightness");
                    settings.set_brightness(dbus_brightness_to_ui_value(server_value + hard_delta));
                    app.global::<Parameters>().set_value_text(
                        dbus_brightness_to_string(settings.brightness.default).into(),
                    );
                    settings.invalidate_deltas();
                }
                "gamma" => {
                    let proxy = proxy_ref.lock().expect("rust: unlock proxy");
                    let server_value = proxy.gamma().expect("rust: get server gamma");
                    let hard_delta = settings.gamma.default - server_value;
                    proxy
                        .update_gamma(hard_delta)
                        .expect("rust: expect set gamma");
                    settings.set_gamma(dbus_gamma_to_ui_value(server_value + hard_delta));
                    app.global::<Parameters>()
                        .set_value_text(dbus_gamma_to_string(settings.gamma.default).into());
                    settings.invalidate_deltas();
                }
                _ => {}
            }
        });
    }

    // on slider widget changed, set the settings...
    {
        let settings_ref = settings.clone();
        app.global::<Parameters>()
            .on_slider_changed(move |name, value| {
                let mut settings = settings_ref.lock().expect("rust: unlock settings");
                match &*name {
                    "temperature" => {
                        settings.set_temperature(value as f64);
                    }
                    "brightness" => {
                        settings.set_brightness(value as f64);
                    }
                    "gamma" => {
                        settings.set_gamma(value as f64);
                    }
                    _ => {}
                }
            });
    }

    // create a timer that invokes tick on the main window
    // and applies deltas in settings to the dbus server.
    let timer = slint::Timer::default();
    {
        let app_weak = app.as_weak();
        let proxy_ref = proxy.clone();
        let settings_ref = settings.clone();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(TICK_DELTA),
            move || {
                let app = app_weak.unwrap();
                app.invoke_tick(TICK_DELTA as f32);

                let proxy = proxy_ref.lock().expect("rust: unlock proxy");
                let mut settings = settings_ref.lock().expect("rust: unlock settings");

                if settings.invert.delta_accumulation != 0.0 {
                    proxy.toggle_inverted().expect("rust: expect set inverted");
                    settings.invalidate_deltas();
                }

                if settings.temperature.delta_accumulation != 0.0 {
                    let server_value =
                        proxy.temperature().expect("rust: get server temperature") as i16;
                    let dbus_delta =
                        ui_temperature_delta_to_dbus_value(settings.temperature.delta_accumulation);
                    let (final_value, clamped_delta) = {
                        let rounded_delta = dbus_temperature_rounded(dbus_delta);
                        let proposed_final_value = server_value + rounded_delta;
                        if proposed_final_value < 1000 {
                            (1000, 1000 - server_value)
                        } else if proposed_final_value > 10000 {
                            (10000, 10000 - server_value)
                        } else {
                            (proposed_final_value, rounded_delta)
                        }
                    };
                    if clamped_delta.abs() > 0 {
                        app.global::<Parameters>()
                            .set_value_text(dbus_temperature_to_string(final_value).into());
                        proxy
                            .update_temperature(clamped_delta)
                            .expect("rust: expect set temperature");
                        settings.invalidate_deltas();
                    }
                }

                if settings.brightness.delta_accumulation != 0.0 {
                    let server_value = proxy.brightness().expect("rust: get server brightness");
                    let rounded_delta = dbus_brightness_rounded(ui_brightness_delta_to_dbus_value(
                        settings.brightness.delta_accumulation,
                    ));
                    let final_value = server_value + rounded_delta;
                    if final_value > 0.2 && final_value < 1.0 {
                        app.global::<Parameters>()
                            .set_value_text(dbus_brightness_to_string(final_value).into());
                        proxy
                            .update_brightness(rounded_delta)
                            .expect("rust: expect set brightness");
                        settings.invalidate_deltas();
                    }
                }

                if settings.gamma.delta_accumulation != 0.0 {
                    let server_value = proxy.gamma().expect("rust: get server gamma");
                    let rounded_delta =
                        dbus_gamma_rounded(settings.gamma.delta_accumulation as f64);
                    let final_value = server_value + rounded_delta;
                    if final_value < 1.5 && final_value > 0.5 {
                        app.global::<Parameters>()
                            .set_value_text(dbus_gamma_to_string(final_value).into());
                        proxy
                            .update_gamma(rounded_delta)
                            .expect("rust: expect set gamma");
                        settings.invalidate_deltas();
                    }
                }
            },
        );
    }

    Ok(app.run()?)
}
