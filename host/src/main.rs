use std::io::{self, Read};
use std::time::Duration;

use clap::{App, AppSettings, Arg};

use serialport::prelude::*;

use slice_deque::SliceDeque;

use byteorder::{ByteOrder, LittleEndian};
use esercom;
use ieee802154::mac::{self, beacon::BeaconOrder};
use zigbee_rs::{
    self,
    application_service::{frame::ApplicationServiceFrame, security},
    network::{
        self,
        beacon::BeaconInformation,
        frame::{DiscoverRoute, NetworkFrame},
    },
    serde::SerdeVariableSize,
};

fn handle_security(payload: &[u8], offset: usize, mut output: &mut [u8]) -> usize {
    print!("SEC ");
    let network_key = [0u8; 16];
    match security::Frame::deserialize(&payload[offset..]) {
        Ok((header, _used)) => {
            print!(
                "Level {:?} Key Identifier {:?}",
                header.control.level, header.control.identifier
            );
            if let Some(src) = header.source {
                let mut source = [0u8; 8];
                LittleEndian::write_u64(&mut source[0..8], src);
                print!(" Source ");
                for b in source.iter() {
                    print!("{:02x} ", b);
                }
            }
            if let Some(seq) = header.sequence {
                print!(" Sequence {}", seq);
            }
            print!(" Counter {}", header.counter);
        }
        Err(e) => {
            println!("Failed to read header, {:?}", e);
            return 0;
        }
    }
    print!(" ");
    let result = security::handle_secure_payload(
        &network_key,
        security::SecurityLevel::EncryptedIntegrity32,
        &payload,
        offset,
        &mut output,
    );
    let size = match result {
        Ok(size) => {
            if size > 0 {
                print!("Payload: ");
                for b in output[..size].iter() {
                    print!("{:02x}", b);
                }
            } else {
                print!("Invalid Key");
            }
            size
        }
        Err(e) => {
            print!("Decryption failed, {:?}", e);
            0
        }
    };
    println!("");
    size
}

fn parse_application_service_frame(payload: &[u8]) {
    print!("APS ");
    match ApplicationServiceFrame::deserialize(payload) {
        Ok((frame, used)) => {
            print!(
                " {:?} {:?}",
                frame.control.frame_type, frame.control.delivery_mode,
            );
            if frame.control.security {
                print!(" Secure");
            }
            if frame.control.acknowledge_request {
                print!(" AckReq");
            }
            if frame.control.extended_header {
                print!(" ExtHdr");
            }
            if let Some(addr) = frame.destination {
                print!(" Dst {:02x}", addr);
            }
            if let Some(group) = frame.group {
                print!(" Group {:02x}{:02x}", group[0], group[1]);
            }
            if let Some(cluster) = frame.cluster {
                print!(" Cluster {:02x}{:02x}", cluster[0], cluster[1]);
            }
            if let Some(profile) = frame.profile {
                print!(" Profile {:02x}{:02x}", profile[0], profile[1]);
            }
            if let Some(addr) = frame.source {
                print!(" Src {:02x}", addr);
            }
            print!(" Counter {:02x} ", frame.counter);
            print!("Payload: ");
            for b in payload[used..].iter() {
                print!("{:02x}", b);
            }
        }
        Err(e) => {
            print!("Failed to parse APS header, {:?}", e);
        }
    }
    println!("");
}

fn parse_network_frame(payload: &[u8]) {
    match NetworkFrame::deserialize(payload) {
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
            let mut aps_payload = [0u8; 256];
            let length = if network_frame.control.security {
                handle_security(&payload, used, &mut aps_payload)
            } else {
                let length = payload.len() - used;
                aps_payload[..length].copy_from_slice(&payload[used..]);
                length
            };
            if length > 0 {
                match network_frame.control.frame_type {
                    network::frame::FrameType::Data | network::frame::FrameType::InterPan => {
                        parse_application_service_frame(&aps_payload[..length])
                    }
                    network::frame::FrameType::Command => {
                        match network::commands::Command::new_command(&aps_payload[..length]) {
                            Ok((cmd, _used)) => {
                                println!("Command {:?}", cmd);
                            }
                            Err(e) => {
                                println!("Failed to decode network command, {:?}", e);
                            }
                        }
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

fn parse_packet(packet: &[u8]) {
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
                    print!(" Payload: ");
                    for b in frame.payload.iter() {
                        print!("{:02x}", b);
                    }
                    println!("");
                    match BeaconInformation::deserialize(frame.payload) {
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
                            println!("Failed to parse beacon information, {:?}", e);
                        }
                    }
                }
                mac::FrameContent::Data => {
                    // TODO: Parse data at higher layer?
                    println!("");
                    parse_network_frame(frame.payload);
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

fn main() {
    let matches = App::new("nRF52840-DK host companion")
        .about("Write stuff")
        .setting(AppSettings::DisableVersion)
        .arg(
            Arg::with_name("port")
                .help("The device path to a serial port")
                .use_delimiter(false)
                .required(true),
        )
        .get_matches();

    let port_name = matches.value_of("port").unwrap();
    let mut settings: SerialPortSettings = Default::default();
    settings.baud_rate = 115200;
    settings.timeout = Duration::from_millis(1000);

    let mut buffer: SliceDeque<u8> = SliceDeque::with_capacity(256);
    let mut data = [0u8; 256];
    let mut pkt_data = [0u8; 256];

    match serialport::open_with_settings(&port_name, &settings) {
        Ok(mut port) => {
            println!("Read packets over {}", &port_name);
            loop {
                match port.read(&mut data) {
                    Ok(rx_count) => {
                        buffer.extend_from_slice(&data[..rx_count]);
                        loop {
                            match esercom::com_decode(buffer.as_slice(), &mut data) {
                                Ok((msg, used, written)) => {
                                    if written == 0 {
                                        break;
                                    }
                                    match msg {
                                        esercom::MessageType::RadioReceive => {
                                            let pkt_len = written;
                                            let link_quality_indicator = data[pkt_len - 1];
                                            let pkt_len = pkt_len - 1; // Remove LQI
                                            pkt_data[..pkt_len].copy_from_slice(&data[..pkt_len]);
                                            println!(
                                                "## Packet {} LQI {}",
                                                pkt_len, link_quality_indicator
                                            );
                                            for b in &pkt_data[..pkt_len] {
                                                print!("{:02x}", b);
                                            }
                                            println!("");
                                            parse_packet(&pkt_data[..pkt_len]);
                                        }
                                        esercom::MessageType::EnergyDetect => {
                                            if written == 2 {
                                                let channel = data[0];
                                                let energy_level = data[1];
                                                println!(
                                                    "## Energy on channel {}: {}",
                                                    channel, energy_level
                                                );
                                            }
                                        }
                                        _ => println!("Other packet {:?}", msg),
                                    }
                                    buffer.truncate_front(buffer.len() - used);
                                }
                                Err(ref e) => {
                                    match *e {
                                        esercom::error::Error::EndNotFound => (),
                                        esercom::error::Error::InvalidLength(l) => {
                                            buffer.truncate_front(buffer.len() - l);
                                        }
                                        _ => {
                                            println!("Bad {:?}", e);
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                    Err(e) => eprintln!("{:?}", e),
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to open \"{}\". Error: {}", port_name, e);
            ::std::process::exit(1);
        }
    }
}
