use crate::button;
use crate::messages;
use crate::mqtt;

pub enum Message {
    MqttConnect,
    MqttDisconnect,
    MqttReceived(String, String, mqtt::Label),
    ButtonPress(button::ButtonId),
    ButtonRelease(button::ButtonId),
    BlankDisplays,
    DisplayPage(usize),
}

pub type Sender = std::sync::mpsc::Sender<messages::Message>;
