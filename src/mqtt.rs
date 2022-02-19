use std::{ops::Sub, borrow::BorrowMut};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::sync::RwLock;


use esp_idf_sys::EspError;
use esp_idf_hal::ledc::Resolution;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration, EspMqttConnection, EspMqttMessage};
use embedded_svc::mqtt::client::{Client, QoS, Publish, Event, TopicToken, Message};

use log::*;

use crate::messages;

#[derive(Clone)]
pub struct Label {
    pub component_id: u32,
    pub subscription_id: u32,
}

// type Callback = fn(&str, &Label, &str);

struct Subscription {
    label: Label,
    // callback: Callback,
}

type Subscriptions = HashMap<String, Vec<Subscription>>;
pub struct Mqtt {
    url: String,
    client: Option<EspMqttClient>,
    // connection: Option<EspMqttConnection>,
    subscriptions: Arc<RwLock<Subscriptions>>,
}

fn get_client(url: &str, subscriptions: Arc<RwLock<Subscriptions>>, tx: messages::Sender) -> Result<EspMqttClient, EspError> {
    let callback = move | msg : Option<Result<Event<EspMqttMessage>, EspError>> | {
        let event_or_error = msg.unwrap();
            match event_or_error {
                Err(e) => info!("MQTT Message ERROR: {}", e),
                Ok(Event::Received(msg)) => {
                    // info!("MQTT Message: {:?}", msg);
                    let token =  unsafe { TopicToken::new() };
                    let topic = msg.topic(&token).to_string();
                    let raw = msg.data();
                    let data = std::str::from_utf8(&raw).unwrap();

                    let lock = subscriptions.read().expect("mutex is poisoned");
                    match lock.get(&topic) {
                        Some(list) =>
                            for s in list {
                                // let callback = s.callback;
                                // callback(&topic, &s.label, data);
                                tx.send(messages::Message::MqttMessage(topic.clone(), data.to_string(), s.label.clone())).unwrap();
                            },
                        None => {},
                    }
                },
                Ok(event) => info!("MQTT event: {:?}", event),
            }
    };

    let conf = MqttClientConfiguration {
        client_id: Some("rust-esp32-std-demo"),
        ..Default::default()
    };

    EspMqttClient::new_with_callback(url, &conf, callback)
}

impl Mqtt {
    pub fn new(url: &str) -> Self {
        Mqtt{
            url: url.to_string(),
            client: None,
            // connection: None,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn connect(&mut self, tx: messages::Sender) -> Result<(), Box<dyn Error>> {
        let mut client = get_client(&self.url, self.subscriptions.clone(), tx)?;

        let subscriptions = self.subscriptions.read().unwrap();
        for (topic, _) in subscriptions.iter() {
            client.subscribe(topic, QoS::AtMostOnce)?;
        }

        self.client = Some(client);

        Ok(())
    }

    pub fn subscribe(&mut self, topic: &str, label: Label) -> Result<(), Box<dyn Error>> {
        let subscription = Subscription{
            label: label,
            // callback: callback
        };

        let client = self.client.as_mut().unwrap();

        let mut subscriptions = self.subscriptions.write().unwrap();
        match subscriptions.get_mut(topic) {
            Some(list) => list.push(subscription),
            None => {
                subscriptions.insert(topic.to_string(), vec![subscription]);
                client.subscribe(topic, QoS::AtMostOnce)?;

            }
        }

        Ok(())
    }

    pub fn publish(&mut self, topic: &str, retain: bool, data: &str) -> Result<u32, Box<dyn Error>> {
        let client = self.client.as_mut().unwrap();

        let rc = client.publish(
            topic,
            QoS::AtMostOnce,
            retain,
            data.as_bytes(),
        )?;

        Ok(rc)
    }

}