use crate::messages;
use crate::mqtt;

pub enum Message {
    MqttConnect,
    MqttDisconnect,
    MqttReceived(String, String, mqtt::Label),
    ButtonPress(u32),
    ButtonRelease(u32),
    BlankDisplays,
}

pub type Sender = std::sync::mpsc::Sender<messages::Message>;
