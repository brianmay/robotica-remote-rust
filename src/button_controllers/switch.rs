use display_interface::DisplayError;

use crate::button_controllers::*;

#[derive(Clone)]
pub struct SwitchConfig {
    pub c: CommonConfig,
}

impl Config for SwitchConfig {
    fn create_controller(&self) -> Box<dyn Controller> {
        Box::new(SwitchController::new(self))
    }
}

pub struct SwitchController {
    config: SwitchConfig,
    power: Option<String>,
}

impl SwitchController {
    pub fn new(config: &SwitchConfig) -> Self {
        Self {
            config: config.clone(),
            power: None,
        }
    }
}

fn topic(parts: Vec<String>) -> String {
    parts.join("/")
}

impl Controller for SwitchController {
    fn get_subscriptions(&self) -> Vec<Subscription> {
        let mut result: Vec<Subscription> = Vec::new();
        let config = &self.config;

        let p = vec![
            "state".to_string(),
            config.c.location.clone(),
            config.c.device.clone(),
            "power".to_string(),
        ];
        let s = Subscription {
            topic: topic(p),
            label: ButtonStateMsgType::Power as u32,
        };
        result.push(s);

        let p = vec![
            "state".to_string(),
            config.c.location.clone(),
            config.c.device.clone(),
            "scenes".to_string(),
        ];
        let s = Subscription {
            topic: topic(p),
            label: ButtonStateMsgType::Scenes as u32,
        };
        result.push(s);

        let p = vec![
            "state".to_string(),
            config.c.location.clone(),
            config.c.device.clone(),
            "priorities".to_string(),
        ];
        let s = Subscription {
            topic: topic(p),
            label: ButtonStateMsgType::Priorities as u32,
        };
        result.push(s);

        result
    }

    fn process_message(&mut self, label: Label, data: String) {
        match label.try_into() {
            Ok(ButtonStateMsgType::Power) => self.power = Some(data),

            _ => error!("Invalid message label {}", label),
        }
    }

    fn process_disconnected(&mut self) {
        self.power = None;
    }

    fn get_display_state(&self) -> DisplayState {
        let action = &self.config.c.action;

        match action {
            Action::TurnOn => get_display_state_turn_on(self),
            Action::TurnOff => get_display_state_turn_off(self),
            Action::Toggle => get_display_state_toggle(self),
        }
    }

    fn get_press_commands(&self) -> Vec<Command> {
        let mut message = serde_json::json!({});

        match self.config.c.action {
            Action::TurnOn => message["action"] = serde_json::json!("turn_on"),
            Action::TurnOff => message["action"] = serde_json::json!("turn_off"),
            Action::Toggle => {
                let display_state = self.get_display_state();
                if let DisplayState::On = display_state {
                    message["action"] = serde_json::json!("turn_off");
                } else {
                    message["action"] = serde_json::json!("turn_on");
                }
            }
        };

        let command = Command {
            location: self.config.c.location.clone(),
            device: self.config.c.device.clone(),
            message,
        };

        vec![command]
    }

    fn get_icon(&self) -> Icon {
        return self.config.c.icon.clone();
    }
}

fn get_display_state_turn_on(lb: &SwitchController) -> DisplayState {
    let power = lb.power.as_deref();

    match power {
        None => DisplayState::Unknown,
        Some("HARD_OFF") => DisplayState::HardOff,
        Some("ON") => DisplayState::On,
        Some("OFF") => DisplayState::Off,
        _ => DisplayState::Error,
    }
}

fn get_display_state_turn_off(lb: &SwitchController) -> DisplayState {
    let power = lb.power.as_deref();

    match power {
        None => DisplayState::Unknown,
        Some("HARD_OFF") => DisplayState::HardOff,
        Some("ON") => DisplayState::Off,
        Some("OFF") => DisplayState::On,
        _ => DisplayState::Error,
    }
}

fn get_display_state_toggle(lb: &SwitchController) -> DisplayState {
    let power = lb.power.as_deref();

    match power {
        None => DisplayState::Unknown,
        Some("HARD_OFF") => DisplayState::HardOff,
        Some("ON") => DisplayState::On,
        Some("OFF") => DisplayState::Off,
        _ => DisplayState::Error,
    }
}

enum ButtonStateMsgType {
    Power,
    Scenes,
    Priorities,
}

impl TryFrom<u32> for ButtonStateMsgType {
    type Error = ();

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            x if x == ButtonStateMsgType::Power as u32 => Ok(ButtonStateMsgType::Power),
            x if x == ButtonStateMsgType::Scenes as u32 => Ok(ButtonStateMsgType::Scenes),
            x if x == ButtonStateMsgType::Priorities as u32 => Ok(ButtonStateMsgType::Priorities),
            _ => Err(()),
        }
    }
}
