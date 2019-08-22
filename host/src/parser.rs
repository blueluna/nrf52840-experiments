use std::convert::TryFrom;

use crate::security::SecurityService;

use ieee802154::mac::{self, beacon::BeaconOrder};
use zigbee_rs::{
    self,
    application_service::{self, ApplicationServiceHeader},
    common::profile_identifier::ProfileIdentifier,
    device_profile::DeviceProfileFrame,
    network::{
        self,
        beacon::BeaconInformation,
        header::DiscoverRoute,
        NetworkHeader,
    },
    pack::Pack,
};

pub struct Parser {
	pub security: SecurityService,
}

impl Parser {
	pub fn new() -> Self {
		Parser{security: SecurityService::new()}
	}

	fn handle_device_profile(&self, payload: &[u8], cluster: u16) {
	    use zigbee_rs::device_profile::DeviceProfileMessage;

	    print!("ZDP ");
	    match DeviceProfileFrame::unpack(payload, cluster) {
	        Ok((frame, _)) => {
	            print!(
	                "SEQ {} ",
	                frame.transaction_sequence,
	            );
	            match frame.message {
	                DeviceProfileMessage::NetworkAddressRequest(req) => {
	                    print!("Network Address Request {} {:?} Start {}", req.address, req.request_type, req.start_index);
	                }
	                DeviceProfileMessage::NetworkAddressResponse(rsp) => {
	                    print!("Network Address Response {:?} {} {}", rsp.status, rsp.network_address, rsp.ieee_address);
	                    if let Some(index) = rsp.start_index {
	                        print!(" Start {}", index);
	                    }
	                    if let Some(associated_devices) = rsp.associated_devices {
	                        print!(" Associated Devices");
	                        for address in associated_devices {
	                            print!(" {}", address);
	                        }
	                    }
	                }
	                DeviceProfileMessage::IeeeAddressRequest(req) => {
	                    print!("Network Address Request {} {:?} Start {}", req.address, req.request_type, req.start_index);
	                }
	                DeviceProfileMessage::IeeeAddressResponse(rsp) => {
	                    print!("Network Address Response {:?} {} {}", rsp.status, rsp.network_address, rsp.ieee_address);
	                    if let Some(index) = rsp.start_index {
	                        print!(" Start {}", index);
	                    }
	                    if let Some(associated_devices) = rsp.associated_devices {
	                        print!(" Associated Devices");
	                        for address in associated_devices {
	                            print!(" {}", address);
	                        }
	                    }
	                }
	                DeviceProfileMessage::NodeDescriptorRequest(req) => {
	                    print!("Node Descriptor Request {}", req.address);
	                }
	                DeviceProfileMessage::MatchDescriptorRequest(req) => {
	                    print!("Match Descriptor Request {:?}", req);
	                }
	                DeviceProfileMessage::MatchDescriptorResponse(rsp) => {
	                    print!("Match Descriptor Response {:?}", rsp);
	                }
	                DeviceProfileMessage::DeviceAnnounce(da) => {
	                    print!("Device Announce {} {} {:?}", da.network_address, da.ieee_address, da.capability);
	                }
	                DeviceProfileMessage::ManagementLinkQualityIndicatorRequest(start_index) => {
	                    print!("LQI Request {} ", start_index);
	                }
	                DeviceProfileMessage::ManagementLinkQualityIndicatorResponse(rsp) => {
	                    print!("LQI Response {:?} ", rsp);
	                }
	            }
	        }
	        Err(e) => {
	            print!("Failed to parse ZDP frame, {:?}", e);
	        }
	    }
	    println!("");
	}

