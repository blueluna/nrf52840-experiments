//! 802.15.4 Network Layer

use ieee802154::{
    beacon::Beacon,
    mac::{
        Address, AddressMode, ExtendedAddress, Frame, FrameType, FrameVersion, Header, PanId,
        Security, ShortAddress, WriteFooter,
    },
    mac_command::{AssociationStatus, CapabilityInformation, Command},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkState {
    Orphan,
    Join,
    QueryStatus,
    Associated,
}

#[derive(Clone, Copy, Debug)]
pub struct NetworkIdentity {
    pub id: Option<PanId>,
    pub short: Option<ShortAddress>,
    pub extended: Option<ExtendedAddress>,
}

impl NetworkIdentity {
    fn new() -> Self {
        NetworkIdentity {
            id: None,
            short: None,
            extended: None,
        }
    }
}

pub struct NetworkLayer {
    state: NetworkState,
    sequence: u8,
    id: NetworkIdentity,
    coordinator_id: NetworkIdentity,
}

impl NetworkLayer {
    pub fn new(extended: ExtendedAddress) -> Self {
        let id = NetworkIdentity {
            id: None,
            short: None,
            extended: Some(extended),
        };
        NetworkLayer {
            state: NetworkState::Orphan,
            sequence: 0,
            id,
            coordinator_id: NetworkIdentity::new(),
        }
    }

    fn handle_beacon(&mut self, frame: &Frame) -> bool {
        let (src_id, src_short) = if let Address::Short(id, short) = frame.header.source {
            (id, short)
        } else {
            return false;
        };
        match Beacon::decode(frame.payload) {
            Ok((beacon, _)) => {
                if beacon.superframe_spec.pan_coordinator
                    && beacon.superframe_spec.association_permit
                {
                    match self.state {
                        NetworkState::Orphan => {
                            self.coordinator_id.id = Some(src_id);
                            self.coordinator_id.short = Some(src_short);
                            self.state = NetworkState::Join;
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    fn handle_mac_command(&mut self, frame: &Frame) -> bool {
        match Command::decode(frame.payload) {
            Ok((command, _)) => match command {
                Command::AssociationResponse(addr, status) => {
                    if status == AssociationStatus::Successful {
                        match self.state {
                            NetworkState::QueryStatus => {
                                self.id.id = frame.header.source.pan_id();
                                self.id.short = Some(addr);
                                self.state = NetworkState::Associated;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Err(_) => {}
        }
        false
    }

    pub fn radio_receive(&mut self, data: &[u8]) -> bool {
        match Frame::decode(data) {
            Ok(frame) => match frame.header.frame_type {
                FrameType::Acknowledgement => match self.state {
                    NetworkState::Join => {
                        self.state = NetworkState::QueryStatus;
                        true
                    }
                    _ => false,
                },
                FrameType::Beacon => self.handle_beacon(&frame),
                FrameType::Data => false,
                FrameType::MacCommand => self.handle_mac_command(&frame),
            },
            Err(_) => false,
        }
    }

    fn sequence_next(&mut self) -> u8 {
        let s = (*self).sequence;
        (*self).sequence = (*self).sequence.wrapping_add(1);
        s
    }

    fn build_beacon(&mut self, mut data: &mut [u8]) -> usize {
        let mut payload = [0u8; 1];
        let command = Command::BeaconRequest;
        let size = command.encode(&mut payload);
        let frame = Frame {
            header: Header {
                seq: self.sequence_next(),
                frame_type: FrameType::MacCommand,
                security: Security::None,
                frame_pending: false,
                ack_request: false,
                pan_id_compress: false,
                version: FrameVersion::Ieee802154_2003,
                destination: Address::broadcast(&AddressMode::Short),
                source: Address::None,
            },
            payload: &payload[..size],
            footer: [0u8; 2],
        };
        frame.encode(&mut data, WriteFooter::No)
    }

    fn build_association_request(&mut self, mut data: &mut [u8]) -> usize {
        let mut payload = [0u8; 2];
        let command = Command::AssociationRequest(CapabilityInformation {
            full_function_device: true,
            mains_power: true,
            idle_receive: true,
            frame_protection: false,
            allocate_address: true,
        });
        let size = command.encode(&mut payload);
        let frame = Frame {
            header: Header {
                seq: self.sequence_next(),
                frame_type: FrameType::MacCommand,
                security: Security::None,
                frame_pending: false,
                ack_request: true,
                pan_id_compress: false,
                version: FrameVersion::Ieee802154_2003,
                destination: Address::Short(
                    self.coordinator_id.id.unwrap(),
                    self.coordinator_id.short.unwrap(),
                ),
                source: Address::Extended(PanId::broadcast(), self.id.extended.unwrap()),
            },
            payload: &payload[..size],
            footer: [0u8; 2],
        };
        frame.encode(&mut data, WriteFooter::No)
    }

    fn build_data_request(&mut self, mut data: &mut [u8]) -> usize {
        let mut payload = [0u8; 1];
        let command = Command::DataRequest;
        let size = command.encode(&mut payload);
        let frame = Frame {
            header: Header {
                seq: self.sequence_next(),
                frame_type: FrameType::MacCommand,
                security: Security::None,
                frame_pending: false,
                ack_request: true,
                pan_id_compress: true,
                version: FrameVersion::Ieee802154_2003,
                destination: Address::Short(
                    self.coordinator_id.id.unwrap(),
                    self.coordinator_id.short.unwrap(),
                ),
                source: Address::Extended(
                    self.coordinator_id.id.unwrap(),
                    self.id.extended.unwrap(),
                ),
            },
            payload: &payload[..size],
            footer: [0u8; 2],
        };
        frame.encode(&mut data, WriteFooter::No)
    }

    pub fn build_packet(&mut self, mut data: &mut [u8]) -> usize {
        match self.state {
            NetworkState::Orphan => self.build_beacon(&mut data),
            NetworkState::Join => self.build_association_request(&mut data),
            NetworkState::QueryStatus => self.build_data_request(&mut data),
            NetworkState::Associated => 0,
        }
    }

    pub fn state(&self) -> NetworkState {
        self.state
    }
}
