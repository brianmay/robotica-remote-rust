use crate::button_controllers::lights::LightConfig;
use crate::button_controllers::switch::SwitchConfig;
use crate::button_controllers::Action;
use crate::button_controllers::CommonConfig;
use crate::button_controllers::Config;
use crate::button_controllers::Icon;

pub const NUM_CONTROLLERS: usize = 2;
pub const NIGHT_TOPIC: &str = "state/Brian/Night/power";

pub fn get_controllers_config() -> [Box<dyn Config>; NUM_CONTROLLERS] {
    [
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Brian".to_string(),
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
                name: "Brian".to_string(),
                location: "Brian".to_string(),
                device: "Fan".to_string(),
                action: Action::Toggle,
                icon: Icon::Fan,
            },
        }),
    ]
}
