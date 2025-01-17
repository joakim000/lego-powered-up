// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Definitions for the various devices which can attach to hubs, e.g. motors

use crate::error::{Error, Result};
use crate::hubs::Port;
use crate::notifications::{HubLedMode, NotificationMessage, Power};
use async_trait::async_trait;
use btleplug::api::{Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use std::fmt::Debug;

/// Trait that any device may implement. Having a single trait covering
/// every device is probably the wrong design, and we should have better
/// abstractions for e.g. motors vs. sensors & LEDs.
#[async_trait]
pub trait Device: Debug + Send + Sync {
    fn port(&self) -> Port;
    fn peripheral(&self) -> &Peripheral;
    fn characteristic(&self) -> &Characteristic;
    async fn send(&mut self, msg: NotificationMessage) -> Result<()> {
        let buf = msg.serialise();
        self.peripheral()
            .write(self.characteristic(), &buf, WriteType::WithoutResponse)
            .await?;
        Ok(())
    }
    async fn set_rgb(&mut self, _rgb: &[u8; 3]) -> Result<()> {
        Err(Error::NotImplementedError(
            "Not implemented for type".to_string(),
        ))
    }
    async fn start_speed(
        &mut self,
        _speed: i8,
        _max_power: Power,
    ) -> Result<()> {
        Err(Error::NotImplementedError(
            "Not implemented for type".to_string(),
        ))
    }
}

/// Struct representing a Hub LED
#[derive(Debug, Clone)]
pub struct HubLED {
    /// RGB colour value
    rgb: [u8; 3],
    _mode: HubLedMode,
    peripheral: Peripheral,
    characteristic: Characteristic,
    port_id: u8,
}

#[async_trait]
impl Device for HubLED {
    fn port(&self) -> Port {
        Port::HubLed
    }

    fn peripheral(&self) -> &Peripheral {
        &self.peripheral
    }

    fn characteristic(&self) -> &Characteristic {
        &self.characteristic
    }

    async fn set_rgb(&mut self, rgb: &[u8; 3]) -> Result<()> {
        use crate::notifications::*;

        self.rgb = *rgb;

        let mode_set_msg =
            NotificationMessage::PortInputFormatSetupSingle(InputSetupSingle {
                port_id: 50,
                mode: 0x01,
                delta: 0x00000001,
                notification_enabled: false,
            });
        self.send(mode_set_msg).await?;

        let subcommand = PortOutputSubcommand::WriteDirectModeData(
            WriteDirectModeDataPayload::SetRgbColors {
                red: rgb[0],
                green: rgb[1],
                blue: rgb[2],
            },
        );

        let msg =
            NotificationMessage::PortOutputCommand(PortOutputCommandFormat {
                port_id: self.port_id,
                startup_info: StartupInfo::ExecuteImmediately,
                completion_info: CompletionInfo::NoAction,
                subcommand,
            });
        self.send(msg).await
    }
}

impl HubLED {
    pub(crate) fn new(
        peripheral: Peripheral,
        characteristic: Characteristic,
        port_id: u8,
    ) -> Self {
        let mode = HubLedMode::Rgb;
        Self {
            rgb: [0; 3],
            _mode: mode,
            characteristic,
            peripheral,
            port_id,
        }
    }
}

/// Struct representing a motor
#[derive(Debug, Clone)]
pub struct Motor {
    peripheral: Peripheral,
    characteristic: Characteristic,
    port: Port,
    port_id: u8,
}

#[async_trait]
impl Device for Motor {
    fn port(&self) -> Port {
        self.port
    }

    fn peripheral(&self) -> &Peripheral {
        &self.peripheral
    }

    fn characteristic(&self) -> &Characteristic {
        &self.characteristic
    }

    async fn start_speed(&mut self, speed: i8, max_power: Power) -> Result<()> {
        use crate::notifications::*;

        let subcommand = PortOutputSubcommand::StartSpeed {
            speed,
            max_power,
            use_acc_profile: true,
            use_dec_profile: true,
        };
        let msg =
            NotificationMessage::PortOutputCommand(PortOutputCommandFormat {
                port_id: self.port_id,
                startup_info: StartupInfo::ExecuteImmediately,
                completion_info: CompletionInfo::NoAction,
                subcommand,
            });
        self.send(msg).await
    }
}

impl Motor {
    pub(crate) fn new(
        peripheral: Peripheral,
        characteristic: Characteristic,
        port: Port,
        port_id: u8,
    ) -> Self {
        Self {
            peripheral,
            characteristic,
            port,
            port_id,
        }
    }
}
