// #![allow(unused)]
use btleplug::api::{
    Central, CentralEvent, Manager as _, Peripheral as _, PeripheralProperties,
    ScanFilter, 
    ValueNotification
};
use btleplug::platform::{Adapter, Manager, PeripheralId, };
pub use btleplug;

// std
use futures::{stream::StreamExt, Stream};
pub use futures;
use std::sync::{Arc};
use core::time::Duration;    
use tokio::sync::Mutex;
use tokio::sync::broadcast; 
#[macro_use]
extern crate log;

// nostd
use num_traits::FromPrimitive;
use core::fmt::Debug;
use core::pin::Pin;

// Crate
pub mod consts;
pub mod devices;
pub mod error;
pub mod hubs;
pub mod notifications;
mod tests;

pub use hubs::Hub;
pub use crate::consts::IoTypeId;
pub use crate::devices::iodevice::IoDevice;

use notifications::{PortValueSingleFormat, PortValueCombinedFormat, NetworkCommand};
use consts::{BLEManufacturerData, HubType};

pub use error::{Error, OptionContext, Result};
// pub use consts::IoTypeId;

type HubMutex = Arc<Mutex<Box<dyn Hub>>>;
type PinnedStream = Pin<Box<dyn Stream<Item = ValueNotification> + Send>>;

pub struct PoweredUp {
    adapter: Adapter,
}

impl PoweredUp {
    pub async fn adapters() -> Result<Vec<Adapter>> {
        let manager = Manager::new().await?;
        Ok(manager.adapters().await?)
    }

    pub async fn init() -> Result<Self> {
        let manager = Manager::new().await?;
        let adapter = manager
            .adapters()
            .await?
            .into_iter()
            .next()
            .context("No adapter found")?;
        Self::with_adapter(adapter).await
    }

    pub async fn with_device_index(index: usize) -> Result<Self> {
        let manager = Manager::new().await?;
        let adapter = manager
            .adapters()
            .await?
            .into_iter()
            .nth(index)
            .context("No adapter found")?;
        Self::with_adapter(adapter).await
    }

    pub async fn with_adapter(adapter: Adapter) -> Result<Self> {
        Ok(Self { adapter })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.adapter.start_scan(ScanFilter::default()).await?;
        Ok(())
    }

    pub async fn find_hub(&mut self) -> Result<Option<DiscoveredHub>> {
        let hubs = self.list_discovered_hubs().await?;
        Ok(hubs.into_iter().next())
    }

    pub async fn list_discovered_hubs(&mut self) -> Result<Vec<DiscoveredHub>> {
        let peripherals = self.adapter.peripherals().await?;
        let mut hubs = Vec::new();
        for peripheral in peripherals {
            let Some(props) = peripheral.properties().await? else{continue;};
            if let Some(hub_type) = identify_hub(&props).await? {
                hubs.push(DiscoveredHub {
                    hub_type,
                    addr: peripheral.id(),
                    name: props
                        .local_name
                        .unwrap_or_else(|| "unknown".to_string()),
                });
            }
        }
        Ok(hubs)
    }