	fn parse_application_service_frame(&self, payload: &[u8]) {
	    print!("APS ");
	    match ApplicationServiceHeader::unpack(payload) {
	        Ok((header, used)) => {
	            print!(
	                "{:?} {:?} ",
	                header.control.frame_type, header.control.delivery_mode,
	            );
	            if header.control.security {
	                print!("Secure ");
	            }
	            if header.control.acknowledge_request {
	                print!("AckReq ");
	            }
	            if header.control.extended_header {
	                print!("ExtHdr ");
	            }
	            if let Some(addr) = header.destination {
	                print!("Dst {:02x} ", addr);
	            }
	            if let Some(group) = header.group {
	                print!("Group {:04x} ", group);
	            }
	            if let Some(cluster) = header.cluster {
	                print!("Cluster {:04x} ", cluster);
	            }
	            if let Some(profile) = header.profile {
	                print!("Profile {:04x} ", profile);
	            }
	            if let Some(addr) = header.source {
	                print!("Src {:02x} ", addr);
	            }
	            print!("Counter {:02x} ", header.counter);
	            print!("Payload: ");
	            for b in payload[used..].iter() {
	                print!("{:02x}", b);
	            }
	            println!("");
	            let mut processed_payload = [0u8; 256];
	            let length = if header.control.security {
	                self.security.decrypt(&payload, used, &mut processed_payload)
	            } else {
	                let length = payload.len() - used;
	                processed_payload[..length].copy_from_slice(&payload[used..]);
	                length
	            };
	            match header.control.frame_type {
	                application_service::header::FrameType::Data => {
	                    if let (Some(cluster), Some(profile)) = (header.cluster, header.profile) {
	                        match ProfileIdentifier::try_from(profile) {
	                            Ok(profile) => {
	                                match profile {
	                                    ProfileIdentifier::DeviceProfile => {
	                                        self.handle_device_profile(&processed_payload[..length], cluster);
	                                    }
	                                    _ => {
	                                        println!("{:?}", profile);
	                                    }
	                                }
	                            }
	                            Err(_) => (),
	                        }
	                    }
	                }
	                application_service::header::FrameType::Command => (),
	                application_service::header::FrameType::Acknowledgement => (),
	                application_service::header::FrameType::InterPan => (),

	            }
	        }
	        Err(e) => {
	            println!("Failed to parse APS header, {:?}", e);
	        }
	    }
	}

	fn parse_network_command(&self, payload: &[u8])
	{
	    use network::commands::Command;
	    print!("NWK CMD ");
	    match Command::unpack(payload) {
	        Ok((cmd, _used)) => {
	            match cmd {
	                Command::RouteRequest(rr) => {
	                    print!("Route Request {:02x} Cost {}", rr.identifier, rr.path_cost);
	                    match rr.destination_address {
	                        network::address::AddressType::Singlecast(a) => print!(" Destination {}", a),
	                        network::address::AddressType::Multicast(a) => print!(" Group {}", a),
	                    }
	                    if let Some(address) = rr.destination_ieee_address {
	                        print!(" Destination {}", address);
	                    }
	                    println!("");

	                }
	                Command::RouteReply(rr) => {
	                    println!("Route Reply {:?}", rr);
	                }
	                Command::NetworkStatus(ns) => {
	                    println!("Network Status {:?}", ns);
	                }
	                Command::Leave(leave) => {
	                    println!("Leave {:?}", leave);
	                }
	                Command::RouteRecord(rr) => {
	                    print!("Route Record ");
	                    for address in rr.relay_list {
	                        print!("{} ", address);
	                    }
	                    println!("");
	                }
	                Command::RejoinRequest(rr) => {
	                    println!("Rejoin Request{:?}", rr);
	                }
	                Command::RejoinResponse(rr) => {
	                    println!("Rejoin Response {:?}", rr);
	                }
	                Command::LinkStatus(ls) => {
	                    print!("Link Status ");
	                    if ls.first_frame && !ls.last_frame {
	                        print!("First ");
	                    }
	                    else if !ls.first_frame && ls.last_frame {
	                        print!("Last ");
	                    }
	                    for entry in ls.entries {
	                        print!("{} Incoming {} Outgoing {} ", entry.address, entry.incoming_cost, entry.outgoing_cost);
	                    }
	                    println!("");
	                }
	                Command::NetworkReport => {
	                    println!("Network Report");
	                }
	                Command::NetworkUpdate => {
	                    println!("Network Update");
	                }
	                Command::EndDeviceTimeoutRequest => {
	                    println!("End-device Timeout Request");
	                }
	                Command::EndDeviceTimeoutResponse => {
	                    println!("End-device Timeout Response");
	                }
	            }
	        }
	        Err(e) => {
	            println!("Failed to decode network command, {:?}", e);
	        }
	    }

	}

