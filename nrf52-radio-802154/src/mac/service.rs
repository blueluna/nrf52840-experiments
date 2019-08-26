//! 802.15.4 MAC services

use ieee802154::mac::{
    command::{AssociationStatus, CapabilityInformation, Command},
    Address, AddressMode, ExtendedAddress, Frame, FrameContent, FrameType, FrameVersion, Header,
    PanId, Security, ShortAddress, WriteFooter,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Orphan,
    ActiveScan,
    Join,
    QueryStatus,
    Associated,
}

#[derive(Clone, Copy, Debug)]
pub struct Identity {
    pub id: Option<PanId>,
    pub short: Option<ShortAddress>,
    pub extended: Option<ExtendedAddress>,
}

impl Identity {
    fn new() -> Self {
        Identity {
            id: None,
            short: None,
            extended: None,
        }
    }
}

pub struct Service {
    state: State,
    sequence: u8,
    pending_acknowledge: bool,
    id: Identity,
    coordinator_id: Identity,
    last_header: Header,
}

impl Service {
    pub fn new(extended: ExtendedAddress) -> Self {
        let id = Identity {
            id: None,
            short: None,
            extended: Some(extended),
        };
        Service {
            state: State::Orphan,
            sequence: 0,
            pending_acknowledge: false,
            id,
            coordinator_id: Identity::new(),
            last_header: Header {
                seq: 0,
                frame_type: FrameType::Acknowledgement,
                security: Security::None,
                frame_pending: false,
                ack_request: false,
                pan_id_compress: false,
                version: FrameVersion::Ieee802154_2003,
                destination: Address::None,
                source: Address::None,
            },
        }
    }

    fn handle_beacon(&mut self, frame: &Frame) -> u32 {
        let (src_id, src_short) = if let Address::Short(id, short) = frame.header.source {
            (id, short)
        } else {
            return 0;
        };
        if let FrameContent::Beacon(beacon) = &frame.content {
            if beacon.superframe_spec.pan_coordinator && beacon.superframe_spec.association_permit {
                if let State::ActiveScan = self.state {
                    self.coordinator_id.id = Some(src_id);
                    self.coordinator_id.short = Some(src_short);
                    self.state = State::Join;
                }
            }
        }
        0
    }

    fn handle_mac_command(&mut self, frame: &Frame) -> u32 {
        if let FrameContent::Command(command) = &frame.content {
            if let Command::AssociationResponse(addr, status) = command {
                if *status == AssociationStatus::Successful {
                    if let State::QueryStatus = self.state {
                        self.id.id = frame.header.source.pan_id();
                        self.id.short = Some(*addr);
                        self.state = State::Associated;
                    }
                }
            }
        }
        0
    }

    fn handle_acknowledge(&mut self, frame: &Frame) -> u32 {
        if frame.header.seq == self.sequence {
            match self.state {
                State::Join => {
                    self.state = State::QueryStatus;
                    10
                }
                _ => 0,
            }
        } else {
            0
        }
    }

    pub fn radio_receive(&mut self, data: &[u8]) -> u32 {
        match Frame::decode(data, false) {
            Ok(frame) => {
                self.pending_acknowledge = if frame.header.ack_request {
                    match frame.header.destination {
                        Address::None => false,
                        Address::Short(_, dst) => {
                            if let Some(address) = self.id.short {
                                address == dst
                            } else {
                                false
                            }
                        }
                        Address::Extended(_, dst) => {
                            if let Some(address) = self.id.extended {
                                address == dst
                            } else {
                                false
                            }
                        }
                    }
                } else {
                    false
                };
                let pending_tx = match frame.header.frame_type {
                    FrameType::Acknowledgement => self.handle_acknowledge(&frame),
                    FrameType::Beacon => self.handle_beacon(&frame),
                    FrameType::Data => 0,
                    FrameType::MacCommand => self.handle_mac_command(&frame),
                };
                self.last_header = frame.header;
                if self.pending_acknowledge {
                    10 // send ack after 10 us
                } else {
                    pending_tx
                }
            }
            Err(_) => 0,
        }
    }

