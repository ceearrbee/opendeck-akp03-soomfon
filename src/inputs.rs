use mirajazz::{error::MirajazzError, types::DeviceInput};

pub fn process_input(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    crate::file_log(&format!("!!! INPUT DETECTED !!! - Code: 0x{:02X}, State: 0x{:02X}", input, state));
    log::info!("!!! INPUT DETECTED !!! - Code: 0x{:02X}, State: 0x{:02X}", input, state);

    match input {
        (1..=6) | 0x25 | 0x30 | 0x31 => read_button_press(input, state),
        0x90 | 0x91 | 0x50 | 0x51 | 0x60 | 0x61 => read_encoder_value(input),
        0x33..=0x35 => read_encoder_press(input, state),
        0x00 => Ok(DeviceInput::NoData),
        _ => Err(MirajazzError::BadData),
    }
}

fn read_button_press(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    let index: u8 = match input {
        // Six buttons with displays
        (1..=6) => input - 1,
        // Three buttons without displays
        0x25 => 6,
        0x30 => 7,
        0x31 => 8,
        _ => return Err(MirajazzError::BadData),
    };

    if state != 0 {
        Ok(DeviceInput::ButtonDown(index))
    } else {
        Ok(DeviceInput::ButtonUp(index))
    }
}

fn read_encoder_value(input: u8) -> Result<DeviceInput, MirajazzError> {
    let (encoder, value): (u8, i8) = match input {
        // Left encoder (Bottom Left)
        0x90 => (0, -1),
        0x91 => (0, 1),
        // Middle (top) encoder
        0x50 => (1, -1),
        0x51 => (1, 1),
        // Right encoder (Bottom Right)
        0x60 => (2, -1),
        0x61 => (2, 1),
        _ => return Err(MirajazzError::BadData),
    };

    Ok(DeviceInput::SingleEncoderTwist(encoder, value))
}

fn read_encoder_press(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    let encoder: u8 = match input {
        0x33 => 0, // Left encoder (Bottom Left)
        0x35 => 1, // Middle (top) encoder
        0x34 => 2, // Right encoder (Bottom Right)
        _ => return Err(MirajazzError::BadData),
    };

    if state != 0 {
        Ok(DeviceInput::EncoderDown(encoder))
    } else {
        Ok(DeviceInput::EncoderUp(encoder))
    }
}