	fn parse_network_frame(&self, payload: &[u8]) {
	    match NetworkHeader::unpack(payload) {
	        Ok((network_frame, used)) => {
	            print!("NWK TYP {:?} ", network_frame.control.frame_type);
	            print!("VER {} ", network_frame.control.protocol_version);
	            match network_frame.control.discover_route {
	                DiscoverRoute::EnableDiscovery => {
	                    print!("DSC ");
	                }
	                DiscoverRoute::SurpressDiscovery => {}
	            }
	            if network_frame.control.security {
	                print!("SEC ");
	            }
	            print!("DST {} ", network_frame.destination_address);
	            print!("SRC {} ", network_frame.source_address);
	            print!("RAD {} ", network_frame.radius);
	            print!("SEQ {} ", network_frame.sequence_number);
	            if let Some(dst) = network_frame.destination_ieee_address {
	                print!("DST {} ", dst);
	            }
	            if let Some(src) = network_frame.source_ieee_address {
	                print!("SRC {} ", src);
	            }
	            if let Some(mc) = network_frame.multicast_control {
	                print!("MC {:?} ", mc);
	            }
	            if let Some(srf) = network_frame.source_route_frame {
	                print!("SRF {:?} ", srf);
	            }
	            print!("Payload: ");
	            for b in payload[used..].iter() {
	                print!("{:02x}", b);
	            }
	            println!("");
	            let mut processed_payload = [0u8; 256];
	            let length = if network_frame.control.security {
	                self.security.decrypt(&payload, used, &mut processed_payload)
	            } else {
	                let length = payload.len() - used;
	                processed_payload[..length].copy_from_slice(&payload[used..]);
	                length
	            };
	            if length > 0 {
	                match network_frame.control.frame_type {
	                    network::header::FrameType::Data | network::header::FrameType::InterPan => {
	                        self.parse_application_service_frame(&processed_payload[..length])
	                    }
	                    network::header::FrameType::Command => {
	                        self.parse_network_command(&processed_payload[..length]);
	                    }
	                }
	            }
	        }
	        Err(ref e) => {
	            print!("Failed to decode network frame, ");
	            match e {
	                zigbee_rs::Error::NotEnoughSpace => {
	                    print!("Not enough space");
	                }
	                zigbee_rs::Error::WrongNumberOfBytes => {
	                    print!("Wrong number of bytes");
	                }
	                zigbee_rs::Error::UnknownFrameType => {
	                    print!("Unkown frame type");
	                }
	                zigbee_rs::Error::BrokenRelayList => {
	                    print!("Broken relay list");
	                }
	                zigbee_rs::Error::UnknownNetworkCommand => {
	                    print!("Unkown network command");
	                }
	                zigbee_rs::Error::UnknownDeliveryMode => {
	                    print!("Unkown delivery mode");
	                }
	                _ => {
	                    print!("{:?}", e);
	                }
	            }
	            println!("");
	        }
	    }
	}

