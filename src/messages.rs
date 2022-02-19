use std::sync::Arc;
use std::sync::Mutex;

use crate::messages;
use crate::mqtt;

pub enum Message {
    MqttMessage(String, String, mqtt::Label),
    ButtonPress(u32),
    ButtonRelease(u32),
}

pub type Sender = std::sync::mpsc::Sender<messages::Message>;