    pub async fn scan(&mut self) -> Result<impl Stream<Item = DiscoveredHub> + '_> {
        let events = self.adapter.events().await?;
        self.adapter.start_scan(ScanFilter::default()).await?;
        Ok(events.filter_map(|event| async {
            let CentralEvent::DeviceDiscovered(id) = event else { None? };
            // get peripheral info
            let peripheral = self.adapter.peripheral(&id).await.ok()?;
            println!("{:?}", peripheral.properties().await.unwrap());
            let Some(props) = peripheral.properties().await.ok()? else { None? };
            if let Some(hub_type) = identify_hub(&props).await.ok()? {
                let hub = DiscoveredHub {
                    hub_type,
                    addr: id,
                    name: props
                        .local_name
                        .unwrap_or_else(|| "unknown".to_string()),
                };
                Some(hub)
            } else { None }
        }))
    }

    pub async fn wait_for_hub(&mut self) -> Result<DiscoveredHub> {
        self.wait_for_hub_filter(HubFilter::Null).await
    }

    pub async fn wait_for_hub_filter(&mut self, filter: HubFilter) -> Result<DiscoveredHub> {
        let mut events = self.adapter.events().await?;
        self.adapter.start_scan(ScanFilter::default()).await?;
        while let Some(event) = events.next().await {
            let CentralEvent::DeviceDiscovered(id) = event else { continue };
            // get peripheral info
            let peripheral = self.adapter.peripheral(&id).await?;
            // println!("{:?}", peripheral.properties().await?);
            let Some(props) = peripheral.properties().await? else { continue };
            if let Some(hub_type) = identify_hub(&props).await? {
                let hub = DiscoveredHub {
                    hub_type,
                    addr: id,
                    name: props
                        .local_name
                        .unwrap_or_else(|| "unknown".to_string()),
                };
                if filter.matches(&hub) {
                    self.adapter.stop_scan().await?;
                    return Ok(hub);
                }
            }
        }
        panic!()
    }

    pub async fn wait_for_hubs_filter(&mut self, filter: HubFilter, count: &u8) -> Result<Vec<DiscoveredHub>> {
        let mut events = self.adapter.events().await?;
        let mut hubs = Vec::new();
        self.adapter.start_scan(ScanFilter::default()).await?;
        while let Some(event) = events.next().await {
            let CentralEvent::DeviceDiscovered(id) = event else { continue };
            // get peripheral info
            let peripheral = self.adapter.peripheral(&id).await?;
            // println!("{:?}", peripheral.properties().await?);
            let Some(props) = peripheral.properties().await? else { continue };
            if let Some(hub_type) = identify_hub(&props).await? {
                let hub = DiscoveredHub {
                    hub_type,
                    addr: id,
                    name: props
                        .local_name
                        .unwrap_or_else(|| "unknown".to_string()),
                };
                if filter.matches(&hub) {
                    hubs.push(hub);
                }
                if hubs.len() == *count as usize {
                    self.adapter.stop_scan().await?;
                    return Ok(hubs);    
                }
            }
        }
        panic!()
    }
   
    pub async fn create_hub(&mut self, hub: &DiscoveredHub,) -> Result<Box<dyn Hub>> {
        info!("Connecting to hub {}...", hub.addr,);

        let peripheral = self.adapter.peripheral(&hub.addr).await?;
        peripheral.connect().await?;
        peripheral.discover_services().await?;
        // tokio::time::sleep(Duration::from_secs(2)).await;
        let chars = peripheral.characteristics();

        // dbg!(&chars);

        let lpf_char = chars
            .iter()
            .find(|c| c.uuid == *consts::blecharacteristic::LPF2_ALL)
            .context("Device does not advertise LPF2_ALL characteristic")?
            .clone();

        match hub.hub_type {
            // These have had some real life-testing.
            HubType::TechnicMediumHub |
            HubType::MoveHub |
            HubType::RemoteControl  => {
                Ok(Box::new(hubs::generic_hub::GenericHub::init(
                    peripheral, lpf_char, hub.hub_type).await?))
            }
            // These are untested, but if they support the same "Lego Wireless protocol 3.0"
            // then they should probably work?
            HubType::Wedo2SmartHub |
            HubType::Hub |
            HubType::DuploTrainBase |
            HubType::Mario          => {
            Ok(Box::new(hubs::generic_hub::GenericHub::init(
                peripheral, lpf_char, hub.hub_type).await?))
            }
            // Here is some hub that advertises LPF2_ALL but is not in the known list.
            // Set kind to Unknown and give it a try, why not?
            _ => {
                Ok(Box::new(hubs::generic_hub::GenericHub::init(
                peripheral, lpf_char, HubType::Unknown).await?))
            }
        }
    }
}

