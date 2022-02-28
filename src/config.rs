use crate::button_controllers::lights::LightConfig;
use crate::button_controllers::music::MusicConfig;
use crate::button_controllers::switch::SwitchConfig;
use crate::button_controllers::Action;
use crate::button_controllers::CommonConfig;
use crate::button_controllers::Config;
use crate::button_controllers::Icon;

pub const NUM_CONTROLLERS: usize = 5;
pub const NIGHT_TOPIC: &str = "state/Brian/Night/power";

pub fn get_controllers_config() -> [Box<dyn Config>; NUM_CONTROLLERS] {
    [
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Brian Light".to_string(),
                location: "Brian".to_string(),
                device: "Light".to_string(),
                action: Action::Toggle,
                icon: Icon::Light,
            },
            scene: "auto".to_string(),
            priority: 100,
        }),
        Box::new(SwitchConfig {
            c: CommonConfig {
                name: "Brian Fan".to_string(),
                location: "Brian".to_string(),
                device: "Fan".to_string(),
                action: Action::Toggle,
                icon: Icon::Fan,
            },
        }),
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Passage".to_string(),
                location: "Passage".to_string(),
                device: "Light".to_string(),
                action: Action::Toggle,
                icon: Icon::Light,
            },
            scene: "default".to_string(),
            priority: 100,
        }),
        Box::new(MusicConfig {
            c: CommonConfig {
                name: "Brian Wake-Up".to_string(),
                location: "Brian".to_string(),
                device: "Robotica".to_string(),
                action: Action::Toggle,
                icon: Icon::WakeUp,
            },
            play_list: "wake_up".to_string(),
        }),
        Box::new(SwitchConfig {
            c: CommonConfig {
                name: "TV".to_string(),
                location: "Dining".to_string(),
                device: "TvSwitch".to_string(),
                action: Action::Toggle,
                icon: Icon::TV,
            },
        }),
    ]
}
