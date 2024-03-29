use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;

use embedded_svc::{
    mqtt::client::{Connection, Details, Event, Message, MessageImpl, QoS},
    utils::mqtt::client::ConnState,
};

use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_sys::EspError;

use log::*;

use crate::{hardware::esp32::get_unique_id, messages};

#[derive(Clone, Debug)]
pub enum Label {
    Button(usize, u32),
    NightStatus,
    LightStatus,
}

#[derive(Debug)]
pub struct Subscription {
    label: Label,
}

pub struct Subscriptions(HashMap<String, Vec<Subscription>>);

impl Subscriptions {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add(&mut self, topic: &str, label: Label) {
        let subscriptions = self.0.entry(topic.to_string()).or_default();
        subscriptions.push(Subscription { label });
    }
}

pub struct Mqtt {
    tx: mpsc::Sender<MqttCommand>,
}

enum MqttCommand {
    MqttConnect,
    MqttDisconnect,
    MqttReceived(String, String),
    Publish(String, bool, String),
}

fn event_to_string(event: &Event<MessageImpl>) -> String {
    match event {
        Event::BeforeConnect => "BeforeConnect".to_string(),
        Event::Connected(connected) => format!("Connected(connected: {connected})"),
        Event::Disconnected => "Disconnected".to_string(),
        Event::Subscribed(message_id) => format!("Subscribed({message_id})"),
        Event::Unsubscribed(message_id) => format!("Unsubscribed({message_id})"),
        Event::Published(message_id) => format!("Published({message_id})"),
        Event::Received(message) => format!("Received({})", message.id()),
        Event::Deleted(message_id) => format!("Deleted({message_id})"),
    }
}

fn get_client(
    url: &str,
    tx: mpsc::Sender<MqttCommand>,
) -> Result<EspMqttClient<ConnState<MessageImpl, EspError>>, EspError> {
    let client_id = format!("robotica-remote-rust_{}", get_unique_id());
    let conf = MqttClientConfiguration {
        client_id: Some(&client_id),
        keep_alive_interval: Some(std::time::Duration::new(60, 0)),
        ..Default::default()
    };

    let (client, mut connection) = EspMqttClient::new_with_conn(url, &conf)?;

    thread::spawn(move || {
        while let Some(msg) = connection.next() {
            let event = msg.as_ref().unwrap();
            match event {
                Event::Received(msg) => match msg.details() {
                    Details::Complete => {
                        let topic = msg.topic().unwrap().to_string();
                        let raw = msg.data();
                        let data = std::str::from_utf8(raw).unwrap();
                        tx.send(MqttCommand::MqttReceived(topic, data.to_string()))
                            .unwrap();
                    }
                    Details::InitialChunk(_) => error!("Got InitialChunk message"),
                    Details::SubsequentChunk(_) => error!("Got SubsequentChunk message"),
                },
                Event::Connected(_) => {
                    tx.send(MqttCommand::MqttConnect).unwrap();
                }
                Event::Disconnected => {
                    tx.send(MqttCommand::MqttDisconnect).unwrap();
                }
                Event::Subscribed(_x) => {
                    // Do nothing
                }
                event => info!("Got unknown MQTT event {:?}", event_to_string(event)),
            }
        }
    });

    Ok(client)
}

impl Mqtt {
    pub fn connect(
        url: &str,
        tx_to_client: messages::Sender,
        subscriptions: Subscriptions,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let url = url.to_string();

        let tx_copy = tx.clone();

        thread::spawn(move || {
            let mut client = get_client(&url, tx_copy).unwrap();

            for received in rx {
                match received {
                    MqttCommand::MqttConnect => {
                        for (topic, _) in subscriptions.0.iter() {
                            client.subscribe(topic, QoS::AtMostOnce).unwrap();
                        }
                        tx_to_client.send(messages::Message::MqttConnect).unwrap();
                    }

                    MqttCommand::MqttDisconnect => {
                        tx_to_client
                            .send(messages::Message::MqttDisconnect)
                            .unwrap();
                    }

                    MqttCommand::MqttReceived(topic, data) => {
                        if let Some(list) = subscriptions.0.get(&topic) {
                            for s in list {
                                tx_to_client
                                    .send(messages::Message::MqttReceived(
                                        topic.clone(),
                                        data.to_string(),
                                        s.label.clone(),
                                    ))
                                    .unwrap();
                            }
                        }
                    }

                    MqttCommand::Publish(topic, retain, data) => {
                        debug!("Publishing {} {}", topic, data);
                        client
                            .publish(&topic, QoS::AtMostOnce, retain, data.as_bytes())
                            .unwrap();
                    }
                }
            }
        });

        Mqtt { tx }
    }

    pub fn publish(&self, topic: &str, retain: bool, data: &str) {
        let tx = self.tx.clone();
        tx.send(MqttCommand::Publish(
            topic.to_string(),
            retain,
            data.to_string(),
        ))
        .unwrap();
    }
}