    fn sequence_next(&mut self) -> u8 {
        (*self).sequence = (*self).sequence.wrapping_add(1);
        (*self).sequence
    }

    /// Build a Imm-Ack frame
    fn build_acknowledge(&mut self, mut data: &mut [u8]) -> (usize, u32) {
        // IEEE 802.15.4-2015 chapter 7.3.3
        //
        // +-------------+--------+---------+-------------+----------+----------+
        // | Destination | Source | Pending | Acknowledge | Compress | Security |
        // +-------------+--------+---------+-------------+----------+----------+
        // | None        | None   | 1       | false       | false    | false    |
        // +-------------+--------+---------+-------------+----------+----------+
        //
        // 1. If this is a response to a data reuqest frame, this is set to true
        //    if there is data pending, otherwise false.
        //
        // No payload
        //
        let frame = Frame {
            header: Header {
                seq: self.last_header.seq,
                frame_type: FrameType::Acknowledgement,
                security: Security::None,
                frame_pending: false,
                ack_request: false,
                pan_id_compress: false,
                version: FrameVersion::Ieee802154_2003,
                destination: Address::None,
                source: Address::None,
            },
            content: FrameContent::Acknowledgement,
            payload: &[],
            footer: [0u8; 2],
        };
        self.pending_acknowledge = false;
        (frame.encode(&mut data, WriteFooter::No), 0)
    }

    /// Build a beacon request frame
    fn build_beacon_request(&mut self, mut data: &mut [u8]) -> (usize, u32) {
        // IEEE 802.15.4-2015 chapter 7.5.8
        //
        // +-------------+--------+---------+-------------+----------+----------+
        // | Destination | Source | Pending | Acknowledge | Compress | Security |
        // +-------------+--------+---------+-------------+----------+----------+
        // | Short       | None   | false   | false       | false    | false    |
        // +-------------+--------+---------+-------------+----------+----------+
        //
        // +------------+------------+-------------+-----------+
        // | Dst PAN Id | Src PAN Id | Destination | Source    |
        // +------------+------------+-------------+-----------+
        // | Broadcast  |            | Broadcast   |           |
        // +------------+------------+-------------+-----------+
        //
        // No payload
        //
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
            content: FrameContent::Command(Command::BeaconRequest),
            payload: &[],
            footer: [0u8; 2],
        };
        self.state = State::ActiveScan;
        (frame.encode(&mut data, WriteFooter::No), 1_000_000)
    }

    fn build_association_request(&mut self, mut data: &mut [u8]) -> (usize, u32) {
        let command = Command::AssociationRequest(CapabilityInformation {
            full_function_device: true,
            mains_power: true,
            idle_receive: true,
            frame_protection: false,
            allocate_address: true,
        });
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
            content: FrameContent::Command(command),
            payload: &[],
            footer: [0u8; 2],
        };
        (frame.encode(&mut data, WriteFooter::No), 1_000_000)
    }

    fn build_data_request(&mut self, mut data: &mut [u8]) -> (usize, u32) {
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
            content: FrameContent::Command(Command::DataRequest),
            payload: &[0u8; 0],
            footer: [0u8; 2],
        };
        (frame.encode(&mut data, WriteFooter::No), 1_000_000)
    }

    pub fn build_packet(&mut self, mut data: &mut [u8]) -> (usize, u32) {
        if self.pending_acknowledge {
            self.build_acknowledge(&mut data)
        } else {
            match self.state {
                State::Orphan => self.build_beacon_request(&mut data),
                State::ActiveScan => {
                    self.state = State::Orphan;
                    (0, 29_000_000)
                }
                State::Join => self.build_association_request(&mut data),
                State::QueryStatus => self.build_data_request(&mut data),
                State::Associated => (0, 0),
            }
        }
    }

    pub fn state(&self) -> State {
        self.state
    }
}
