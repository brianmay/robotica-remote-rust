use crate::button_controllers::*;

#[derive(Clone)]
pub struct MusicConfig {
    pub c: CommonConfig,
    pub play_list: String,
}

impl Config for MusicConfig {
    fn create_controller(&self) -> Box<dyn Controller> {
        Box::new(MusicController::new(self))
    }
}

pub struct MusicController {
    config: MusicConfig,
    play_list: Option<String>,
}

impl MusicController {
    pub fn new(config: &MusicConfig) -> Self {
        Self {
            config: config.clone(),
            play_list: None,
        }
    }
}

fn topic(parts: &[String]) -> String {
    parts.join("/")
}

impl Controller for MusicController {
    fn get_subscriptions(&self) -> Vec<Subscription> {
        let mut result: Vec<Subscription> = Vec::new();
        let config = &self.config;

        let p = [
            "state".to_string(),
            config.c.location.clone(),
            config.c.device.clone(),
            "play_list".to_string(),
        ];
        let s = Subscription {
            topic: topic(&p),
            label: ButtonStateMsgType::PlayList as u32,
        };
        result.push(s);

        result
    }

    fn process_message(&mut self, label: Label, data: String) {
        match label.try_into() {
            Ok(ButtonStateMsgType::PlayList) => self.play_list = Some(data),

            _ => error!("Invalid message label {}", label),
        }
    }

    fn process_disconnected(&mut self) {
        self.play_list = None;
    }

    fn get_display_state(&self) -> DisplayState {
        let play_list = self.play_list.as_deref();
        let state = match play_list {
            None => DisplayState::Unknown,
            Some("ERROR") => DisplayState::Error,
            Some("STOP") => DisplayState::Off,
            Some(pl) if pl == self.config.play_list => DisplayState::On,
            _ => DisplayState::Off,
        };

        let action = &self.config.c.action;
        get_display_state_for_action(state, action)
    }

    fn get_press_commands(&self) -> Vec<Command> {
        let play = match self.config.c.action {
            Action::TurnOn => true,
            Action::TurnOff => false,
            Action::Toggle => {
                let display_state = self.get_display_state();
                !matches!(display_state, DisplayState::On)
            }
        };

        let message = if play {
            serde_json::json!({
                "music": {"play_list": self.config.play_list}
            })
        } else {
            serde_json::json!({
                "music": {"stop": true}
            })
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
    PlayList,
}

impl TryFrom<u32> for ButtonStateMsgType {
    type Error = ();

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            x if x == ButtonStateMsgType::PlayList as u32 => Ok(ButtonStateMsgType::PlayList),
            _ => Err(()),
        }
    }
}
