#![allow(clippy::single_component_path_imports)]
#![feature(backtrace)]

use std::env;
use std::ops::Range;
use std::sync::mpsc;

use anyhow::Result;
use boards::Board;
use boards::NUM_CONTROLLERS_PER_PAGE;
use embedded_svc::timer::OnceTimer;
use embedded_svc::timer::Timer;
use embedded_svc::timer::TimerService;
use esp_idf_svc::timer::EspTimer;
use esp_idf_svc::timer::EspTimerService;
use log::*;

mod button;
use button::ButtonId;
use pretty_env_logger::env_logger::WriteStyle;

mod button_controllers;

mod display;
use crate::button_controllers::DisplayState;
use crate::display::DisplayCommand;
use crate::messages::Message;

mod boards;
mod config;

mod input;
mod messages;
mod mqtt;
mod wifi;

const MQTT_URL: &str = env!("MQTT_URL");

#[cfg(esp32s2)]
include!(env!("EMBUILD_GENERATED_SYMBOLS_FILE"));

#[cfg(esp32s2)]
const ULP: &[u8] = include_bytes!(env!("EMBUILD_GENERATED_BIN_FILE"));

enum TimeOfDay {
    Day,
    Night,
}

struct RequestedDisplayStatus {
    time_of_day: TimeOfDay,
    forced_on: bool,
    night_timer: bool,
}

struct ActualDisplayStatus {
    timer_on: bool,
    display_on: bool,
}

impl RequestedDisplayStatus {
    fn get_timer_required(&self) -> bool {
        self.night_timer
    }

    fn get_display_required(&self) -> bool {
        matches!(self.time_of_day, TimeOfDay::Day) || self.forced_on || self.night_timer
    }

    fn turn_night_timer_on(&mut self) {
        self.night_timer = true;
    }

    fn turn_night_timer_off(&mut self) {
        self.night_timer = false;
    }
}

fn update_display(
    display: &mpsc::Sender<DisplayCommand>,
    id_in_page: usize,
    controller: &dyn button_controllers::Controller,
    state: button_controllers::DisplayState,
) {
    let icon = controller.get_icon();
    let name = controller.get_name();
    let message = DisplayCommand::DisplayState(state, icon, id_in_page, name);
    display.send(message).unwrap();
}

fn update_displays(
    display: &mpsc::Sender<DisplayCommand>,
    controllers: &[Box<dyn button_controllers::Controller>],
    page_num: usize,
) {
    let controllers = get_controllers_per_page(controllers, page_num);
    for (id_in_page, controller) in controllers.iter().enumerate() {
        if let Some(controller) = controller {
            let state = controller.get_display_state();
            update_display(display, id_in_page, *controller, state);
        } else {
            let message = DisplayCommand::DisplayNone(id_in_page);
            display.send(message).unwrap();
        };
    }
}

fn do_blank(
    display: &mpsc::Sender<DisplayCommand>,
    timer: &mut EspTimer,
    requested_display_status: &RequestedDisplayStatus,
    status: &mut ActualDisplayStatus,
    force_timer_reset: bool,
) {
    let timer_required = requested_display_status.get_timer_required();
    let display_required = requested_display_status.get_display_required();

    match (timer_required, status.timer_on) {
        (true, true) => {
            if force_timer_reset {
                info!("resetting blank timer");
                timer.cancel().unwrap();
                timer.after(std::time::Duration::new(10, 0)).unwrap();
                status.timer_on = true;
            }
        }
        (true, false) => {
            info!("starting blank timer");
            timer.cancel().unwrap();
            timer.after(std::time::Duration::new(10, 0)).unwrap();
            status.timer_on = true;
        }
        (false, true) => {
            info!("stopping blank timer");
            timer.cancel().unwrap();
            status.timer_on = false;
        }
        (false, false) => {}
    };

    match (display_required, status.display_on) {
        (true, false) => {
            info!("turning display on");
            status.display_on = true;
            display.send(DisplayCommand::UnBlankAll).unwrap();
        }
        (false, true) => {
            info!("turning display off");
            status.display_on = false;
            display.send(DisplayCommand::BlankAll).unwrap();
        }
        (true, true) => {}
        (false, false) => {}
    };
}

