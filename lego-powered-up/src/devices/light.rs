use async_trait::async_trait;
use core::fmt::Debug;
use crate::{Error, Result};
use crate::notifications::NotificationMessage;
use crate::notifications::InputSetupSingle;
use btleplug::api::{Characteristic, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use crate::notifications::PortOutputSubcommand;
use crate::notifications::PortOutputCommandFormat;
use crate::notifications::WriteDirectModeDataPayload;
use crate::notifications::StartupInfo;
use crate::notifications::CompletionInfo;
pub use crate::consts::Color;

#[async_trait]
pub trait HubLed: Debug + Send + Sync {
    fn p(&self) -> Option<Peripheral>;
    fn c(&self) -> Option<Characteristic>;
    fn port(&self) -> u8;

    async fn set_hubled_mode(&self, mode: HubLedMode) -> Result<()> {
        let mode_set_msg =
            NotificationMessage::PortInputFormatSetupSingle(InputSetupSingle {
                port_id: self.port(),
                mode: mode as u8,
                delta: 1,
                notification_enabled: true,
            });
        let p = match self.p() {
            Some(p) => p,
            None => return Err(Error::NoneError((String::from("Not a Hub LED"))))
        };
        crate::hubs::send(p, self.c().unwrap(), mode_set_msg).await
    }

    async fn set_hubled_rgb(&self, rgb: &[u8; 3]) -> Result<()> {
        let subcommand = PortOutputSubcommand::WriteDirectModeData(
            WriteDirectModeDataPayload::SetRgbColors {
                red: rgb[0],
                green: rgb[1],
                blue: rgb[2],
            },
        );

        let msg =
            NotificationMessage::PortOutputCommand(PortOutputCommandFormat {
                port_id: self.port(),
                startup_info: StartupInfo::ExecuteImmediately,
                completion_info: CompletionInfo::NoAction,
                subcommand,
            });
        let p = match self.p() {
            Some(p) => p,
            None => return Err(Error::NoneError((String::from("Not a Hub LED"))))
        };
        crate::hubs::send(p, self.c().unwrap(), msg).await
    }

    async fn set_hubled_color(&self, color: Color) -> Result<()> {
        let subcommand = PortOutputSubcommand::WriteDirectModeData(
            WriteDirectModeDataPayload::SetRgbColorNo(color as i8));
            // {
            //     red: rgb[0],
            //     green: rgb[1],
            //     blue: rgb[2],
            // },
        // );

        let msg =
            NotificationMessage::PortOutputCommand(PortOutputCommandFormat {
                port_id: self.port(),
                startup_info: StartupInfo::ExecuteImmediately,
                completion_info: CompletionInfo::NoAction,
                subcommand,
            });
        let p = match self.p() {
            Some(p) => p,
            None => return Err(Error::NoneError((String::from("Not a Hub LED"))))
        };
        crate::hubs::send(p, self.c().unwrap(), msg).await
    }
}

pub enum HubLedMode {
    /// Colour may be set to one of a number of specific named colours
    Colour = 0x0,
    /// Colour may be set to any 12-bit RGB value
    Rgb = 0x01,
}