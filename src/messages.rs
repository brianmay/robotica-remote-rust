use crate::button;
use crate::messages;
use crate::mqtt;

pub enum Message {
    MqttConnect,
    MqttDisconnect,
    MqttReceived(String, String, mqtt::Label),
    #[allow(dead_code)]
    ButtonPress(button::ButtonId),
    #[allow(dead_code)]
    ButtonRelease(button::ButtonId),
    BlankDisplays,
}

pub type Sender = std::sync::mpsc::Sender<messages::Message>;
