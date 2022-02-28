use crate::button_controllers::*;

#[derive(Clone)]
pub struct LightConfig {
    pub c: CommonConfig,
    pub scene: String,
    pub priority: Priority,
}

impl Config for LightConfig {
    fn create_controller(&self) -> Box<dyn Controller> {
        Box::new(LightController::new(self))
    }
}

pub struct LightController {
    config: LightConfig,
    power: Option<String>,
    scenes: Option<Vec<String>>,
    priorities: Option<Vec<Priority>>,
}

impl LightController {
    pub fn new(config: &LightConfig) -> Self {
        Self {
            config: config.clone(),
            power: None,
            scenes: None,
            priorities: None,
        }
    }
}

fn topic(parts: &[&str]) -> String {
    parts.join("/")
}

impl Controller for LightController {
    fn get_subscriptions(&self) -> Vec<Subscription> {
        let mut result: Vec<Subscription> = Vec::new();
        let config = &self.config;

        let p = ["state", &config.c.location, &config.c.device, "power"];
        let s = Subscription {
            topic: topic(&p),
            label: ButtonStateMsgType::Power as u32,
        };
        result.push(s);

        let p = ["state", &config.c.location, &config.c.device, "scenes"];
        let s = Subscription {
            topic: topic(&p),
            label: ButtonStateMsgType::Scenes as u32,
        };
        result.push(s);

        let p = ["state", &config.c.location, &config.c.device, "priorities"];
        let s = Subscription {
            topic: topic(&p),
            label: ButtonStateMsgType::Priorities as u32,
        };
        result.push(s);

        result
    }

    fn process_message(&mut self, label: Label, data: String) {
        match label.try_into() {
            Ok(ButtonStateMsgType::Power) => self.power = Some(data),

            Ok(ButtonStateMsgType::Scenes) => match serde_json::from_str(&data) {
                Ok(scenes) => self.scenes = Some(scenes),
                Err(e) => error!("Invalid scenes value {}: {}", data, e),
            },

            Ok(ButtonStateMsgType::Priorities) => match serde_json::from_str(&data) {
                Ok(priorities) => self.priorities = Some(priorities),
                Err(e) => error!("Invalid priorities value {}: {}", data, e),
            },

            _ => error!("Invalid message label {}", label),
        }
    }

    fn process_disconnected(&mut self) {
        self.power = None;
        self.scenes = None;
        self.priorities = None;
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
        let mut message = serde_json::json!({
            "scene": self.config.scene,
            "priority": self.config.priority,
        });

        match self.config.c.action {
            Action::TurnOn => {}
            Action::TurnOff => message["action"] = serde_json::json!("turn_off"),
            Action::Toggle => {
                let display_state = self.get_display_state();
                if let DisplayState::On = display_state {
                    message["action"] = serde_json::json!("turn_off");
                };
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

fn get_display_state_turn_on(lb: &LightController) -> DisplayState {
    let power = lb.power.as_deref();
    let scenes = lb.scenes.as_deref();
    let scene = &lb.config.scene;

    let scenes_empty = match scenes {
        Some(scenes) if !scenes.is_empty() => false,
        Some(_) => true,
        None => true,
    };

    match power {
        None => DisplayState::Unknown,
        Some("HARD_OFF") => DisplayState::HardOff,
        Some("ON") if scenes_empty => DisplayState::OnOther,
        Some("OFF") if scenes_empty => DisplayState::Off,
        _ => match scenes {
            None => DisplayState::Unknown,
            Some(scenes) if scenes.contains(scene) => DisplayState::On,
            Some(_) if !scenes_empty => DisplayState::OnOther,
            Some(_) => DisplayState::Off,
        },
    }
}

fn get_display_state_turn_off(lb: &LightController) -> DisplayState {
    let power = lb.power.as_deref();
    let scenes = lb.scenes.as_deref();
    let priorities = lb.priorities.as_deref();
    let priority = lb.config.priority;

    let scenes_empty = match scenes {
        Some(scenes) if !scenes.is_empty() => false,
        Some(_) => true,
        None => true,
    };

    match power {
        None => DisplayState::Unknown,
        Some("HARD_OFF") => DisplayState::HardOff,
        Some("ON") if scenes_empty => DisplayState::Off,
        Some("OFF") if scenes_empty => DisplayState::On,
        _ => match priorities {
            None => DisplayState::Unknown,
            Some(priorities) if priorities.contains(&priority) => DisplayState::Off,
            Some(_) => DisplayState::On,
        },
    }
}

fn get_display_state_toggle(lb: &LightController) -> DisplayState {
    let power = lb.power.as_deref();
    let scenes = lb.scenes.as_deref();
    let scene = &lb.config.scene;

    let scenes_empty = match scenes {
        Some(scenes) if !scenes.is_empty() => false,
        Some(_) => true,
        None => true,
    };

    match power {
        None => DisplayState::Unknown,
        Some("HARD_OFF") => DisplayState::HardOff,
        Some("ON") if scenes_empty => DisplayState::OnOther,
        Some("OFF") if scenes_empty => DisplayState::Off,
        _ => match scenes {
            None => DisplayState::Unknown,
            Some(scenes) if scenes.contains(scene) => DisplayState::On,
            Some(_) if !scenes_empty => DisplayState::OnOther,
            Some(_) => DisplayState::Off,
        },
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

type Priority = i32;