/// Properties by which to filter discovered hubs
#[derive(Debug)]
pub enum HubFilter {
    /// Hub name must match the provided value
    Name(String),
    /// Hub address must match the provided value
    Addr(String),
    /// Always matches
    Null,
}

impl HubFilter {
    /// Test whether the discovered hub matches the provided filter mode
    pub fn matches(&self, hub: &DiscoveredHub) -> bool {
        use HubFilter::*;
        match self {
            Name(n) => hub.name == *n,
            Addr(a) => format!("{:?}", hub.addr) == *a,
            Null => true,
        }
    }
}

/// Struct describing a discovered hub. This description may be passed
/// to `PoweredUp::create_hub` to initialise a connection.
#[derive(Clone, Debug)]
pub struct DiscoveredHub {
    /// Type of hub, e.g. TechnicMediumHub
    pub hub_type: HubType,
    /// BLE address
    pub addr: PeripheralId,
    /// Friendly name of the hub, as set in the PoweredUp/Control+ apps
    pub name: String,
}

async fn identify_hub(props: &PeripheralProperties) -> Result<Option<HubType>> {
    use HubType::*;

    if props
        .services
        .contains(&consts::bleservice::WEDO2_SMART_HUB)
    {
        return Ok(Some(Wedo2SmartHub));
    } else if props.services.contains(&consts::bleservice::LPF2_HUB) {
        if let Some(manufacturer_id) = props.manufacturer_data.get(&919) {
            // Can't do it with a match because some devices are just manufacturer
            // data while some use other characteristics
            if let Some(m) = BLEManufacturerData::from_u8(manufacturer_id[1]) {
                use BLEManufacturerData::*;
                return Ok(Some(match m {
                    DuploTrainBaseId => DuploTrainBase,
                    HubId => Hub,
                    MarioId => Mario,
                    MoveHubId => MoveHub,
                    RemoteControlId => RemoteControl,
                    TechnicMediumHubId => TechnicMediumHub,
                }));
            }
        }
    }
    Ok(None)
}

pub struct ConnectedHub {
    pub name: String,
    pub mutex: HubMutex,
    pub kind: HubType,
}
impl ConnectedHub {
    pub async fn setup_hub (created_hub: Box<dyn Hub>) -> Result<ConnectedHub> {    
        let connected_hub = ConnectedHub {
            kind: created_hub.kind(),
            name: created_hub.name().await?,                                                    
            mutex: Arc::new(Mutex::new(created_hub)),
        };
        
        // Set up hub handlers
        //      Attached IO
        let name_to_handler = connected_hub.name.clone();
        let mutex_to_handler = connected_hub.mutex.clone();
        let singlevalue_sender = broadcast::channel::<PortValueSingleFormat>(3).0;
        let combinedvalue_sender = broadcast::channel::<PortValueCombinedFormat>(3).0;
        let networkcmd_sender = broadcast::channel::<NetworkCommand>(3).0;
        {
            let lock = &mut connected_hub.mutex.lock().await;
            let stream_to_handler: PinnedStream = lock.peripheral().notifications().await?;    
            lock.channels().singlevalue_sender = Some(singlevalue_sender.clone());
            lock.channels().combinedvalue_sender = Some(combinedvalue_sender.clone());
            lock.channels().networkcmd_sender = Some(networkcmd_sender.clone());
            tokio::spawn(async move {
                crate::hubs::io_event::io_event_handler(
                    stream_to_handler, 
                    mutex_to_handler,
          name_to_handler,
            singlevalue_sender,
                    combinedvalue_sender,
                    networkcmd_sender
                ).await.expect("Error setting up main handler");
            });
        }
        //  TODO    Hub alerts etc.
        
        // Subscribe to btleplug peripheral
        {
            let lock = connected_hub.mutex.lock().await;
            lock.peripheral().subscribe(&lock.characteristic()).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(1500)).await; //Wait for devices to be collected
        Ok(connected_hub)
    }
}