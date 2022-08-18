use crate::button_controllers::lights::LightConfig;
use crate::button_controllers::music::MusicConfig;
use crate::button_controllers::switch::SwitchConfig;
use crate::button_controllers::Action;
use crate::button_controllers::CommonConfig;
use crate::button_controllers::Config;
use crate::button_controllers::Icon;

pub const NUM_CONTROLLERS: usize = 6;
pub const NIGHT_TOPIC: &str = "state/Brian/Night/power";
pub const LIGHT_TOPIC: &str = "state/Brian/Light/power";

pub fn get_controllers_config() -> [Box<dyn Config>; NUM_CONTROLLERS] {
    [
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Brian Auto".to_string(),
                topic_substr: "Brian/Light".to_string(),
                action: Action::Toggle,
                icon: Icon::Light,
            },
            scene: "auto".to_string(),
            priority: 100,
        }),
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Brian On".to_string(),
                topic_substr: "Brian/Light".to_string(),
                action: Action::Toggle,
                icon: Icon::Light,
            },
            scene: "default".to_string(),
            priority: 100,
        }),
        Box::new(SwitchConfig {
            c: CommonConfig {
                name: "Brian Fan".to_string(),
                topic_substr: "Brian/Fan".to_string(),
                action: Action::Toggle,
                icon: Icon::Fan,
            },
        }),
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Passage".to_string(),
                topic_substr: "Passage/Light".to_string(),
                action: Action::Toggle,
                icon: Icon::Light,
            },
            scene: "default".to_string(),
            priority: 100,
        }),
        Box::new(MusicConfig {
            c: CommonConfig {
                name: "Brian Wake-Up".to_string(),
                topic_substr: "Brian/Robotica".to_string(),
                action: Action::Toggle,
                icon: Icon::WakeUp,
            },
            play_list: "wake_up".to_string(),
        }),
        Box::new(SwitchConfig {
            c: CommonConfig {
                name: "TV".to_string(),
                topic_substr: "Dining/TvSwitch".to_string(),
                action: Action::Toggle,
                icon: Icon::TV,
            },
        }),
    ]
}