fn button_press(
    controllers: &mut [Box<dyn button_controllers::Controller>],
    id: usize,
    mqtt: &mqtt::Mqtt,
) {
    info!("Got button {} press", id);
    let controller_or_none = controllers.get_mut(id as usize);
    if let Some(controller) = controller_or_none {
        let commands = controller.get_press_commands();
        for command in commands {
            let topic = command.get_topic();
            let data = command.get_message();
            info!("Send {}: {}", topic, data);
            mqtt.publish(&topic, false, &data);
        }
    } else {
        error!("Controller for button {} does not exist", id);
    }
}

fn get_controller_range_for_page(page: usize) -> Range<usize> {
    let start = page * NUM_CONTROLLERS_PER_PAGE;
    let end = start + NUM_CONTROLLERS_PER_PAGE;
    Range { start, end }
}

fn get_controllers_per_page(
    controllers: &[Box<dyn button_controllers::Controller>],
    page: usize,
) -> Vec<Option<&dyn button_controllers::Controller>> {
    let mut range = get_controller_range_for_page(page);
    let len = controllers.len();

    if range.end > len {
        range.end = len
    }
    let controllers = &controllers[range];
    let len = controllers.len();

    let mut output: Vec<_> = controllers.iter().map(|x| Some(x.as_ref())).collect();
    output.extend((len..NUM_CONTROLLERS_PER_PAGE).map(|_| None));

    output
}

fn controller_to_page_id(controller_id: usize) -> (usize, usize) {
    let page_num = controller_id / NUM_CONTROLLERS_PER_PAGE;
    let id_in_page = controller_id % NUM_CONTROLLERS_PER_PAGE;
    (page_num, id_in_page)
}

fn page_to_controller_id(page_num: usize, id_in_page: usize) -> usize {
    page_num * NUM_CONTROLLERS_PER_PAGE + id_in_page
}

fn get_num_pages(controllers: &[Box<dyn button_controllers::Controller>]) -> usize {
    let len = controllers.len();
    let num = NUM_CONTROLLERS_PER_PAGE;
    len / num + usize::from(len % num != 0)
}

