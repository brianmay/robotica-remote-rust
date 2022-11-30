use crate::display::icon::Icon;
use robotica_common::controllers::lights;
use robotica_common::controllers::music;
use robotica_common::controllers::switch;
use robotica_common::controllers::Action;
use robotica_common::controllers::ConfigTrait;
use robotica_common::controllers::ControllerTrait;
use robotica_common::controllers::DisplayState;
use robotica_common::controllers::Subscription;

pub const NUM_CONTROLLERS: usize = 6;
pub const NIGHT_TOPIC: &str = "state/Brian/Night/power";
pub const LIGHT_TOPIC: &str = "state/Brian/Light/power";

pub struct Controller {
    name: String,
    icon: Icon,
    controller: Box<dyn ControllerTrait>,
}

impl Controller {
    pub fn get_subscriptions(&self) -> Vec<Subscription> {
        self.controller.get_subscriptions()
    }

    pub fn process_message(&mut self, label: u32, data: String) {
        self.controller.process_message(label, data);
    }

    pub fn process_disconnected(&mut self) {
        self.controller.process_disconnected();
    }

    pub fn get_press_commands(&self) -> Vec<robotica_common::mqtt::MqttMessage> {
        self.controller.get_press_commands()
    }

    pub fn get_display_state(&self) -> DisplayState {
        self.controller.get_display_state()
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_icon(&self) -> Icon {
        self.icon
    }
}

pub fn get_controllers_config() -> [Controller; NUM_CONTROLLERS] {
    [
        Controller {
            name: "On".to_string(),
            icon: Icon::Light,
            controller: Box::new(
                lights::Config {
                    topic_substr: "Brian/Light".to_string(),
                    action: Action::Toggle,
                    scene: "default".to_string(),
                    priority: 100,
                }
                .create_controller(),
            ),
        },
        Controller {
            name: "Auto".to_string(),
            icon: Icon::Light,
            controller: Box::new(
                lights::Config {
                    topic_substr: "Brian/Light".to_string(),
                    action: Action::Toggle,
                    scene: "auto".to_string(),
                    priority: 100,
                }
                .create_controller(),
            ),
        },
        Controller {
            name: "Brian Fan".to_string(),
            icon: Icon::Fan,
            controller: Box::new(
                switch::Config {
                    topic_substr: "Brian/Fan".to_string(),
                    action: Action::Toggle,
                }
                .create_controller(),
            ),
        },
        Controller {
            name: "Passage".to_string(),
            icon: Icon::Light,
            controller: Box::new(
                lights::Config {
                    topic_substr: "Passage/Light".to_string(),
                    action: Action::Toggle,
                    scene: "default".to_string(),
                    priority: 100,
                }
                .create_controller(),
            ),
        },
        Controller {
            name: "Brian Wake-Up".to_string(),
            icon: Icon::Speaker,
            controller: Box::new(
                music::Config {
                    topic_substr: "Brian/Robotica".to_string(),
                    action: Action::Toggle,
                    play_list: "wake_up".to_string(),
                }
                .create_controller(),
            ),
        },
        Controller {
            name: "TV".to_string(),
            icon: Icon::TV,
            controller: Box::new(
                switch::Config {
                    topic_substr: "Dining/TvSwitch".to_string(),
                    action: Action::Toggle,
                }
                .create_controller(),
            ),
        },
    ]
}
