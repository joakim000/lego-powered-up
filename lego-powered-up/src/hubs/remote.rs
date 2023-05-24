/// Definition for the Remote Control
use super::*;
use crate::notifications::{NotificationMessage,};

#[derive(Debug, )]
pub struct RemoteControl {
    peripheral: Peripheral,
    lpf_characteristic: Characteristic,
    properties: HubProperties,
    connected_io: HashMap<u8, ConnectedIo>,
}

#[async_trait::async_trait]
impl Hub for RemoteControl {
    async fn name(&self) -> Result<String> {
        Ok(self
            .peripheral
            .properties()
            .await?
            .context("No properties found for hub")?
            .local_name
            .unwrap_or_default())
    }

    fn properties(&self) -> &HubProperties {
        &self.properties
    }
    fn characteristic(&self) -> &Characteristic {
        &self.lpf_characteristic
    }
    fn peripheral(&self) -> &Peripheral {
        &self.peripheral
    }

   

    async fn disconnect(&self) -> Result<()> {
        if self.is_connected().await? {
            self.peripheral.disconnect().await?;
        }
        Ok(())
    }

    async fn is_connected(&self) -> Result<bool> {
        Ok(self.peripheral.is_connected().await?)
    }

    async fn send_raw(&self, msg: &[u8]) -> Result<()> {
        let write_type = WriteType::WithoutResponse;
        Ok(self
            .peripheral
            .write(&self.lpf_characteristic, msg, write_type)
            .await?)
    }

    // fn send(&self, msg: NotificationMessage) -> Result<()> {
    //     let msg = msg.serialise();
    //     self.send_raw(&msg)?;
    //     Ok(())
    // }

    async fn subscribe(&self, char: Characteristic) -> Result<()> {
        Ok(self.peripheral.subscribe(&char).await?)
    }

    async fn attached_io(&self) -> Vec<ConnectedIo> {
        let mut ret = Vec::with_capacity(self.connected_io.len());
        for (_k, v) in self.connected_io.iter() {
            ret.push(v.clone());
        }

        ret.sort_by_key(|x| x.port_id);

        ret
    }

    // fn process_io_event(&mut self, evt: AttachedIo) {
    //     match evt.event {
    //         IoAttachEvent::AttachedIo { hw_rev, fw_rev } => {
    //             if let Some(port) = self.port_from_id(evt.port) {
    //                 let io = ConnectedIo {
    //                     port_id: evt.port,
    //                     port,
    //                     fw_rev,
    //                     hw_rev,
    //                 };
    //                 self.connected_io.insert(evt.port, io);
    //             }
    //         }
    //         IoAttachEvent::DetachedIo { io_type_id: _ } => {}
    //         IoAttachEvent::AttachedVirtualIo {
    //             port_a: _,
    //             port_b: _,
    //         } => {}
    //     }
    // }

    async fn port(&self, port_id: Port) -> Result<Box<dyn Device>> {
        let port =
            *self.properties.port_map.get(&port_id).ok_or_else(|| {
                crate::Error::NoneError(format!(
                    "Port type `{port_id:?}` not supported"
                ))
            })?;
        Ok(match port_id {
            Port::HubLed => Box::new(devices::HubLED::new(
                self.peripheral.clone(),
                self.lpf_characteristic.clone(),
                port,
            )),
            Port::A | Port::B  => {
                Box::new(devices::RemoteButtons::new(
                    self.peripheral.clone(),
                    self.lpf_characteristic.clone(),
                    port_id,
                    port,
                ))
            }
            _ => todo!(),
        })
    }

     // Port information
     async fn request_port_info(&mut self, port_id: u8, infotype: InformationType) -> Result<()> {
        let msg =
        NotificationMessage::PortInformationRequest(InformationRequest {
            port_id,
            information_type: infotype,
        });
        self.send(msg).await
    }
    async fn request_mode_info(&mut self, port_id: u8, mode: u8, infotype: ModeInformationType) -> Result<()> {
        let msg =
        NotificationMessage::PortModeInformationRequest(ModeInformationRequest {
            port_id,
            mode,
            information_type: infotype,
        });
        self.send(msg).await
    }

}


// # PORTS
// PORT_A = 0x00
// PORT_B = 0x01
// PORT_LED = 0x34
// PORT_VOLTAGE = 0x3B
// PORT_RSSI = 0x3C


impl RemoteControl {
    /// Initialisation method
    pub async fn init(
        peripheral: Peripheral,
        lpf_characteristic: Characteristic,
    ) -> Result<Self> {
        // Peripheral is already connected before we get here

        let props = peripheral
            .properties()
            .await?
            .context("No properties found for hub")?;

        let mut port_map = PortMap::with_capacity(10);
        port_map.insert(Port::A, 0x0);
        port_map.insert(Port::B, 0x1);
        port_map.insert(Port::HubLed, 0x34);
        port_map.insert(Port::VoltageSensor, 0x3b);
        port_map.insert(Port::Rssi, 0x3c);

        let properties = HubProperties {
            mac_address: props.address.to_string(),
            name: props.local_name.unwrap_or_default(),
            rssi: props.tx_power_level.unwrap_or_default(),
            port_map,
            ..Default::default()
        };

        Ok(Self {
            peripheral,
            lpf_characteristic,
            properties,
            connected_io: Default::default(),
        })
    }

    // async fn port_from_id(&self, _port_id: u8) -> Option<Port> {
    // for (k, v) in self.port_map().await.iter() {
    //     if *v == port_id {
    //         return Some(*k);
    //     }
    // }
    // None
    // }
}
