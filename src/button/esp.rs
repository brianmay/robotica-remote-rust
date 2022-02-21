use anyhow::Error;

use crate::button::*;
use crate::messages;

type Result<T, E = Error> = core::result::Result<T, E>;

pub fn configure_button<T: 'static + InputPin<Error = EspError> + Send>(
    pin: T,
    tx: messages::Sender,
    id: u32,
) -> Result<()> {
    let frequency = 100;

    let debounced_encoder_pin = Debouncer::new(pin, Active::Low, 30, frequency);
    let encoder_button_1 = Button::new(debounced_encoder_pin, id);
    encoder_button_1.connect(tx);

    Ok(())
}
