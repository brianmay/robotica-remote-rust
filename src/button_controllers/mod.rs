pub mod lights;
pub mod music;
pub mod switch;

use std::convert::TryFrom;
use std::convert::TryInto;

use log::*;

type Label = u32;

pub struct Command {
    topic: String,
    message: serde_json::Value,
}

impl Command {
    pub fn get_topic(&self) -> &str {
        &self.topic
    }

    pub fn get_message(&self) -> String {
        self.message.to_string()
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub enum Action {
    TurnOn,
    TurnOff,
    Toggle,
}

pub struct Subscription {
    pub topic: String,
    pub label: Label,
}

#[allow(dead_code)]
#[derive(std::cmp::Eq, std::cmp::PartialEq, Clone, Debug)]
pub enum DisplayState {
    HardOff,
    Error,
    Unknown,
    On,
    Off,
    OnOther,
}

pub trait Config {
    fn create_controller(&self) -> Box<dyn Controller>;
}

#[derive(Clone, Debug)]
pub enum Icon {
    Light,
    Fan,
    WakeUp,
    TV,
}
#[derive(Clone)]
pub struct CommonConfig {
    pub name: String,
    pub topic_substr: String,
    pub action: Action,
    pub icon: Icon,
}

pub trait Controller {
    fn get_subscriptions(&self) -> Vec<Subscription>;
    fn process_disconnected(&mut self);
    fn process_message(&mut self, label: Label, data: String);
    fn get_display_state(&self) -> DisplayState;
    fn get_press_commands(&self) -> Vec<Command>;
    fn get_icon(&self) -> Icon;
    fn get_name(&self) -> String;
}

fn get_display_state_for_action(state: DisplayState, action: &Action) -> DisplayState {
    match action {
        Action::TurnOn => state,
        Action::TurnOff => match state {
            DisplayState::HardOff => DisplayState::HardOff,
            DisplayState::Error => DisplayState::Error,
            DisplayState::Unknown => DisplayState::Unknown,
            DisplayState::On => DisplayState::Off,
            DisplayState::Off => DisplayState::On,
            DisplayState::OnOther => DisplayState::Off,
        },
        Action::Toggle => state,
    }
}