fn main() -> Result<()> {
    pretty_env_logger::formatted_timed_builder()
        .filter(None, LevelFilter::Trace)
        .write_style(WriteStyle::Always)
        .init();

    let (tx, rx) = mpsc::channel();

    let board = boards::configure_devices(tx.clone())?;
    let display = board.get_display();

    let config_list = config::get_controllers_config();

    let mut controllers: Vec<Box<dyn button_controllers::Controller>> =
        config_list.iter().map(|x| x.create_controller()).collect();

    let mqtt = mqtt::Mqtt::connect(MQTT_URL, tx.clone());

    for (index, f) in controllers.iter().enumerate() {
        let subscriptions = f.get_subscriptions();
        for s in subscriptions {
            let label = mqtt::Label::Button(index, s.label);
            info!("Subscribing to {}.", s.topic);
            mqtt.subscribe(&s.topic, label);
        }
    }

    mqtt.subscribe(config::NIGHT_TOPIC, mqtt::Label::NightStatus);

    let mut timer_service = EspTimerService::new().unwrap();
    let mut timer = timer_service
        .timer(move || {
            tx.send(Message::BlankDisplays).unwrap();
        })
        .unwrap();

    let mut requested_display_status: RequestedDisplayStatus = RequestedDisplayStatus {
        time_of_day: TimeOfDay::Day,
        forced_on: false,
        night_timer: false,
    };
    let mut status: ActualDisplayStatus = ActualDisplayStatus {
        display_on: true,
        timer_on: false,
    };

    do_blank(
        &display,
        &mut timer,
        &requested_display_status,
        &mut status,
        false,
    );

    let mut page_num = 0;
    let last_page = get_num_pages(&controllers) - 1;

    update_displays(&display, &controllers, page_num);
    display.send(DisplayCommand::ShowPage(page_num)).unwrap();

    for received in rx {
        match received {
            Message::MqttReceived(_, power, mqtt::Label::NightStatus) => {
                info!("Got night: {}", power);
                match power.as_str() {
                    "ON" => requested_display_status.time_of_day = TimeOfDay::Night,
                    "OFF" => requested_display_status.time_of_day = TimeOfDay::Day,
                    _ => {}
                };
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    false,
                );
            }
            Message::MqttReceived(topic, data, mqtt::Label::Button(id, sid)) => {
                info!("Got message: {} - {}", topic, data);
                let controller = controllers.get_mut(id as usize).unwrap();
                let old_state = controller.get_display_state();
                controller.process_message(sid, data);
                let state = controller.get_display_state();
                if id == config::NIGHT_CONTROLLER {
                    match state {
                        DisplayState::Off => requested_display_status.forced_on = false,
                        DisplayState::HardOff => requested_display_status.forced_on = false,
                        DisplayState::On => requested_display_status.forced_on = true,
                        DisplayState::OnOther => requested_display_status.forced_on = true,
                        DisplayState::Error => {}
                        DisplayState::Unknown => {}
                    }
                    do_blank(
                        &display,
                        &mut timer,
                        &requested_display_status,
                        &mut status,
                        false,
                    );
                }

                let (msg_page_num, id_in_page) = controller_to_page_id(id);
                if page_num == msg_page_num && old_state != state {
                    update_display(&display, id_in_page, controller.as_ref(), state);
                }
            }
            Message::MqttConnect => {
                info!("Got connected");
            }
            Message::MqttDisconnect => {
                info!("Got disconnected");
                for controller in controllers.iter_mut() {
                    controller.process_disconnected();
                }
                update_displays(&display, &controllers, page_num);
            }
            Message::ButtonPress(ButtonId::Physical(id_in_page)) => {
                if status.display_on {
                    let id = page_to_controller_id(page_num, id_in_page);
                    button_press(&mut controllers, id, &mqtt);
                    display
                        .send(DisplayCommand::ButtonPressed(id_in_page))
                        .unwrap();
                }
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonPress(ButtonId::Controller(id)) => {
                button_press(&mut controllers, id, &mqtt);
                let (msg_page_num, id_in_page) = controller_to_page_id(id);
                if msg_page_num == page_num {
                    display
                        .send(DisplayCommand::ButtonPressed(id_in_page))
                        .unwrap();
                }
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonPress(ButtonId::PageUp) => {
                info!("got page up");
                page_num = page_num.saturating_add(1);
                if page_num > last_page {
                    page_num = last_page
                };
                display.send(DisplayCommand::ShowPage(page_num)).unwrap();
                update_displays(&display, &controllers, page_num);
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonPress(ButtonId::PageDown) => {
                info!("got page down");
                page_num = page_num.saturating_sub(1);
                display.send(DisplayCommand::ShowPage(page_num)).unwrap();
                update_displays(&display, &controllers, page_num);
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonRelease(ButtonId::Physical(id_in_page)) => {
                info!("Got button release");
                display
                    .send(DisplayCommand::ButtonReleased(id_in_page))
                    .unwrap();
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonRelease(ButtonId::Controller(id)) => {
                info!("Got button release");
                let (msg_page_num, id_in_page) = controller_to_page_id(id);
                if msg_page_num == page_num {
                    display
                        .send(DisplayCommand::ButtonReleased(id_in_page))
                        .unwrap();
                }
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonRelease(_) => {
                info!("Got button release");
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::BlankDisplays => {
                info!("Got blank display timer");
                requested_display_status.turn_night_timer_off();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
        }
    }

    Ok(())
}