	fn parse_mac(&self, packet: &[u8]) {
	    use mac::Address;
	    match mac::Frame::decode(packet, false) {
	        Ok(frame) => {
	            print!("802.15.4");
	            match frame.header.frame_type {
	                mac::FrameType::Acknowledgement => {
	                    print!(" TYPE: Acknowledgement");
	                }
	                mac::FrameType::Beacon => {
	                    print!(" TYPE: Beacon");
	                }
	                mac::FrameType::Data => {
	                    print!(" TYPE: Data");
	                }
	                mac::FrameType::MacCommand => {
	                    print!(" TYPE: Command");
	                }
	            }
	            print!(
	                "{}",
	                if frame.header.frame_pending {
	                    " PEND"
	                } else {
	                    ""
	                }
	            );
	            print!("{}", if frame.header.ack_request { " ACK" } else { "" });
	            print!(
	                "{}",
	                if frame.header.pan_id_compress {
	                    " CMPR"
	                } else {
	                    ""
	                }
	            );
	            print!(" SEQ: {}", frame.header.seq);
	            match frame.header.destination {
	                Address::Short(i, a) => {
	                    print!(" DST: {:04x}:{:04x}", i.0, a.0);
	                }
	                Address::Extended(i, a) => {
	                    print!(" DST: {:04x}:{:016x}", i.0, a.0);
	                }
	                Address::None => {
	                    print!(" DST: None");
	                }
	            }
	            match frame.header.source {
	                Address::Short(i, a) => {
	                    print!(" SRC: {:04x}:{:04x}", i.0, a.0);
	                }
	                Address::Extended(i, a) => {
	                    print!(" SRC: {:04x}:{:016x}", i.0, a.0);
	                }
	                Address::None => {
	                    print!(" SRC: None");
	                }
	            }
	            match frame.content {
	                mac::FrameContent::Acknowledgement => {
	                    // Nothing here
	                    println!("");
	                }
	                mac::FrameContent::Beacon(beacon) => {
	                    print!(" Beacon ");
	                    if beacon.superframe_spec.beacon_order != BeaconOrder::OnDemand {
	                        print!(
	                            "Beacon order {:?} Superframe order {:?} Final CAP slot {}",
	                            beacon.superframe_spec.beacon_order,
	                            beacon.superframe_spec.superframe_order,
	                            beacon.superframe_spec.final_cap_slot
	                        )
	                    }
	                    let coordinator = if beacon.superframe_spec.pan_coordinator {
	                        "Coordinator"
	                    } else {
	                        "Device"
	                    };
	                    let association_permit = if beacon.superframe_spec.association_permit {
	                        "Permit association"
	                    } else {
	                        "Deny association"
	                    };
	                    print!("\"{}\" \"{}\"", coordinator, association_permit);
	                    if beacon.superframe_spec.battery_life_extension {
	                        print!("\"Battery life extension\"");
	                    }
	                    if beacon.guaranteed_time_slot_info.permit {
	                        print!(
	                            "GTS slots {}",
	                            beacon.guaranteed_time_slot_info.slots().len()
	                        )
	                    }
	                    match BeaconInformation::unpack(frame.payload) {
	                        Ok((bi, _)) => {
	                            let router = if bi.router_capacity { "Router" } else { "" };
	                            let end_device = if bi.end_device_capacity {
	                                "End Device"
	                            } else {
	                                ""
	                            };
	                            println!("Protocol {:?} Stack {:?} Version {} {} Depth {} {} Address {} TX offset {:08x} Update {:02x}",
	                                     bi.protocol_indentifier,
	                                     bi.stack_profile,
	                                     bi.network_protocol_version,
	                                     router,
	                                     bi.device_depth,
	                                     end_device,
	                                     bi.extended_pan_address,
	                                     bi.tx_offset,
	                                     bi.network_update_identifier,
	                            );
	                        }
	                        Err(e) => {
	                            print!("Failed to parse beacon information, {:?}", e);
			                    print!(" Payload: ");
			                    for b in frame.payload.iter() {
			                        print!("{:02x}", b);
			                    }
			                    println!("");
	                        }
	                    }
	                }
	                mac::FrameContent::Data => {
	                    println!("");
	                    self.parse_network_frame(frame.payload);
	                }
	                mac::FrameContent::Command(command) => {
	                    println!(" Command {:?}", command);
	                }
	            }
	        }
	        Err(e) => {
	            print!("Unknown Packet, ");
	            match e {
	                mac::DecodeError::NotEnoughBytes => {
	                    println!("NotEnoughBytes");
	                }
	                mac::DecodeError::InvalidFrameType(_) => {
	                    println!("InvalidFrameType");
	                }
	                mac::DecodeError::SecurityNotSupported => {
	                    println!("SecurityNotSupported");
	                }
	                mac::DecodeError::InvalidAddressMode(_) => {
	                    println!("Invalid Address Mode");
	                }
	                mac::DecodeError::InvalidFrameVersion(_) => {
	                    println!("InvalidFrameVersion");
	                }
	                mac::DecodeError::InvalidValue => {
	                    println!("InvalidValue");
	                }
	            }
	        }
	    }
	}
	
	pub fn parse_packet(&mut self, packet: &[u8]) {
		self.parse_mac(packet);
	}
}