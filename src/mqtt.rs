use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use anyhow::Error;

use embedded_svc::mqtt::client::{Client, Event, Message, Publish, QoS, TopicToken};
use esp_idf_hal::ledc::Resolution;
use esp_idf_svc::mqtt::client::{
    EspMqttClient, EspMqttConnection, EspMqttMessage, MqttClientConfiguration,
};
use esp_idf_sys::EspError;

use log::*;

use crate::messages;

type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Clone)]
pub struct Label {
    pub component_id: u32,
    pub subscription_id: u32,
}

struct Subscription {
    label: Label,
}

type Subscriptions = HashMap<String, Vec<Subscription>>;
pub struct Mqtt {
    tx: Option<mpsc::Sender<MqttCommand>>,
    url: String,
}

enum MqttCommand {
    MqttConnect,
    MqttDisconnect,
    MqttReceived(String, String),
    Subscribe(String, Label),
    Publish(String, bool, String),
}

fn get_client(url: &str, tx: mpsc::Sender<MqttCommand>) -> Result<EspMqttClient, EspError> {
    let callback = move |msg: Option<Result<Event<EspMqttMessage>, EspError>>| {
        info!("Got callback");
        let event_or_error = msg.unwrap();
        match event_or_error {
            Err(e) => info!("MQTT Message ERROR: {}", e),
            Ok(Event::Received(msg)) => {
                let token = unsafe { TopicToken::new() };
                let topic = msg.topic(&token).to_string();
                let raw = msg.data();
                let data = std::str::from_utf8(&raw).unwrap();
                tx.send(MqttCommand::MqttReceived(topic, data.to_string()))
                    .unwrap();
            }
            Ok(Event::Connected(_)) => {
                tx.send(MqttCommand::MqttConnect).unwrap();
            }
            Ok(Event::Disconnected) => {
                tx.send(MqttCommand::MqttDisconnect).unwrap();
            }
            Ok(event) => info!("MQTT event: {:?}", event),
        }
        info!("Done callback");
    };

    let conf = MqttClientConfiguration {
        client_id: Some("rust-esp32-std-demo"),
        ..Default::default()
    };

    EspMqttClient::new_with_callback(url, &conf, callback)
}

impl Mqtt {
    pub fn new(url: &str) -> Self {
        Mqtt {
            tx: None,
            url: url.to_string(),
        }
    }

    pub fn connect(&mut self, tx_to_client: messages::Sender) -> Result<()> {
        let url = self.url.clone();
        let (tx, rx) = mpsc::channel();
        self.tx = Some(tx.clone());

        thread::spawn(move || {
            let mut client = get_client(&url, tx).unwrap();

            let mut subscriptions: Subscriptions = HashMap::new();

            for received in rx {
                match received {
                    MqttCommand::MqttConnect => {
                        info!("got MqttConnect");
                        for (topic, _) in subscriptions.iter() {
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
                        info!("got MqttMessage");
                        if let Some(list) = subscriptions.get(&topic) {
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

                    MqttCommand::Subscribe(topic, label) => {
                        info!("got Subscribe");
                        let subscription = Subscription { label };

                        match subscriptions.get_mut(&topic) {
                            Some(list) => list.push(subscription),
                            None => {
                                subscriptions.insert(topic.to_string(), vec![subscription]);
                                client.subscribe(topic, QoS::AtMostOnce).unwrap();
                            }
                        };
                    }

                    MqttCommand::Publish(topic, retain, data) => {
                        info!("got Publish");
                        client
                            .publish(topic, QoS::AtMostOnce, retain, data.as_bytes())
                            .unwrap();
                    }
                }
            }
        });

        Ok(())
    }

    pub fn subscribe(&self, topic: &str, label: Label) -> Result<()> {
        let tx = self.tx.clone().unwrap();
        tx.send(MqttCommand::Subscribe(topic.to_string(), label))?;
        Ok(())
    }

    pub fn publish(&self, topic: &str, retain: bool, data: &str) -> Result<()> {
        let tx = self.tx.clone().unwrap();
        tx.send(MqttCommand::Publish(
            topic.to_string(),
            retain,
            data.to_string(),
        ))?;
        Ok(())
    }
}
