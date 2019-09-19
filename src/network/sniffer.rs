use ::std::boxed::Box;

use ::pnet::datalink::{DataLinkReceiver, NetworkInterface};
use ::pnet::packet::ethernet::{EtherType, EthernetPacket};
use ::pnet::packet::ip::IpNextHeaderProtocol;
use ::pnet::packet::ipv4::Ipv4Packet;
use ::pnet::packet::tcp::TcpPacket;
use ::pnet::packet::udp::UdpPacket;
use ::pnet::packet::Packet;

use ::ipnetwork::IpNetwork;
use ::std::net::{IpAddr, SocketAddr};

use crate::network::{Connection, Protocol};

pub struct Segment {
    pub connection: Connection,
    pub direction: Direction,
    pub ip_length: u128,
}

#[derive(PartialEq, Hash, Eq, Debug, Clone, PartialOrd)]
pub enum Direction {
    Download,
    Upload,
}

impl Direction {
    pub fn new (network_interface_ips: &Vec<IpNetwork>, ip_packet: &Ipv4Packet) -> Self {
        match network_interface_ips
            .iter()
            .any(|ip_network| ip_network.ip() == ip_packet.get_source())
        {
            true => Direction::Upload,
            false => Direction::Download,
        }
    }
}

pub struct Sniffer {
    network_interface: NetworkInterface,
    network_frames: Box<DataLinkReceiver>,
}

impl Sniffer {
    pub fn new(network_interface: NetworkInterface, network_frames: Box<DataLinkReceiver>) -> Self {
        Sniffer {
            network_interface,
            network_frames,
        }
    }
    pub fn next(&mut self) -> Option<Segment> {
        // TODO: https://github.com/libpnet/libpnet/issues/343
        // make this non-blocking for faster exits
        let bytes = self.network_frames.next().unwrap_or_else(|e| {
            panic!("An error occurred while reading: {}", e);
        });
        let packet = EthernetPacket::new(bytes)?;
        match packet.get_ethertype() {
            EtherType(2048) => {
                let ip_packet = Ipv4Packet::new(packet.payload())?;
                let (protocol, source_port, destination_port) =
                    match ip_packet.get_next_level_protocol() {
                        IpNextHeaderProtocol(6) => {
                            let message = TcpPacket::new(ip_packet.payload())?;
                            (
                                Protocol::Tcp,
                                message.get_source(),
                                message.get_destination(),
                            )
                        }
                        IpNextHeaderProtocol(17) => {
                            let datagram = UdpPacket::new(ip_packet.payload())?;
                            (
                                Protocol::Udp,
                                datagram.get_source(),
                                datagram.get_destination(),
                            )
                        }
                        _ => return None,
                    };
                let direction = Direction::new(&self.network_interface.ips, &ip_packet);
                let from = SocketAddr::new(IpAddr::V4(ip_packet.get_source()), source_port);
                let to = SocketAddr::new(IpAddr::V4(ip_packet.get_destination()), destination_port );
                let mut connection = Connection::new(from, to, protocol)?;
                if let Direction::Download = direction {
                    connection.swap_direction();
                }
                let ip_length = ip_packet.get_total_length() as u128;
                Some(Segment { connection, ip_length, direction })
            }
            _ => None,
        }
    }
}