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

fn topic(parts: &[String]) -> String {
    parts.join("/")
}

impl Controller for SwitchController {
    fn get_subscriptions(&self) -> Vec<Subscription> {
        let mut result: Vec<Subscription> = Vec::new();
        let config = &self.config;

        let p = [
            "state".to_string(),
            config.c.location.clone(),
            config.c.device.clone(),
            "power".to_string(),
        ];
        let s = Subscription {
            topic: topic(&p),
            label: ButtonStateMsgType::Power as u32,
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
        let power = self.power.as_deref();

        let state = match power {
            None => DisplayState::Unknown,
            Some("HARD_OFF") => DisplayState::HardOff,
            Some("ON") => DisplayState::On,
            Some("OFF") => DisplayState::Off,
            _ => DisplayState::Error,
        };

        let action = &self.config.c.action;
        get_display_state_for_action(state, action)
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
        self.config.c.icon.clone()
    }

    fn get_name(&self) -> String {
        self.config.c.name.clone()
    }
}

enum ButtonStateMsgType {
    Power,
}

impl TryFrom<u32> for ButtonStateMsgType {
    type Error = ();

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            x if x == ButtonStateMsgType::Power as u32 => Ok(ButtonStateMsgType::Power),
            _ => Err(()),
        }
    }
}
